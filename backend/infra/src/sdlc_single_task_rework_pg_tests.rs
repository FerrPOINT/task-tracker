//! Child of lifecycle::regression_tests; main owns registration and PostgreSQL QA.
//! Real repository transactions, synthetic role artifacts and trusted terminal receipts.
//! No live Fleet/Workflow execution or attachment-byte verification is claimed.
use super::*;

struct ReworkFixture {
    db: DatabaseConnection,
    repo: PostgresSdlcRepository,
    task: Uuid,
    owner: Principal,
    scheduler: Principal,
    reader: Principal,
    frozen: TaskRoutingSnapshot,
    pm_history: Json,
}

struct OriginalStep {
    reserve: LifecycleCommand,
    reserved: LifecycleReceipt,
    complete: LifecycleCommand,
    completed: LifecycleReceipt,
    producer: Principal,
    proposal_command: SubmitDeliveryProposal,
    proposal: Json,
}

async fn pm_history(db: &DatabaseConnection, task: Uuid) -> Json {
    db.query_one(statement(
        "SELECT jsonb_build_object('assignment',t.state->'assignment',
         'requirements',t.state->'revisions','confirmations',t.state->'confirmations','evidence',t.state->'evidence',
         'pm_owner_version',t.pm_owner_version,'pm_execution_id',t.pm_execution_id,'pm_admission_state',t.pm_admission_state,
         'creation',(SELECT to_jsonb(c) FROM sdlc_draft_creations c WHERE c.task_id=t.task_id),
         'assignments',(SELECT jsonb_agg(to_jsonb(a) ORDER BY version) FROM sdlc_assignments a WHERE a.task_id=t.task_id),
         'executions',(SELECT jsonb_agg(to_jsonb(e) ORDER BY ordinal) FROM sdlc_pm_executions e WHERE e.task_id=t.task_id),
         'leases',(SELECT jsonb_agg(to_jsonb(l) ORDER BY l.execution_id) FROM sdlc_pm_execution_leases l JOIN sdlc_pm_executions e USING(execution_id) WHERE e.task_id=t.task_id),
         'completions',(SELECT jsonb_agg(to_jsonb(c) ORDER BY completion_id) FROM sdlc_pm_completions c WHERE c.task_id=t.task_id),
         'routing',(SELECT to_jsonb(s) FROM sdlc_task_routing_snapshots s WHERE s.task_id=t.task_id),
         'intent',(SELECT jsonb_agg(to_jsonb(i) ORDER BY intent_id) FROM sdlc_analysis_intents i WHERE i.task_id=t.task_id)) AS snapshot
         FROM sdlc_tasks t WHERE task_id=$1",
        vec![task.into()],
    )).await.unwrap().unwrap().try_get("", "snapshot").unwrap()
}

async fn complete_initial_pm(
    repo: &PostgresSdlcRepository,
    task: Uuid,
    owner: &Principal,
    scheduler: &Principal,
    agent: Uuid,
) {
    let (reservation, replayed) = repo
        .reserve_pm_draft(
            task,
            owner,
            parse(json!({
        "requested_agent_id":agent,"expected_owner_version":0,"expected_assignment_version":null,
        "idempotency_key":"rework:pm:reserve"})),
        )
        .await
        .unwrap();
    assert!(!replayed);
    let pm = actor(
        "pm",
        false,
        vec![
            "task-tracker:read".into(),
            reservation.assignment.scope(task),
        ],
    );
    let fence = MachineFence {
        assignment_id: reservation.assignment.assignment_id,
        execution_id: reservation.assignment.execution_id,
        agent_id: agent,
        assignment_version: reservation.assignment.version,
    };
    let (lease, replayed) = repo
        .claim_execution_lease(
            task,
            &pm,
            ClaimExecutionLease {
                expected_owner_version: reservation.owner_cas.version,
                fence: fence.clone(),
                idempotency_key: "rework:pm:claim".into(),
            },
        )
        .await
        .unwrap();
    assert!(!replayed);
    let document: RequirementsDocument = parse(
        json!({"goal":"One task with actual persisted rework",
        "scope":["One root task"],"exclusions":["No children"],"scenarios":["Review and testing reject work"],
        "acceptance_criteria":["Rework repeats review and testing before deployment"],"constraints":[],
        "dependencies":[],"assumptions":[],"checklist":["check"],"prerequisites":["prerequisite"]}),
    );
    let hash = app::sdlc::canonical_hash(&document).unwrap();
    repo.execute_verified(
        task,
        &pm,
        SdlcCommand::PublishRevision(PublishRevision {
            fence: fence.clone(),
            expected_requirement_revision: None,
            document,
            idempotency_key: "rework:pm:requirements".into(),
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
                fence: fence.clone(),
                requirement_revision: 1,
                content_hash: hash.clone(),
                check_id: check.into(),
                evidence_reference: format!("fixture://rework-readiness-{check}"),
                idempotency_key: format!("rework:pm:evidence:{check}"),
            }),
            reservation.clone(),
            None,
        )
        .await
        .unwrap();
    }
    let confirmation = repo
        .execute_verified(
            task,
            owner,
            SdlcCommand::Confirm {
                revision: 1,
                command: ConfirmCommand {
                    content_hash: hash.clone(),
                    idempotency_key: "rework:pm:confirm".into(),
                    expected_routing_policy_version: Some(1),
                },
            },
            reservation,
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        repo.confirmation_read(task, &pm, 1)
            .await
            .unwrap()
            .id
            .to_string(),
        confirmation["id"].as_str().unwrap()
    );
    // Consent alone cannot allocate or queue the first technical assignment.
    assert!(matches!(
        repo.lifecycle_command(
            task,
            scheduler,
            LifecycleCommand::EnsureQueued {
                idempotency_key: "rework:before-pm-complete".into(),
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let (completed, replayed) = repo
        .complete_pm_draft(
            task,
            scheduler,
            CompletePmDraft {
                contract_version: 1,
                fence,
                lease_id: lease.lease.lease_id,
                lease_version: lease.lease.version,
                requirement_revision: 1,
                content_hash: hash,
                terminal_receipt_ref: "fixture://rework-pm-terminal".into(),
                terminal_receipt_sha256: "a".repeat(64),
                workflow_receipt_ref: "fixture://rework-pm-workflow".into(),
                workflow_receipt_sha256: "b".repeat(64),
                idempotency_key: "rework:pm:complete".into(),
            },
        )
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(completed.fence.execution_id, lease.fence.execution_id);
    crate::sdlc::seed_analysis_fixture(repo.db.raw(), task, owner).await;
}

impl ReworkFixture {
    async fn new() -> (Self, LifecycleReceipt) {
        let url = std::env::var("TT_LIFECYCLE_TEST_DATABASE_URL")
            .expect("fresh isolated rework database required");
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
                vec![id.into(),format!("rework-{subject}@example.test").into(),subject.into()]).await.unwrap();
        }
        exec(&db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'RW','One-task rework',$2,$3)",
            vec![project.into(),owner_id.into(),Uuid::new_v4().into()]).await.unwrap();
        exec(&db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Rework','[]' FROM projects WHERE id=$1",
            vec![project.into()]).await.unwrap();
        exec(&db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub<>'owner'",
            vec![project.into()]).await.unwrap();
        let repo = PostgresSdlcRepository::connect(
            &url,
            SdlcConfig {
                instance_id: "single-task-rework-test".into(),
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
                idempotency_key: "rework:policy:1".into(),
            },
        )
        .await
        .unwrap();
        let (draft, replayed) = repo
            .create_draft(
                project,
                &owner,
                CreateDraftCommand {
                    flow: Some(DraftFlow::SingleTaskV1),
                    title: "One actual Tracker task".into(),
                    description: "Synthetic persisted Reviewer and Tester rework fixture".into(),
                    idempotency_key: "rework:draft".into(),
                },
            )
            .await
            .unwrap();
        assert!(!replayed);
        let task = draft.task_id;
        complete_initial_pm(
            &repo,
            task,
            &owner,
            &scheduler,
            routes.project_manager.agent_id,
        )
        .await;
        let (queued, replayed) = repo
            .lifecycle_command(
                task,
                &scheduler,
                LifecycleCommand::EnsureQueued {
                    idempotency_key: "rework:queue".into(),
                },
            )
            .await
            .unwrap();
        assert!(!replayed);
        assert_eq!(queued.task.stage, LifecycleStage::Analysis);
        let frozen = queued.task.routing.clone();
        assert_eq!(frozen.policy.version, 1);
        // A later project policy must never select agents or pins for this frozen task.
        let mut newer = serde_json::to_value(&routes).unwrap();
        for (role, _) in routes.entries() {
            newer[role]["agent_id"] = json!(Uuid::new_v4());
            newer[role]["fleet_config_revision"] = json!(2);
        }
        let (new_policy, replayed) = repo
            .set_routing_policy(
                project,
                &owner,
                SetRoutingPolicy {
                    expected_version: Some(1),
                    routes: parse(newer),
                    idempotency_key: "rework:policy:2".into(),
                },
            )
            .await
            .unwrap();
        assert!(!replayed);
        assert_eq!(new_policy.version, 2);
        assert_ne!(new_policy.routing_hash, frozen.policy.routing_hash);
        let pm_history = pm_history(&db, task).await;
        assert_eq!(pm_history["pm_admission_state"], json!("completed"));
        assert_eq!(pm_history["completions"].as_array().unwrap().len(), 1);
        (
            Self {
                db,
                repo,
                task,
                owner,
                scheduler,
                reader,
                frozen,
                pm_history,
            },
            queued,
        )
    }

    async fn assert_current(&self, expected: &LifecycleReceipt) {
        let read = self
            .repo
            .lifecycle_read(self.task, &self.reader)
            .await
            .unwrap();
        assert_eq!(&read.receipt, expected);
        assert_eq!(read.runtime_state, expected.task.runtime);
        assert_eq!(expected.task.task_id, self.task);
        assert_eq!(expected.task.routing, self.frozen);
        assert!(expected.task.cycle <= MAX_REWORK_CYCLES);
        let canonical = self.repo.read(self.task, &self.owner).await.unwrap();
        assert_eq!(canonical.task_id, self.task);
        assert_eq!(canonical.root_task_id, self.task);
        assert_eq!(canonical.stage, expected.task.stage.canonical());
        assert_eq!(canonical.status, Some(expected.task.status));
        assert_eq!(
            canonical.retained_fields["lifecycle"],
            serde_json::to_value(&expected.task).unwrap()
        );
        let tx = self.db.begin().await.unwrap();
        let mapped =
            mapped_issue_status(&tx, expected.task.stage.canonical(), expected.task.status)
                .await
                .unwrap();
        let actual: Uuid = tx
            .query_one(statement(
                "SELECT status_id FROM issues WHERE id=$1",
                vec![self.task.into()],
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "status_id")
            .unwrap();
        assert_eq!(actual, mapped);
        tx.rollback().await.unwrap();
        assert_eq!(count(&self.db, "issues").await, 1);
        assert_eq!(count(&self.db, "sdlc_tasks").await, 1);
        assert_eq!(pm_history(&self.db, self.task).await, self.pm_history);
    }

    async fn step(
        &self,
        queued: &LifecycleReceipt,
        output: DeliveryResult,
        index: usize,
    ) -> OriginalStep {
        assert_eq!(queued.task.status, LifecycleStatus::Ready);
        assert_eq!(queued.task.runtime, RuntimeState::Queued);
        assert!(queued.assignment.is_none());
        assert!(queued.lease.is_none());
        assert!(queued.binding.is_none());
        self.assert_current(queued).await;
        let reserve = LifecycleCommand::Reserve {
            expected_task_version: queued.task.version,
            idempotency_key: format!("rework:{index}:reserve"),
        };
        let (reserved, replayed) = self
            .repo
            .lifecycle_command(self.task, &self.scheduler, reserve.clone())
            .await
            .unwrap();
        assert!(!replayed);
        assert_eq!(reserved.task.version, queued.task.version + 1);
        assert_eq!(reserved.task.status, LifecycleStatus::Ready);
        assert_eq!(reserved.task.runtime, RuntimeState::Reserved);
        assert_eq!(reserved.task.stage, queued.task.stage);
        assert_eq!(reserved.task.cycle, queued.task.cycle);
        assert!(reserved.result.is_none());
        let assignment = reserved.assignment.as_ref().unwrap();
        assert_eq!(assignment.task_id, self.task);
        assert_eq!(assignment.role, output.role());
        assert_eq!(assignment.stage, queued.task.stage);
        assert_eq!(assignment.cycle, queued.task.cycle);
        assert_eq!(assignment.routing, self.frozen);
        assert_eq!(
            assignment.previous_result_id,
            queued.task.previous_result_id
        );
        assert_eq!(
            assignment.assignment_hash,
            app::sdlc_lifecycle::assignment_hash(assignment).unwrap()
        );
        assert_eq!(assignment.attempt, 1);
        assignment.execution.as_ref().unwrap().validate().unwrap();
        let route = self
            .frozen
            .policy
            .routes
            .entries()
            .into_iter()
            .find(|(role, _)| *role == output.role().key())
            .unwrap()
            .1;
        assert_eq!(assignment.agent_id, route.agent_id);
        assert_eq!(
            reserved.lease.as_ref().unwrap().holder_subject,
            self.scheduler.subject
        );
        assert_eq!(count(&self.db, "sdlc_delivery_holds").await, 1);
        self.assert_current(&reserved).await;
        let fence = delivery_fence(&reserved);
        let binding = RunBinding {
            chat_id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
            machine_subject: "producer".into(),
            dispatch_receipt: artifact(&format!("rework-{index}-dispatch")),
        };
        let (running, replayed) = self
            .repo
            .lifecycle_command(
                self.task,
                &self.scheduler,
                LifecycleCommand::BindRun {
                    fence: fence.clone(),
                    binding: binding.clone(),
                    idempotency_key: format!("rework:{index}:bind"),
                },
            )
            .await
            .unwrap();
        assert!(!replayed);
        assert_eq!(running.task.status, LifecycleStatus::InProgress);
        assert_eq!(running.task.runtime, RuntimeState::Running);
        assert_eq!(running.task.stage, queued.task.stage);
        assert_eq!(running.task.cycle, queued.task.cycle);
        assert_eq!(running.task.version, reserved.task.version + 1);
        self.assert_current(&running).await;
        let producer = actor(
            "producer",
            false,
            vec!["task-tracker:read".into(), assignment.scope()],
        );
        if let Some(previous) = &queued.result {
            assert_eq!(assignment.previous_result_id, Some(previous.result_id));
            let DeliveryInput::Delivery(actual) = self
                .repo
                .lifecycle_result(self.task, &producer, previous.result_id)
                .await
                .unwrap()
            else {
                panic!("expected accepted delivery predecessor");
            };
            assert_eq!(&actual, previous);
        }
        let proposal_command = SubmitDeliveryProposal {
            fence: fence.clone(),
            workflow_receipt: artifact(&format!("rework-{index}-workflow")),
            result: output.clone(),
            idempotency_key: format!("rework:{index}:proposal"),
        };
        let mut foreign = assignment.clone();
        foreign.assignment_id = Uuid::new_v4();
        let foreign_actor = actor(
            "producer",
            false,
            vec!["task-tracker:read".into(), foreign.scope()],
        );
        assert!(matches!(
            self.repo
                .submit_delivery_proposal(self.task, &foreign_actor, proposal_command.clone())
                .await,
            Err(AppError::Forbidden)
        ));
        let (proposal, replayed) = self
            .repo
            .submit_delivery_proposal(self.task, &producer, proposal_command.clone())
            .await
            .unwrap();
        assert!(!replayed);
        assert_eq!(proposal.assignment_id, assignment.assignment_id);
        assert_eq!(proposal.execution_id, assignment.execution_id);
        assert_eq!(
            proposal.output_sha256,
            app::sdlc::canonical_hash(&output).unwrap()
        );
        // Publishing a proposal cannot advance the stage, complete the run or release capacity.
        self.assert_current(&running).await;
        assert_eq!(count(&self.db, "sdlc_delivery_holds").await, 1);
        let complete = LifecycleCommand::Complete {
            fence,
            terminal: TerminalReceipt {
                chat_id: binding.chat_id,
                run_id: binding.run_id,
                fleet_receipt: artifact(&format!("rework-{index}-actual-terminal-fixture")),
                workflow_receipt: proposal.workflow_receipt.clone(),
                output_sha256: proposal.output_sha256.clone(),
            },
            result: output.clone(),
            idempotency_key: format!("rework:{index}:complete"),
        };
        assert!(matches!(
            self.repo
                .lifecycle_command(self.task, &producer, complete.clone())
                .await,
            Err(AppError::Forbidden)
        ));
        let (completed, replayed) = self
            .repo
            .lifecycle_command(self.task, &self.scheduler, complete.clone())
            .await
            .unwrap();
        assert!(!replayed);
        assert_eq!(completed.task.version, running.task.version + 1);
        assert_eq!(completed.task.routing, self.frozen);
        assert!(completed.assignment.is_none());
        assert!(completed.lease.is_none());
        assert!(completed.binding.is_none());
        assert_eq!(count(&self.db, "sdlc_delivery_holds").await, 0);
        let accepted = completed.result.as_ref().unwrap();
        assert_eq!(accepted.assignment_id, assignment.assignment_id);
        assert_eq!(accepted.task_id, self.task);
        assert_eq!(accepted.previous_stage, queued.task.stage);
        assert_eq!(accepted.next_stage, completed.task.stage);
        assert_eq!(accepted.next_status, completed.task.status);
        assert_eq!(accepted.cycle, completed.task.cycle);
        assert_eq!(accepted.output, output);
        assert_eq!(completed.task.previous_result_id, Some(accepted.result_id));
        self.assert_current(&completed).await;
        OriginalStep {
            reserve,
            reserved,
            complete,
            completed,
            producer,
            proposal_command,
            proposal: serde_json::to_value(proposal).unwrap(),
        }
    }

    async fn journal_snapshot(&self) -> Json {
        self.db.query_one(statement(
            "SELECT jsonb_build_object(
             'head',(SELECT to_jsonb(h) FROM sdlc_delivery_heads h WHERE task_id=$1),
             'task',(SELECT to_jsonb(t) FROM sdlc_tasks t WHERE task_id=$1),
             'issue',(SELECT to_jsonb(i) FROM issues i WHERE id=$1),
             'assignments',(SELECT jsonb_agg(to_jsonb(a) ORDER BY assignment_id) FROM sdlc_delivery_assignments a WHERE task_id=$1),
             'bindings',(SELECT jsonb_agg(to_jsonb(b) ORDER BY b.assignment_id) FROM sdlc_delivery_run_bindings b JOIN sdlc_delivery_assignments a USING(assignment_id) WHERE a.task_id=$1),
             'holds',(SELECT COALESCE(jsonb_agg(to_jsonb(h) ORDER BY assignment_id),'[]') FROM sdlc_delivery_holds h WHERE task_id=$1),
             'proposals',(SELECT jsonb_agg(to_jsonb(p) ORDER BY proposal_id) FROM sdlc_delivery_proposals p WHERE task_id=$1),
             'proposal_operations',(SELECT jsonb_agg(to_jsonb(p) ORDER BY idempotency_key) FROM sdlc_delivery_proposal_operations p WHERE task_id=$1),
             'results',(SELECT jsonb_agg(to_jsonb(r) ORDER BY result_id) FROM sdlc_delivery_results r WHERE task_id=$1),
             'operations',(SELECT jsonb_agg(to_jsonb(o) ORDER BY idempotency_key) FROM sdlc_delivery_operations o WHERE task_id=$1),
             'outbox',(SELECT jsonb_agg(to_jsonb(o) ORDER BY sequence) FROM sdlc_outbox o WHERE task_id=$1),
             'status_history',(SELECT jsonb_agg(to_jsonb(h) ORDER BY id) FROM issue_status_history h WHERE issue_id=$1)) AS snapshot",
            vec![self.task.into()],
        )).await.unwrap().unwrap().try_get("","snapshot").unwrap()
    }

    async fn assert_original_replays(&self, original: &OriginalStep) {
        let expected_result = original.completed.result.as_ref().unwrap();
        let DeliveryInput::Delivery(actual_result) = self
            .repo
            .lifecycle_result(self.task, &self.reader, expected_result.result_id)
            .await
            .unwrap()
        else {
            panic!("expected retained delivery result");
        };
        assert_eq!(&actual_result, expected_result);
        let (reserved, replayed) = self
            .repo
            .lifecycle_command(self.task, &self.scheduler, original.reserve.clone())
            .await
            .unwrap();
        assert!(replayed);
        assert_eq!(reserved, original.reserved);
        let (completed, replayed) = self
            .repo
            .lifecycle_command(self.task, &self.scheduler, original.complete.clone())
            .await
            .unwrap();
        assert!(replayed);
        assert_eq!(completed, original.completed);
        assert_eq!(
            self.repo
                .lifecycle_operation(self.task, &self.reader, original.complete.key())
                .await
                .unwrap(),
            original.completed
        );
        let (proposal, replayed) = self
            .repo
            .submit_delivery_proposal(
                self.task,
                &original.producer,
                original.proposal_command.clone(),
            )
            .await
            .unwrap();
        assert!(replayed);
        assert_eq!(serde_json::to_value(proposal).unwrap(), original.proposal);
        assert_eq!(
            serde_json::to_value(
                self.repo
                    .delivery_proposal_operation(
                        self.task,
                        &original.producer,
                        &original.proposal_command.idempotency_key
                    )
                    .await
                    .unwrap()
            )
            .unwrap(),
            original.proposal
        );
        let mut changed = original.proposal_command.clone();
        changed.workflow_receipt.sha256 = "c".repeat(64);
        assert!(matches!(
            self.repo
                .submit_delivery_proposal(self.task, &original.producer, changed)
                .await,
            Err(AppError::Conflict(_))
        ));
        let mut changed = original.complete.clone();
        let LifecycleCommand::Complete { terminal, .. } = &mut changed else {
            panic!("Complete fixture required");
        };
        terminal.fleet_receipt.sha256 = "d".repeat(64);
        assert!(matches!(
            self.repo
                .lifecycle_command(self.task, &self.scheduler, changed)
                .await,
            Err(AppError::Conflict(_))
        ));
    }
}

fn developer() -> DeliveryResult {
    DeliveryResult::Developer {
        commit_sha: "e".repeat(40),
        changes: artifact("rework-changes"),
        checks: vec![artifact("rework-checks")],
    }
}

fn reviewer(verdict: ReviewVerdict) -> DeliveryResult {
    DeliveryResult::Reviewer {
        verdict,
        report: artifact("rework-review-report"),
    }
}

fn tester(verdict: TestVerdict) -> DeliveryResult {
    DeliveryResult::Tester {
        verdict,
        report: artifact("rework-test-report"),
    }
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn one_task_persists_review_and_testing_rework_then_repeats_gates_before_devops() {
    let (fixture, mut cursor) = ReworkFixture::new().await;
    assert_eq!(MAX_REWORK_CYCLES, 3);
    // Exercise all three allowed cycles, then complete the same task. The fourth
    // rejection's terminal-stop path remains the existing pure policy regression.
    let stages = [
        (
            DeliveryResult::Analyst {
                analysis: artifact("rework-analysis"),
            },
            LifecycleStage::Analysis,
            LifecycleStage::Architecture,
            0,
        ),
        (
            DeliveryResult::Architect {
                design: artifact("rework-design"),
            },
            LifecycleStage::Architecture,
            LifecycleStage::Development,
            0,
        ),
        (
            developer(),
            LifecycleStage::Development,
            LifecycleStage::Review,
            0,
        ),
        (
            reviewer(ReviewVerdict::Rework),
            LifecycleStage::Review,
            LifecycleStage::Rework,
            1,
        ),
        (
            developer(),
            LifecycleStage::Rework,
            LifecycleStage::Review,
            1,
        ),
        (
            reviewer(ReviewVerdict::Approved),
            LifecycleStage::Review,
            LifecycleStage::Testing,
            1,
        ),
        (
            tester(TestVerdict::Rework),
            LifecycleStage::Testing,
            LifecycleStage::Rework,
            2,
        ),
        (
            developer(),
            LifecycleStage::Rework,
            LifecycleStage::Review,
            2,
        ),
        (
            reviewer(ReviewVerdict::Rework),
            LifecycleStage::Review,
            LifecycleStage::Rework,
            3,
        ),
        (
            developer(),
            LifecycleStage::Rework,
            LifecycleStage::Review,
            3,
        ),
        (
            reviewer(ReviewVerdict::Approved),
            LifecycleStage::Review,
            LifecycleStage::Testing,
            3,
        ),
        (
            tester(TestVerdict::Passed),
            LifecycleStage::Testing,
            LifecycleStage::Deployment,
            3,
        ),
        (
            DeliveryResult::DevOps {
                deployment: artifact("rework-deployment"),
                verification: artifact("rework-health-acceptance"),
            },
            LifecycleStage::Deployment,
            LifecycleStage::Deployment,
            3,
        ),
    ];
    let mut originals = Vec::new();
    let mut assignments = std::collections::HashSet::new();
    let mut executions = std::collections::HashSet::new();
    let mut keys = std::collections::HashSet::new();
    let mut results = std::collections::HashSet::new();
    for (index, (output, from, to, cycle)) in stages.into_iter().enumerate() {
        assert_eq!(cursor.task.stage, from);
        let original = fixture.step(&cursor, output, index).await;
        let assignment = original.reserved.assignment.as_ref().unwrap();
        assert!(assignments.insert(assignment.assignment_id));
        assert!(executions.insert(assignment.execution_id));
        assert!(keys.insert(assignment.execution.as_ref().unwrap().key.clone()));
        cursor = original.completed.clone();
        assert_eq!(cursor.task.stage, to);
        assert_eq!(cursor.task.cycle, cycle);
        assert!(results.insert(cursor.result.as_ref().unwrap().result_id));
        assert_eq!(cursor.task.transient_retries, 0);
        assert_eq!(cursor.task.resume_chat_id, None);
        if index == 12 {
            assert_eq!(cursor.task.status, LifecycleStatus::Completed);
            assert_eq!(cursor.task.runtime, RuntimeState::Terminal);
            assert_eq!(cursor.task.waiting_reason, None);
        } else {
            assert_eq!(cursor.task.status, LifecycleStatus::Ready);
            assert_eq!(cursor.task.runtime, RuntimeState::Queued);
            assert_eq!(
                cursor.task.waiting_reason.as_deref(),
                Some("queued_for_assignment")
            );
        }
        originals.push(original);
        assert_eq!(
            count(&fixture.db, "sdlc_delivery_assignments").await,
            (index + 1) as i64
        );
        assert_eq!(
            count(&fixture.db, "sdlc_delivery_results").await,
            (index + 1) as i64
        );
        assert_eq!(
            count(&fixture.db, "sdlc_delivery_proposals").await,
            (index + 1) as i64
        );
    }
    assert_eq!(cursor.task.cycle, MAX_REWORK_CYCLES);
    for identities in [&assignments, &executions, &results] {
        assert_eq!(identities.len(), 13);
    }
    assert_eq!(keys.len(), 13);
    assert_eq!(count(&fixture.db, "sdlc_pm_executions").await, 1);
    assert_eq!(count(&fixture.db, "sdlc_pm_completions").await, 1);
    assert_eq!(count(&fixture.db, "sdlc_task_routing_snapshots").await, 1);
    assert_eq!(count(&fixture.db, "sdlc_task_routing_generations").await, 0);
    assert_eq!(count(&fixture.db, "sdlc_analysis_reservations").await, 0);
    assert_eq!(count(&fixture.db, "sdlc_delivery_holds").await, 0);
    let terminal_history = fixture.journal_snapshot().await;
    // Historical proposal/command replay must retain the original cycle and stage,
    // even after later assignments and final completion on the SAME task.
    for original in &originals {
        fixture.assert_original_replays(original).await;
    }
    assert!(matches!(
        fixture
            .repo
            .lifecycle_command(
                fixture.task,
                &fixture.scheduler,
                LifecycleCommand::Reserve {
                    expected_task_version: cursor.task.version,
                    idempotency_key: "rework:after-completed:reserve".into(),
                }
            )
            .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(fixture.journal_snapshot().await, terminal_history);
    fixture.assert_current(&cursor).await;
}
