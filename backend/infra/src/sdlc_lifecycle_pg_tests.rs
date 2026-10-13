//! Synthetic repository fixtures, not receipts from a real Fleet/Workflow execution.
use super::*;
use domain::sdlc_execution_lease::ClaimExecutionLease;
use domain::sdlc_reservation::CompletePmDraft;
use domain::sdlc_routing::{RoleRoutes, SetRoutingPolicy};
use sea_orm_migration::MigratorTrait;
use serde::de::DeserializeOwned;

#[path = "sdlc_draft_flow_pg_tests.rs"]
mod draft_flow_tests;
#[path = "sdlc_pm_resume_pg_tests.rs"]
mod pm_resume_tests;

fn parse<T: DeserializeOwned>(value: Json) -> T {
    serde_json::from_value(value).unwrap()
}

fn actor(subject: &str, human: bool, extra: Vec<String>) -> Principal {
    let mut scopes: std::collections::HashSet<String> = extra.into_iter().collect();
    if !human {
        scopes.insert("task-tracker:write".into());
    }
    Principal {
        subject: subject.into(),
        human_session: human,
        trusted_human: human,
        scopes,
    }
}

fn artifact(label: &str) -> ArtifactReceipt {
    ArtifactReceipt {
        reference: format!("fixture://{label}"),
        sha256: "a".repeat(64),
    }
}

fn routes() -> RoleRoutes {
    let mut routes = json!({});
    for (i, role) in [
        "project_manager",
        "analyst",
        "architect",
        "developer",
        "reviewer",
        "tester",
        "devops",
    ]
    .iter()
    .enumerate()
    {
        let suffix = role.replace('_', "-");
        let profile = match *role {
            "tester" => "hermes-sdlc-quality".into(),
            "devops" => "hermes-sdlc-operations".into(),
            _ => format!("hermes-sdlc-{suffix}"),
        };
        routes[*role] = json!({"agent_id":Uuid::new_v4(),"fleet_config_revision":1,
            "package_commit":domain::sdlc_routing::PACKAGE_COMMIT,"package_manifest_sha256":"a".repeat(64),
            "namespace_id":(i+1).to_string(),"namespace_name":format!("hermes-{suffix}"),
            "workflow_id":(i+11).to_string(),"workflow_key":format!("hermes-sdlc:{role}"),"profile":profile,
            "workflow_catalog_version":3,"workflow_catalog_sha256":"b".repeat(64)});
    }
    parse(routes)
}

fn delivery_fence(receipt: &LifecycleReceipt) -> DeliveryFence {
    let assignment = receipt.assignment.as_ref().unwrap();
    DeliveryFence {
        assignment_id: assignment.assignment_id,
        execution_id: assignment.execution_id,
        fencing_token: assignment.fencing_token,
        lease_id: assignment.lease_id,
        expected_lease_version: receipt.lease.as_ref().unwrap().version,
    }
}

async fn count(db: &DatabaseConnection, table: &str) -> i64 {
    db.query_one(statement(
        &format!("SELECT count(*) AS n FROM {table}"),
        vec![],
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "n")
    .unwrap()
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn canonical_projection_heartbeat_race_terminal_recovery_and_disabled_replay() {
    let url = std::env::var("TT_LIFECYCLE_TEST_DATABASE_URL").expect("isolated database required");
    let db = Database::connect(&url).await.unwrap();
    let row = db.query_one(statement("SELECT current_database() AS name,to_regclass('public.seaql_migrations') IS NULL AS fresh",vec![])).await.unwrap().unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_lifecycle_test")
    );
    assert!(
        row.try_get::<bool>("", "fresh").unwrap(),
        "use a fresh disposable database"
    );
    migration::Migrator::up(&db, None).await.unwrap();
    let row = db
        .query_one(statement(
            "SELECT to_regclass('public.sdlc_delivery_heads') IS NULL AS absent",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap();
    if row.try_get::<bool>("", "absent").unwrap() {
        db.execute_unprepared(include_str!(
            "../../migration/src/single_task_lifecycle.sql"
        ))
        .await
        .unwrap();
    }
    let owner_id = Uuid::new_v4();
    let project = Uuid::new_v4();
    for (subject, id) in [
        ("owner", owner_id),
        ("pm", Uuid::new_v4()),
        ("scheduler", Uuid::new_v4()),
        ("verifier", Uuid::new_v4()),
        ("producer", Uuid::new_v4()),
    ] {
        exec(&db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,$3,'!',$3,true)",
            vec![id.into(),format!("{subject}@example.test").into(),subject.into()]).await.unwrap();
    }
    exec(&db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'LC','Lifecycle',$2,$3)",vec![project.into(),owner_id.into(),Uuid::new_v4().into()]).await.unwrap();
    exec(&db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Lifecycle','[]' FROM projects WHERE id=$1",vec![project.into()]).await.unwrap();
    exec(&db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub<>'owner'",vec![project.into()]).await.unwrap();
    let mut repo = PostgresSdlcRepository::connect(
        &url,
        SdlcConfig {
            instance_id: "lifecycle-test".into(),
            orchestrator_subject: "pm".into(),
            verifier_subject: "verifier".into(),
            reservation_scheduler_subject: "scheduler".into(),
            single_task_project_ids: [project].into_iter().collect(),
        },
    )
    .await
    .unwrap();
    let owner = actor("owner", true, vec![]);
    let scheduler = actor("scheduler", false, vec![]);
    let mut reader = scheduler.clone();
    reader.scopes = ["task-tracker:read".to_owned()].into_iter().collect();
    let routes = draft_flow_tests::single_task_routes();
    repo.set_routing_policy(
        project,
        &owner,
        SetRoutingPolicy {
            expected_version: None,
            routes: routes.clone(),
            idempotency_key: "policy".into(),
        },
    )
    .await
    .unwrap();
    let (draft, _) = repo
        .create_draft(
            project,
            &owner,
            CreateDraftCommand {
                flow: Some(DraftFlow::SingleTaskV1),
                title: "Lifecycle fixture".into(),
                description: "Synthetic acceptance fixture".into(),
                idempotency_key: "draft".into(),
            },
        )
        .await
        .unwrap();
    let task = draft.task_id;
    let (reservation,_) = repo.reserve_pm_draft(task,&owner,parse(json!({"requested_agent_id":routes.project_manager.agent_id,
        "expected_owner_version":0,"expected_assignment_version":null,"idempotency_key":"pm-reserve"}))).await.unwrap();
    let pm = actor(
        "pm",
        false,
        vec![
            "task-tracker:read".into(),
            reservation.assignment.scope(task),
        ],
    );
    let pm_fence: MachineFence = parse(
        json!({"assignment_id":reservation.assignment.assignment_id,
        "execution_id":reservation.assignment.execution_id,"agent_id":reservation.assignment.agent_id,"assignment_version":1}),
    );
    let (lease, _) = repo
        .claim_execution_lease(
            task,
            &pm,
            ClaimExecutionLease {
                expected_owner_version: 1,
                fence: pm_fence.clone(),
                idempotency_key: "pm-claim".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        repo.read(task, &owner).await.unwrap().status,
        Some(LifecycleStatus::InProgress)
    );
    let document: RequirementsDocument = parse(
        json!({"goal":"Fixture","scope":["Scope"],"exclusions":[],"scenarios":["Scenario"],
        "acceptance_criteria":["Acceptance"],"constraints":[],"dependencies":[],"assumptions":[],"checklist":["check"],"prerequisites":["prerequisite"]}),
    );
    let hash = app::sdlc::canonical_hash(&document).unwrap();
    repo.execute_verified(
        task,
        &pm,
        SdlcCommand::PublishRevision(PublishRevision {
            fence: pm_fence.clone(),
            expected_requirement_revision: None,
            document,
            idempotency_key: "publish".into(),
        }),
        reservation.clone(),
        None,
    )
    .await
    .unwrap();
    let verifier = actor(
        "verifier",
        false,
        vec![format!("task-tracker:sdlc:evidence:{task}:1")],
    );
    for check in ["check", "prerequisite"] {
        repo.execute_verified(
            task,
            &verifier,
            SdlcCommand::Evidence(EvidenceCommand {
                fence: pm_fence.clone(),
                requirement_revision: 1,
                content_hash: hash.clone(),
                check_id: check.into(),
                evidence_reference: format!("fixture://{check}"),
                idempotency_key: check.into(),
            }),
            reservation.clone(),
            None,
        )
        .await
        .unwrap();
    }
    assert!(matches!(
        repo.confirmation_read(task, &pm, 1).await,
        Err(AppError::NotFound(_))
    ));
    let confirmation = repo
        .execute_verified(
            task,
            &owner,
            SdlcCommand::Confirm {
                revision: 1,
                command: ConfirmCommand {
                    content_hash: hash.clone(),
                    idempotency_key: "confirm".into(),
                    expected_routing_policy_version: Some(1),
                },
            },
            reservation.clone(),
            None,
        )
        .await
        .unwrap();
    let before_receipt_reads = [
        count(&db, "sdlc_confirmations").await,
        count(&db, "sdlc_outbox").await,
        count(&db, "sdlc_pm_lease_operations").await,
        count(&db, "sdlc_idempotency").await,
    ];
    let mut pm_reader = pm.clone();
    pm_reader.scopes.remove("task-tracker:write");
    let read_confirmation = repo.confirmation_read(task, &pm_reader, 1).await.unwrap();
    assert_eq!(
        serde_json::to_value(&read_confirmation).unwrap(),
        confirmation
    );
    assert_eq!(read_confirmation.stage, Stage::Backlog);
    assert_eq!(
        repo.read(task, &owner).await.unwrap().stage,
        Stage::Analysis
    );
    assert!(matches!(
        repo.confirmation_read(task, &pm_reader, 2).await,
        Err(AppError::NotFound(_))
    ));
    exec(&db,"DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub='pm')",vec![project.into()]).await.unwrap();
    assert!(matches!(
        repo.confirmation_read(task, &pm_reader, 1).await,
        Err(AppError::Forbidden)
    ));
    exec(&db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub='pm'",vec![project.into()]).await.unwrap();
    assert_eq!(
        [
            count(&db, "sdlc_confirmations").await,
            count(&db, "sdlc_outbox").await,
            count(&db, "sdlc_pm_lease_operations").await,
            count(&db, "sdlc_idempotency").await
        ],
        before_receipt_reads
    );
    repo.complete_pm_draft(
        task,
        &scheduler,
        CompletePmDraft {
            contract_version: 1,
            fence: pm_fence,
            lease_id: lease.lease.lease_id,
            lease_version: 1,
            requirement_revision: 1,
            content_hash: hash.clone(),
            terminal_receipt_ref: "fixture://pm-terminal".into(),
            terminal_receipt_sha256: "a".repeat(64),
            workflow_receipt_ref: "fixture://pm-workflow".into(),
            workflow_receipt_sha256: "b".repeat(64),
            idempotency_key: "pm-complete".into(),
        },
    )
    .await
    .unwrap();
    let ensure = LifecycleCommand::EnsureQueued {
        idempotency_key: "queue".into(),
    };
    let (queued, _) = repo
        .lifecycle_command(task, &scheduler, ensure.clone())
        .await
        .unwrap();
    let tx = db.begin().await.unwrap();
    assert_eq!(
        mapped_issue_status(&tx, Stage::Development, LifecycleStatus::Ready)
            .await
            .unwrap(),
        mapped_issue_status(&tx, Stage::Review, LifecycleStatus::Ready)
            .await
            .unwrap()
    );
    assert_eq!(count(&db, "sdlc_issue_status_mapping").await, 27);
    tx.rollback().await.unwrap();
    let reserve = LifecycleCommand::Reserve {
        expected_task_version: queued.task.version,
        idempotency_key: "reserve-0".into(),
    };
    let (reserved, _) = repo
        .lifecycle_command(task, &scheduler, reserve.clone())
        .await
        .unwrap();
    let stale = delivery_fence(&reserved);
    let binding = RunBinding {
        chat_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        machine_subject: "producer".into(),
        dispatch_receipt: artifact("dispatch"),
    };
    db.execute_unprepared("CREATE FUNCTION fixture_fail_lifecycle_outbox() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        IF NEW.event_type='delivery.run_bound' THEN RAISE EXCEPTION 'fixture outbox failure'; END IF; RETURN NEW; END $$;
        CREATE TRIGGER fixture_fail_lifecycle_outbox BEFORE INSERT ON sdlc_outbox
        FOR EACH ROW EXECUTE FUNCTION fixture_fail_lifecycle_outbox();").await.unwrap();
    assert!(
        repo.lifecycle_command(
            task,
            &scheduler,
            LifecycleCommand::BindRun {
                fence: stale.clone(),
                binding: binding.clone(),
                idempotency_key: "bind-0".into(),
            }
        )
        .await
        .is_err()
    );
    assert_eq!(count(&db, "sdlc_delivery_run_bindings").await, 0);
    assert_eq!(count(&db, "sdlc_delivery_holds").await, 1);
    assert_eq!(
        repo.lifecycle_read(task, &reader)
            .await
            .unwrap()
            .receipt
            .task
            .runtime,
        RuntimeState::Reserved
    );
    db.execute_unprepared("DROP TRIGGER fixture_fail_lifecycle_outbox ON sdlc_outbox; DROP FUNCTION fixture_fail_lifecycle_outbox();").await.unwrap();
    repo.lifecycle_command(
        task,
        &scheduler,
        LifecycleCommand::BindRun {
            fence: stale.clone(),
            binding: binding.clone(),
            idempotency_key: "bind-0".into(),
        },
    )
    .await
    .unwrap();
    let heartbeat = LifecycleCommand::Heartbeat {
        fence: stale.clone(),
        idempotency_key: "heartbeat-0".into(),
    };
    let outbox_before_heartbeat = count(&db, "sdlc_outbox").await;
    let operations_before_heartbeat = count(&db, "sdlc_delivery_operations").await;
    let mirror_source_before_heartbeat: Vec<Json> = repo
        .metadata_outbox(task, &owner, 0, 100)
        .await
        .unwrap()
        .into_iter()
        .map(|candidate| serde_json::to_value(candidate.event.unwrap()).unwrap())
        .collect();
    let (renewed, _) = repo
        .lifecycle_command(task, &scheduler, heartbeat.clone())
        .await
        .unwrap();
    assert_eq!(renewed.lease.as_ref().unwrap().version, 2);
    assert_eq!(count(&db, "sdlc_outbox").await, outbox_before_heartbeat);
    assert_eq!(
        count(&db, "sdlc_delivery_operations").await,
        operations_before_heartbeat + 1
    );
    let mirror_source_after_heartbeat: Vec<Json> = repo
        .metadata_outbox(task, &owner, 0, 100)
        .await
        .unwrap()
        .into_iter()
        .map(|candidate| serde_json::to_value(candidate.event.unwrap()).unwrap())
        .collect();
    assert_eq!(
        mirror_source_after_heartbeat,
        mirror_source_before_heartbeat
    );
    let heartbeat_audit = db.query_one(statement(
        "SELECT event_id,result FROM sdlc_delivery_operations WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key='heartbeat-0'",
        vec![task.into(),scheduler.subject.clone().into()],
    )).await.unwrap().unwrap();
    assert!(
        heartbeat_audit
            .try_get::<Option<Uuid>>("", "event_id")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        heartbeat_audit.try_get::<Json>("", "result").unwrap(),
        serde_json::to_value(&renewed).unwrap()
    );
    let fresh_after_heartbeat = repo.lifecycle_read(task, &reader).await.unwrap();
    assert_eq!(
        serde_json::to_value(&fresh_after_heartbeat.receipt).unwrap(),
        serde_json::to_value(&renewed).unwrap()
    );
    assert!(fresh_after_heartbeat.observed_at >= renewed.lease.as_ref().unwrap().heartbeat_at);
    assert!(fresh_after_heartbeat.observed_at < renewed.lease.as_ref().unwrap().expires_at);
    assert!(
        repo.lifecycle_command(
            task,
            &scheduler,
            LifecycleCommand::Heartbeat {
                fence: stale.clone(),
                idempotency_key: "stale-heartbeat".into()
            }
        )
        .await
        .is_err()
    );
    let producer = actor(
        "producer",
        false,
        vec![
            "task-tracker:read".into(),
            reserved.assignment.as_ref().unwrap().scope(),
        ],
    );
    let output = DeliveryResult::Analyst {
        analysis: artifact("analysis"),
    };
    let proposal_command = SubmitDeliveryProposal {
        fence: stale.clone(),
        workflow_receipt: artifact("workflow"),
        result: output.clone(),
        idempotency_key: "proposal-0".into(),
    };
    let (concurrent_heartbeat, concurrent_proposal) = tokio::join!(
        repo.lifecycle_command(
            task,
            &scheduler,
            LifecycleCommand::Heartbeat {
                fence: delivery_fence(&renewed),
                idempotency_key: "concurrent-heartbeat".into(),
            }
        ),
        repo.submit_delivery_proposal(task, &producer, proposal_command.clone())
    );
    assert_eq!(
        concurrent_heartbeat
            .unwrap()
            .0
            .lease
            .as_ref()
            .unwrap()
            .version,
        3
    );
    let (proposal, _) = concurrent_proposal.unwrap();
    // The concurrent proposal remains a business event; the second heartbeat does not.
    assert_eq!(count(&db, "sdlc_outbox").await, outbox_before_heartbeat + 1);
    let operations_after_race = count(&db, "sdlc_delivery_operations").await;
    let original_heartbeat = repo
        .lifecycle_command(task, &scheduler, heartbeat)
        .await
        .unwrap();
    assert!(original_heartbeat.1);
    assert_eq!(original_heartbeat.0.lease.as_ref().unwrap().version, 2);
    assert_eq!(
        serde_json::to_value(&original_heartbeat.0).unwrap(),
        serde_json::to_value(&renewed).unwrap()
    );
    assert_eq!(
        count(&db, "sdlc_delivery_operations").await,
        operations_after_race
    );
    assert_eq!(count(&db, "sdlc_outbox").await, outbox_before_heartbeat + 1);
    assert_eq!(
        repo.lifecycle_operation(task, &reader, "heartbeat-0")
            .await
            .unwrap()
            .lease
            .as_ref()
            .unwrap()
            .version,
        2
    );
    assert!(
        repo.lifecycle_operation(task, &scheduler, "heartbeat-0")
            .await
            .is_err()
    );
    assert_eq!(
        repo.lifecycle_read(task, &reader)
            .await
            .unwrap()
            .receipt
            .lease
            .as_ref()
            .unwrap()
            .version,
        3
    );
    assert_eq!(
        proposal.output_sha256,
        app::sdlc::canonical_hash(&output).unwrap()
    );
    assert!(
        repo.submit_delivery_proposal(task, &producer, proposal_command)
            .await
            .unwrap()
            .1
    );
    assert!(
        db.execute_unprepared("DELETE FROM sdlc_delivery_holds")
            .await
            .is_err()
    );
    // Exercise actual server-clock expiry without disabling guards or mutating protected ledgers.
    db.execute_unprepared("SELECT pg_sleep(31)").await.unwrap();
    let lease_operations_before_receipt = count(&db, "sdlc_pm_lease_operations").await;
    let outbox_before_receipt = count(&db, "sdlc_outbox").await;
    assert_eq!(
        repo.confirmation_read(task, &pm_reader, 1).await.unwrap(),
        read_confirmation
    );
    assert_eq!(
        count(&db, "sdlc_pm_lease_operations").await,
        lease_operations_before_receipt
    );
    assert_eq!(count(&db, "sdlc_outbox").await, outbox_before_receipt);
    let fresh = repo.lifecycle_read(task, &reader).await.unwrap();
    assert_eq!(fresh.runtime_state, RuntimeState::Unknown);
    assert_eq!(fresh.receipt.task.runtime, RuntimeState::Running);
    let terminal = TerminalReceipt {
        chat_id: binding.chat_id,
        run_id: binding.run_id,
        fleet_receipt: artifact("terminal"),
        workflow_receipt: proposal.workflow_receipt,
        output_sha256: proposal.output_sha256,
    };
    assert!(
        repo.lifecycle_command(
            task,
            &scheduler,
            LifecycleCommand::Complete {
                fence: stale.clone(),
                terminal,
                result: output,
                idempotency_key: "expired-complete".into()
            }
        )
        .await
        .is_err()
    );
    let reconcile = LifecycleCommand::Reconcile {
        fence: stale,
        terminal: RuntimeTerminalObservation {
            assignment_hash: reserved
                .assignment
                .as_ref()
                .unwrap()
                .assignment_hash
                .clone(),
            chat_id: binding.chat_id,
            run_id: binding.run_id,
            state: RuntimeTerminalState::Failed,
            receipt: artifact("verified-terminal"),
        },
        recovery: RecoveryDisposition::RetryTransient {
            replay_safety_receipt: artifact("safe-replay"),
        },
        idempotency_key: "reconcile".into(),
    };
    let (recovered, _) = repo
        .lifecycle_command(task, &scheduler, reconcile.clone())
        .await
        .unwrap();
    assert_eq!(recovered.task.stage, LifecycleStage::Analysis);
    assert!(recovered.result.is_none());
    assert_eq!(count(&db, "sdlc_delivery_results").await, 0);
    assert_eq!(count(&db, "sdlc_delivery_holds").await, 0);
    assert!(
        repo.lifecycle_command(task, &scheduler, reconcile)
            .await
            .unwrap()
            .1
    );
    let (retry, _) = repo
        .lifecycle_command(
            task,
            &scheduler,
            LifecycleCommand::Reserve {
                expected_task_version: recovered.task.version,
                idempotency_key: "retry".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(retry.assignment.as_ref().unwrap().attempt, 2);
    assert!(
        retry.assignment.as_ref().unwrap().fencing_token
            > reserved.assignment.as_ref().unwrap().fencing_token
    );
    assert_eq!(count(&db, "sdlc_delivery_assignments").await, 2);
    assert!(repo.lifecycle_read(task, &producer).await.is_err());
    let mut wrong_chat = binding.clone();
    wrong_chat.chat_id = Uuid::new_v4();
    wrong_chat.run_id = Uuid::new_v4();
    assert!(
        repo.lifecycle_command(
            task,
            &scheduler,
            LifecycleCommand::BindRun {
                fence: delivery_fence(&retry),
                binding: wrong_chat,
                idempotency_key: "wrong-chat".into()
            }
        )
        .await
        .is_err()
    );
    repo.config.single_task_project_ids.clear();
    assert!(
        repo.lifecycle_command(task, &scheduler, ensure)
            .await
            .unwrap()
            .1
    );
    assert_eq!(
        serde_json::to_value(
            repo.lifecycle_command(task, &scheduler, reserve)
                .await
                .unwrap()
                .0
        )
        .unwrap(),
        serde_json::to_value(reserved).unwrap()
    );
    assert!(matches!(
        repo.lifecycle_command(
            task,
            &scheduler,
            LifecycleCommand::EnsureQueued {
                idempotency_key: "disabled-new".into()
            }
        )
        .await,
        Err(AppError::Unavailable(_))
    ));
    repo.lifecycle_read(task, &reader).await.unwrap();
    repo.lifecycle_command(
        task,
        &scheduler,
        LifecycleCommand::Heartbeat {
            fence: delivery_fence(&retry),
            idempotency_key: "disabled-heartbeat".into(),
        },
    )
    .await
    .unwrap();
    let mut cursor = repo.lifecycle_read(task, &reader).await.unwrap().receipt;
    let mut ready_ids = Vec::new();
    let outputs = [
        DeliveryResult::Analyst {
            analysis: artifact("retry-analysis"),
        },
        DeliveryResult::Architect {
            design: artifact("design"),
        },
        DeliveryResult::Developer {
            commit_sha: "b".repeat(40),
            changes: artifact("changes"),
            checks: vec![artifact("checks")],
        },
        DeliveryResult::Reviewer {
            verdict: ReviewVerdict::Approved,
            report: artifact("review"),
        },
        DeliveryResult::Tester {
            verdict: TestVerdict::Passed,
            report: artifact("tests"),
        },
        DeliveryResult::DevOps {
            deployment: artifact("deployment"),
            verification: artifact("verification"),
        },
    ];
    for (i, result) in outputs.into_iter().enumerate() {
        if cursor.assignment.is_none() {
            let command = LifecycleCommand::Reserve {
                expected_task_version: cursor.task.version,
                idempotency_key: format!("role-{i}-reserve"),
            };
            if i == 1 {
                assert!(matches!(
                    repo.lifecycle_command(task, &scheduler, command.clone())
                        .await,
                    Err(AppError::Unavailable(_))
                ));
                repo.config.single_task_project_ids.insert(project);
            }
            cursor = repo
                .lifecycle_command(task, &scheduler, command)
                .await
                .unwrap()
                .0;
        }
        let fence = delivery_fence(&cursor);
        let role_binding = RunBinding {
            chat_id: cursor.task.resume_chat_id.unwrap_or_else(Uuid::new_v4),
            run_id: Uuid::new_v4(),
            machine_subject: "producer".into(),
            dispatch_receipt: artifact("role-dispatch"),
        };
        repo.lifecycle_command(
            task,
            &scheduler,
            LifecycleCommand::BindRun {
                fence: fence.clone(),
                binding: role_binding.clone(),
                idempotency_key: format!("role-{i}-bind"),
            },
        )
        .await
        .unwrap();
        let role_producer = actor(
            "producer",
            false,
            vec![
                "task-tracker:read".into(),
                cursor.assignment.as_ref().unwrap().scope(),
            ],
        );
        let (proposal, _) = repo
            .submit_delivery_proposal(
                task,
                &role_producer,
                SubmitDeliveryProposal {
                    fence: fence.clone(),
                    workflow_receipt: artifact("role-workflow"),
                    result: result.clone(),
                    idempotency_key: format!("role-{i}-proposal"),
                },
            )
            .await
            .unwrap();
        cursor = repo
            .lifecycle_command(
                task,
                &scheduler,
                LifecycleCommand::Complete {
                    fence,
                    terminal: TerminalReceipt {
                        chat_id: role_binding.chat_id,
                        run_id: role_binding.run_id,
                        fleet_receipt: artifact("role-terminal"),
                        workflow_receipt: proposal.workflow_receipt,
                        output_sha256: proposal.output_sha256,
                    },
                    result,
                    idempotency_key: format!("role-{i}-complete"),
                },
            )
            .await
            .unwrap()
            .0;
        let canonical = repo.read(task, &owner).await.unwrap();
        assert_eq!(canonical.task_id, task);
        assert_eq!(canonical.stage, cursor.task.stage.canonical());
        assert_eq!(canonical.status, Some(cursor.task.status));
        if matches!(canonical.stage, Stage::Development | Stage::Review) {
            let id: Uuid = db
                .query_one(statement(
                    "SELECT status_id FROM issues WHERE id=$1",
                    vec![task.into()],
                ))
                .await
                .unwrap()
                .unwrap()
                .try_get("", "status_id")
                .unwrap();
            ready_ids.push((canonical.stage, id));
        }
    }
    assert_eq!(ready_ids.len(), 2);
    assert_ne!(ready_ids[0].0, ready_ids[1].0);
    assert_eq!(ready_ids[0].1, ready_ids[1].1);
    assert_eq!(cursor.task.status, LifecycleStatus::Completed);
    assert_eq!(count(&db, "sdlc_delivery_results").await, 6);
    assert_eq!(count(&db, "sdlc_delivery_holds").await, 0);
    for candidate in repo.metadata_outbox(task, &owner, 0, 100).await.unwrap() {
        let event = candidate
            .event
            .expect("new events must have metadata projection");
        let wire = serde_json::to_string(&event).unwrap();
        assert!(!wire.contains("fixture://"));
        assert!(!wire.contains("Synthetic acceptance fixture"));
    }
}
