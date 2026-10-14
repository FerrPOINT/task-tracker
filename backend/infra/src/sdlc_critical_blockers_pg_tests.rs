//! Child of the existing candidates fixture, not another DB runner.
//! Runtime/Workflow observations and business content below are explicitly synthetic.
use super::*;
use crate::sdlc::attachment_artifacts as artifacts;
use domain::attachment_artifact::{ArtifactUpload, Seal};
use domain::sdlc_critical_blockers::*;

struct Running {
    task: Uuid,
    head: LifecycleReceipt,
    producer: Principal,
    evidence: domain::attachment_artifact::AttachmentArtifact,
}

fn fence(head: &LifecycleReceipt) -> DeliveryFence {
    let a = head.assignment.as_ref().unwrap();
    DeliveryFence {
        assignment_id: a.assignment_id,
        execution_id: a.execution_id,
        fencing_token: a.fencing_token,
        lease_id: a.lease_id,
        expected_lease_version: head.lease.as_ref().unwrap().version,
    }
}

async fn bind(f: &Fixture, task: Uuid) -> LifecycleReceipt {
    let head = f
        .repo
        .lifecycle_read(task, &f.reader)
        .await
        .unwrap()
        .receipt;
    let reserved = f
        .repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::Reserve {
                expected_task_version: head.task.version,
                idempotency_key: format!("reserve-{}", head.task.version),
            },
        )
        .await
        .unwrap()
        .0;
    f.repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::BindRun {
                fence: fence(&reserved),
                binding: RunBinding {
                    chat_id: Uuid::new_v4(),
                    run_id: Uuid::new_v4(),
                    machine_subject: "critical-agent".into(),
                    dispatch_receipt: ArtifactReceipt {
                        reference: "fixture://synthetic-dispatch".into(),
                        sha256: "a".repeat(64),
                    },
                },
                idempotency_key: format!("bind-{}", reserved.task.version),
            },
        )
        .await
        .unwrap()
        .0
}

async fn output(
    f: &Fixture,
    task: Uuid,
    head: LifecycleReceipt,
    storage: &domain::InMemoryStorage,
) -> Running {
    let producer = actor(
        "critical-agent",
        false,
        &[
            "task-tracker:read",
            "task-tracker:write",
            &head.assignment.as_ref().unwrap().scope(),
        ],
    );
    let item = artifacts::publish(
        &f.db,
        &f.repo.config,
        &producer,
        shared::IssueId::from_uuid(task),
        ArtifactUpload {
            file_name: "critical-evidence.json".into(),
            content_type: "application/json".into(),
            bytes: b"{ \"synthetic\": \"contradictory requirements\" }".to_vec(),
        },
        storage,
    )
    .await
    .unwrap();
    let evidence = Seal::parse(item.storage_key.as_ref())
        .unwrap()
        .unwrap()
        .artifact();
    Running {
        task,
        head,
        producer,
        evidence,
    }
}

fn result(r: &Running) -> DeliveryResult {
    match r.head.task.stage {
        LifecycleStage::Analysis => DeliveryResult::Analyst {
            analysis: r.evidence.receipt.clone(),
        },
        LifecycleStage::Architecture => DeliveryResult::Architect {
            design: r.evidence.receipt.clone(),
        },
        _ => panic!("fixture supports only its actual Analyst/Architect assignments"),
    }
}

async fn proposal(f: &Fixture, r: &Running) -> TerminalReceipt {
    let output = result(r);
    let workflow_receipt = ArtifactReceipt {
        reference: "fixture://synthetic-workflow-completion-not-live-proof".into(),
        sha256: "b".repeat(64),
    };
    f.repo
        .submit_delivery_proposal(
            r.task,
            &r.producer,
            SubmitDeliveryProposal {
                fence: fence(&r.head),
                workflow_receipt: workflow_receipt.clone(),
                result: output.clone(),
                idempotency_key: format!("proposal-{}", r.head.task.version),
            },
        )
        .await
        .unwrap();
    let binding = r.head.binding.as_ref().unwrap();
    TerminalReceipt {
        chat_id: binding.chat_id,
        run_id: binding.run_id,
        fleet_receipt: ArtifactReceipt {
            reference: "fixture://synthetic-native-terminal".into(),
            sha256: "c".repeat(64),
        },
        workflow_receipt,
        output_sha256: app::sdlc::canonical_hash(&output).unwrap(),
    }
}

async fn start(
    f: &Fixture,
    key: &str,
    architect: bool,
    storage: &domain::InMemoryStorage,
) -> Running {
    let task = f.confirmed(key, true).await;
    f.repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::EnsureQueued {
                idempotency_key: "queue".into(),
            },
        )
        .await
        .unwrap();
    let mut r = output(f, task, bind(f, task).await, storage).await;
    if architect {
        let terminal = proposal(f, &r).await;
        f.repo
            .lifecycle_command(
                task,
                &f.writer,
                LifecycleCommand::Complete {
                    fence: fence(&r.head),
                    terminal,
                    result: result(&r),
                    idempotency_key: "complete-analysis".into(),
                },
            )
            .await
            .unwrap();
        r = output(f, task, bind(f, task).await, storage).await;
    }
    r
}

fn blocker(r: &Running) -> SubmitCriticalBlocker {
    SubmitCriticalBlocker {
        fence: fence(&r.head),
        requirement_revision: r.head.task.routing.requirement_revision,
        content_hash: r.head.task.routing.content_hash.clone(),
        kind: CriticalBlockerKind::RequirementsContradiction,
        question: "Which retention requirement is authoritative?".into(),
        rationale: "Deletion and permanent retention cannot both apply to the same record.".into(),
        options: vec![],
        recommendation: None,
        evidence: vec![r.evidence.clone()],
        idempotency_key: "critical-post".into(),
    }
}

async fn snapshot(f: &Fixture, task: Uuid) -> Json {
    f.db.query_one(statement("SELECT jsonb_build_object(
        'state',(SELECT state FROM sdlc_tasks WHERE task_id=$1),
        'execution',(SELECT execution FROM sdlc_delivery_heads WHERE task_id=$1),
        'issue',(SELECT jsonb_build_object('status',status_id,'updated_at',updated_at) FROM issues WHERE id=$1),
        'blockers',(SELECT count(*) FROM sdlc_critical_blockers WHERE task_id=$1),
        'operations',(SELECT count(*) FROM sdlc_critical_blocker_operations WHERE task_id=$1),
        'heads',(SELECT count(*) FROM sdlc_critical_blocker_heads WHERE task_id=$1),
        'returns',(SELECT count(*) FROM sdlc_critical_pm_returns WHERE task_id=$1),
        'assignments',(SELECT count(*) FROM sdlc_delivery_assignments WHERE task_id=$1),
        'results',(SELECT count(*) FROM sdlc_delivery_results WHERE task_id=$1),
        'proposals',(SELECT count(*) FROM sdlc_delivery_proposals WHERE task_id=$1),
        'reconciliations',(SELECT count(*) FROM sdlc_delivery_reconciliations WHERE task_id=$1),
        'holds',(SELECT count(*) FROM sdlc_delivery_holds WHERE task_id=$1),
        'outbox',(SELECT count(*) FROM sdlc_outbox WHERE task_id=$1),
        'permits',(SELECT count(*) FROM sdlc_delivery_projection_permits WHERE task_id=$1)) AS value",
        vec![task.into()])).await.unwrap().unwrap().try_get("", "value").unwrap()
}

// A deliberately corrupt provider: actual metadata remains sealed, downloaded bytes differ.
struct CorruptBytes;
#[async_trait::async_trait]
impl domain::FileStorage for CorruptBytes {
    async fn put(&self, _: &str, _: &str, _: Vec<u8>) -> Result<(), AppError> {
        Err(AppError::Forbidden)
    }
    async fn get(&self, _: &str, _: &str) -> Result<Vec<u8>, AppError> {
        Ok(b"tampered bytes".to_vec())
    }
    async fn delete(&self, _: &str, _: &str) -> Result<(), AppError> {
        Err(AppError::Forbidden)
    }
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test; synthetic runtime only"]
async fn actual_critical_post_is_atomic_replayed_authorized_held_and_race_fenced() {
    let f = Fixture::new().await;
    if f.db
        .query_one(statement(
            "SELECT to_regclass('sdlc_critical_blockers') IS NULL AS missing",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<bool>("", "missing")
        .unwrap()
    {
        // Fresh-fixture source check only; main registers this in the ONE unreleased 096 packet.
        f.db.execute_unprepared(include_str!("../../migration/src/critical_pm_return.sql"))
            .await
            .unwrap();
    }
    assert!(
        f.db.query_one(statement(
            "SELECT to_regclass('sdlc_critical_pm_returns') IS NOT NULL AS present",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<bool>("", "present")
        .unwrap(),
        "old unreleased hold-only 096 is not an accepted upgrade baseline; use a fresh fixture"
    );
    let user = Uuid::new_v4();
    exec(&f.db, "INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)
        VALUES($1,'critical-agent@example.test','critical-agent','critical-agent','!','critical-agent',true)", vec![user.into()]).await.unwrap();
    exec(
        &f.db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'developer')",
        vec![f.project.into(), user.into()],
    )
    .await
    .unwrap();
    let storage = domain::InMemoryStorage::default();
    // Hold Architect, leaving the distinct Analyst agent/pool slot available for the race case.
    let mut r = start(&f, "critical-main", true, &storage).await;
    let terminal = proposal(&f, &r).await;
    let body = blocker(&r);
    let original_completion_proposal = SubmitDeliveryProposal {
        fence: fence(&r.head),
        workflow_receipt: terminal.workflow_receipt.clone(),
        result: result(&r),
        idempotency_key: format!("proposal-{}", r.head.task.version),
    };
    let frozen_body = body.clone();
    // Fleet heartbeat advances the SERVER lease; the caller's startup fence stays untouched.
    r.head = f
        .repo
        .lifecycle_command(
            r.task,
            &f.writer,
            LifecycleCommand::Heartbeat {
                fence: fence(&r.head),
                idempotency_key: "before-critical-heartbeat".into(),
            },
        )
        .await
        .unwrap()
        .0;
    assert!(body.fence.expected_lease_version < r.head.lease.as_ref().unwrap().version);
    let before = snapshot(&f, r.task).await;
    let initial_routing = r.head.task.routing.clone();
    // The database itself rejects a proposal-only commit, not just an application error path.
    let tx = f.db.begin().await.unwrap();
    let plan = app::sdlc_critical_blockers::prepare_and_hold(
        &r.head,
        &r.producer,
        &f.repo.config,
        &body,
        app::sdlc_critical_blockers::BlockerAllocation {
            blocker_id: Uuid::now_v7(),
            routing_generation: 1,
            observed_at: crate::sdlc::lifecycle::now(&tx).await.unwrap(),
        },
    )
    .unwrap();
    let unbacked = f.db.begin().await.unwrap();
    assert!(
        PostgresSdlcRepository::save_delivery_projection(
            &unbacked,
            &r.producer,
            &plan.lifecycle,
            false
        )
        .await
        .is_err()
    );
    unbacked.rollback().await.unwrap();
    assert_eq!(snapshot(&f, r.task).await, before);
    for changed_fence in 0..3 {
        let mut invalid = body.clone();
        match changed_fence {
            0 => invalid.fence.expected_lease_version = r.head.lease.as_ref().unwrap().version + 1,
            1 => invalid.fence.lease_id = Uuid::new_v4(),
            _ => invalid.fence.fencing_token += 1,
        }
        assert!(
            f.repo
                .submit_critical_blocker_with_storage(r.task, &r.producer, invalid, &storage)
                .await
                .is_err()
        );
        assert_eq!(snapshot(&f, r.task).await, before);
    }
    exec(&tx, "INSERT INTO sdlc_critical_blockers(blocker_id,task_id,assignment_id,payload,previous_receipt)
        VALUES($1,$2,$3,$4,$5)", vec![plan.blocker.blocker_id.into(), r.task.into(),
            body.fence.assignment_id.into(), json_value(&plan.blocker).unwrap(), json_value(&r.head).unwrap()]).await.unwrap();
    assert!(tx.commit().await.is_err());
    assert_eq!(snapshot(&f, r.task).await, before);
    assert!(
        f.repo
            .submit_critical_blocker_with_storage(r.task, &r.producer, body.clone(), &CorruptBytes)
            .await
            .is_err()
    );
    assert_eq!(snapshot(&f, r.task).await, before);
    let mut invalid = body.clone();
    invalid.recommendation = Some(Uuid::new_v4());
    assert!(
        f.repo
            .submit_critical_blocker_with_storage(r.task, &r.producer, invalid, &storage)
            .await
            .is_err()
    );
    assert_eq!(snapshot(&f, r.task).await, before);
    let mut wrong_hash = body.clone();
    wrong_hash.evidence[0].receipt.sha256 = "f".repeat(64);
    assert!(
        f.repo
            .submit_critical_blocker_with_storage(r.task, &r.producer, wrong_hash, &storage)
            .await
            .is_err()
    );
    assert_eq!(snapshot(&f, r.task).await, before);
    let impersonation = actor(
        "owner",
        false,
        &[
            "task-tracker:read",
            "task-tracker:write",
            &r.head.assignment.as_ref().unwrap().scope(),
        ],
    );
    assert!(matches!(
        f.repo
            .submit_critical_blocker_with_storage(r.task, &impersonation, body.clone(), &storage)
            .await,
        Err(AppError::Forbidden)
    ));
    assert_eq!(snapshot(&f, r.task).await, before);

    // Force failure AFTER projection writes: no partial held/head/proposal/journal may remain.
    f.db.execute_unprepared("CREATE FUNCTION critical_test_reject_event() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN IF NEW.event_type='critical.blocker_held' THEN RAISE EXCEPTION 'synthetic late write failure'; END IF;
        RETURN NEW; END $$; CREATE TRIGGER critical_test_reject_event BEFORE INSERT ON sdlc_outbox
        FOR EACH ROW EXECUTE FUNCTION critical_test_reject_event();").await.unwrap();
    assert!(
        f.repo
            .submit_critical_blocker_with_storage(r.task, &r.producer, body.clone(), &storage)
            .await
            .is_err()
    );
    assert_eq!(snapshot(&f, r.task).await, before);
    f.db.execute_unprepared("DROP TRIGGER critical_test_reject_event ON sdlc_outbox; DROP FUNCTION critical_test_reject_event();").await.unwrap();

    let (one, two) = tokio::join!(
        f.repo
            .submit_critical_blocker_with_storage(r.task, &r.producer, body.clone(), &storage),
        f.repo
            .submit_critical_blocker_with_storage(r.task, &r.producer, body.clone(), &storage),
    );
    let (one, replay_one) = one.unwrap();
    let (two, replay_two) = two.unwrap();
    assert_eq!(one, two);
    assert_ne!(replay_one, replay_two);
    assert_eq!(one.version, 1);
    assert_eq!(one.state, CriticalBlockerState::Held);
    assert!(one.proposal.options.is_empty());
    assert_eq!(one.pm_review, None);
    assert_eq!(one.technical_terminal, None);
    assert_eq!(one.producer_subject, "critical-agent");
    assert_eq!(one.proposal, frozen_body);
    assert_eq!(
        one.blocker_sha256,
        app::sdlc::canonical_hash(&frozen_body).unwrap()
    );
    let held = snapshot(&f, r.task).await;
    for count in ["blockers", "operations", "heads", "holds"] {
        assert_eq!(held[count], json!(1));
    }
    assert_eq!(held["permits"], json!(0));
    let actual = f
        .repo
        .lifecycle_read(r.task, &f.reader)
        .await
        .unwrap()
        .receipt;
    assert_eq!(actual.task.stage, r.head.task.stage);
    assert_eq!(actual.task.status, r.head.task.status);
    assert_eq!(actual.task.runtime, RuntimeState::Running);
    assert_eq!(actual.assignment, r.head.assignment);
    assert_eq!(actual.binding, r.head.binding);
    assert_eq!(actual.lease, r.head.lease);
    assert_eq!(actual.task.routing, initial_routing);
    assert_eq!(actual.task.version, r.head.task.version + 1);
    assert_eq!(actual.task.waiting_reason.as_deref(), Some(HOLD_REASON));
    assert_eq!(
        f.repo
            .critical_blocker_operation(r.task, &r.producer, &body.idempotency_key)
            .await
            .unwrap(),
        one
    );
    for reader in [&f.owner, &f.reader, &r.producer] {
        assert_eq!(
            f.repo
                .critical_blocker(r.task, reader, one.blocker_id)
                .await
                .unwrap(),
            one
        );
    }
    let foreign = actor(
        "foreign",
        false,
        &[
            "task-tracker:read",
            &r.head.assignment.as_ref().unwrap().scope(),
        ],
    );
    assert!(matches!(
        f.repo
            .critical_blocker(r.task, &foreign, one.blocker_id)
            .await,
        Err(AppError::Forbidden)
    ));
    exec(
        &f.db,
        "INSERT INTO project_members(project_id,user_id,role)
        SELECT $1,id,'developer' FROM users WHERE central_sub='foreign'",
        vec![f.project.into()],
    )
    .await
    .unwrap();
    for not_owner in [
        actor("foreign", true, &[]),
        actor("foreign", false, &["task-tracker:read", "admin"]),
    ] {
        assert!(matches!(
            f.repo
                .critical_blocker(r.task, &not_owner, one.blocker_id)
                .await,
            Err(AppError::Forbidden)
        ));
    }
    let mut changed = body.clone();
    changed.question.push('?');
    assert!(matches!(
        f.repo
            .submit_critical_blocker_with_storage(r.task, &r.producer, changed, &storage)
            .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(snapshot(&f, r.task).await, held);

    // Existing Heartbeat remains usable and does not change the original Held receipt.
    let heartbeat = f
        .repo
        .lifecycle_command(
            r.task,
            &f.writer,
            LifecycleCommand::Heartbeat {
                fence: fence(&actual),
                idempotency_key: "held-heartbeat".into(),
            },
        )
        .await
        .unwrap()
        .0;
    r.head = heartbeat;
    let replay_after_heartbeat = f
        .repo
        .submit_critical_blocker_with_storage(r.task, &r.producer, body.clone(), &CorruptBytes)
        .await
        .unwrap();
    assert_eq!(replay_after_heartbeat, (one.clone(), true));
    assert_eq!(
        f.repo
            .critical_blocker_operation(r.task, &r.producer, &body.idempotency_key)
            .await
            .unwrap(),
        one
    );
    let before_progress = snapshot(&f, r.task).await;
    let historical = f
        .repo
        .submit_delivery_proposal(r.task, &r.producer, original_completion_proposal.clone())
        .await
        .unwrap();
    assert!(historical.1);
    assert_eq!(historical.0.result, original_completion_proposal.result);
    assert_eq!(snapshot(&f, r.task).await, before_progress);
    let fresh = SubmitDeliveryProposal {
        fence: fence(&r.head),
        idempotency_key: "normal-completion-after-hold".into(),
        ..original_completion_proposal
    };
    match f
        .repo
        .submit_delivery_proposal(r.task, &r.producer, fresh)
        .await
        .unwrap_err()
    {
        AppError::Conflict(message) => assert!(message.contains("critical PM review holds")),
        error => panic!("expected critical hold conflict, got {error:?}"),
    }
    assert_eq!(snapshot(&f, r.task).await, before_progress);
    let binding = r.head.binding.as_ref().unwrap();
    let native = RuntimeTerminalObservation {
        assignment_hash: one.assignment_hash.clone(),
        chat_id: binding.chat_id,
        run_id: binding.run_id,
        state: RuntimeTerminalState::Cancelled,
        receipt: ArtifactReceipt {
            reference: "fixture://synthetic-native-terminal".into(),
            sha256: "d".repeat(64),
        },
    };
    for command in [
        LifecycleCommand::EnsureQueued {
            idempotency_key: "held-queue".into(),
        },
        LifecycleCommand::Reserve {
            expected_task_version: r.head.task.version,
            idempotency_key: "held-reserve".into(),
        },
        LifecycleCommand::Complete {
            fence: fence(&r.head),
            terminal,
            result: result(&r),
            idempotency_key: "held-complete".into(),
        },
        LifecycleCommand::Reconcile {
            fence: fence(&r.head),
            terminal: native.clone(),
            recovery: RecoveryDisposition::Stop,
            idempotency_key: "held-stop".into(),
        },
        LifecycleCommand::Reconcile {
            fence: fence(&r.head),
            terminal: RuntimeTerminalObservation {
                state: RuntimeTerminalState::Failed,
                ..native
            },
            recovery: RecoveryDisposition::RetryTransient {
                replay_safety_receipt: ArtifactReceipt {
                    reference: "fixture://synthetic-replay-safety".into(),
                    sha256: "e".repeat(64),
                },
            },
            idempotency_key: "held-retry".into(),
        },
    ] {
        assert!(matches!(
            f.repo.lifecycle_command(r.task, &f.writer, command).await,
            Err(AppError::Conflict(_))
        ));
        assert_eq!(snapshot(&f, r.task).await, before_progress);
    }
    let tx = f.db.begin().await.unwrap();
    assert!(
        exec(
            &tx,
            "DELETE FROM sdlc_delivery_holds WHERE task_id=$1",
            vec![r.task.into()]
        )
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    let tx = f.db.begin().await.unwrap();
    assert!(
        exec(
            &tx,
            "UPDATE sdlc_critical_blocker_operations SET result='{}' WHERE task_id=$1",
            vec![r.task.into()]
        )
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    assert_eq!(snapshot(&f, r.task).await, before_progress);

    let race = start(&f, "critical-race", false, &storage).await;
    let terminal = proposal(&f, &race).await;
    let race_body = blocker(&race);
    let (post, complete) = tokio::join!(
        f.repo
            .submit_critical_blocker_with_storage(race.task, &race.producer, race_body, &storage),
        f.repo.lifecycle_command(
            race.task,
            &f.writer,
            LifecycleCommand::Complete {
                fence: fence(&race.head),
                terminal,
                result: result(&race),
                idempotency_key: "racing-complete".into(),
            }
        ),
    );
    assert_ne!(
        post.is_ok(),
        complete.is_ok(),
        "exactly one actual task-locked operation wins"
    );
    let final_state = snapshot(&f, race.task).await;
    if post.is_ok() {
        assert_eq!(final_state["blockers"], json!(1));
        assert_eq!(final_state["holds"], json!(1));
        assert_eq!(final_state["state"]["waiting_reason"], json!(HOLD_REASON));
    } else {
        assert_eq!(final_state["blockers"], json!(0));
        assert_eq!(final_state["operations"], json!(0));
        assert_eq!(final_state["heads"], json!(0));
        assert_eq!(final_state["holds"], json!(0));
        assert_eq!(final_state["state"]["stage"], json!("Architecture"));
    }
    return_held_to_pm(&f, &r, &one, &body).await;
    assert_eq!(
        snapshot(&f, race.task).await,
        final_state,
        "return must not release/change another task"
    );
}

/// Same actual PG fixture, transaction paths and sealed evidence; native readback is SYNTHETIC.
async fn return_held_to_pm(
    f: &Fixture,
    r: &Running,
    original: &CriticalBlockerReceipt,
    original_body: &SubmitCriticalBlocker,
) {
    let held = f
        .repo
        .lifecycle_read(r.task, &f.reader)
        .await
        .unwrap()
        .receipt;
    let binding = held.binding.as_ref().unwrap();
    let command = ReturnCriticalToPm {
        review: CriticalPmReviewGuard {
            blocker_id: original.blocker_id,
            blocker_sha256: original.blocker_sha256.clone(),
            expected_blocker_version: original.version,
            expected_task_version: held.task.version,
            fence: fence(&held),
            idempotency_key: "return-critical-to-pm".into(),
        },
        terminal: RuntimeTerminalObservation {
            assignment_hash: original.assignment_hash.clone(),
            chat_id: binding.chat_id,
            run_id: binding.run_id,
            state: RuntimeTerminalState::Cancelled,
            receipt: ArtifactReceipt {
                reference: "fixture://SYNTHETIC-actual-native-terminal-readback".into(),
                sha256: "d".repeat(64),
            },
        },
    };
    let before = snapshot(f, r.task).await;
    assert_eq!(before["returns"], json!(0));
    assert_eq!(before["holds"], json!(1));
    for actor in [&r.producer, &f.owner, &f.reader] {
        assert!(matches!(
            f.repo
                .return_critical_to_pm_with_terminal(r.task, actor, command.clone())
                .await,
            Err(AppError::Forbidden)
        ));
        assert_eq!(snapshot(f, r.task).await, before);
    }
    // Caller must preserve the body but refresh ALL current CAS fields after heartbeat.
    for mismatch in 0..10 {
        let mut invalid = command.clone();
        match mismatch {
            0 => invalid.review.expected_task_version += 1,
            1 => invalid.review.expected_blocker_version += 1,
            2 => invalid.review.fence.expected_lease_version -= 1,
            3 => invalid.review.fence.expected_lease_version += 1,
            4 => invalid.review.fence.lease_id = Uuid::new_v4(),
            5 => invalid.review.fence.fencing_token += 1,
            6 => invalid.terminal.run_id = Uuid::new_v4(),
            7 => invalid.terminal.assignment_hash = "e".repeat(64),
            8 => invalid.review.blocker_sha256 = "f".repeat(64),
            _ => invalid.terminal.receipt.reference.clear(),
        }
        assert!(
            f.repo
                .return_critical_to_pm_with_terminal(r.task, &f.writer, invalid)
                .await
                .is_err()
        );
        assert_eq!(snapshot(f, r.task).await, before);
    }
    for nonterminal in ["unknown", "running", "stop_acknowledged", "timeout"] {
        let mut invalid = serde_json::to_value(&command).unwrap();
        invalid["terminal"]["state"] = json!(nonterminal);
        assert!(serde_json::from_value::<ReturnCriticalToPm>(invalid).is_err());
        assert_eq!(snapshot(f, r.task).await, before);
    }
    let plan = app::sdlc_critical_blockers::return_to_pm(
        &held,
        original,
        &f.writer,
        &f.repo.config,
        &command,
    )
    .unwrap();
    let tx = f.db.begin().await.unwrap();
    assert!(
        PostgresSdlcRepository::save_delivery_projection(&tx, &f.writer, &plan.lifecycle, false)
            .await
            .is_err(),
        "no journal-free canonical Draft or capacity release"
    );
    tx.rollback().await.unwrap();
    assert_eq!(snapshot(f, r.task).await, before);

    // SQL gates themselves reject forged terminal, stale lease, unrelated state and unrecoverable keys.
    for mismatch in 0..4 {
        let mut raw = serde_json::to_value(&command).unwrap();
        let mut after = serde_json::to_value(&plan.lifecycle).unwrap();
        let mut after_blocker = serde_json::to_value(&plan.blocker).unwrap();
        let key = if mismatch == 3 {
            "."
        } else {
            &command.review.idempotency_key
        };
        match mismatch {
            0 => {
                raw["terminal"]["state"] = json!("unknown");
                after_blocker["technical_terminal"] = raw["terminal"].clone();
            }
            1 => {
                raw["review"]["fence"]["expected_lease_version"] =
                    json!(held.lease.as_ref().unwrap().version - 1)
            }
            2 => after["task"]["cycle"] = json!(held.task.cycle + 1),
            _ => raw["review"]["idempotency_key"] = json!(key),
        }
        let tx = f.db.begin().await.unwrap();
        assert!(exec(&tx, "INSERT INTO sdlc_critical_pm_returns(return_id,task_id,blocker_id,assignment_id,actor_subject,
            idempotency_key,request_sha256,command,before_receipt,after_receipt,after_blocker)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)", vec![Uuid::now_v7().into(), r.task.into(),
                original.blocker_id.into(), command.review.fence.assignment_id.into(), f.writer.subject.clone().into(),
                key.into(), app::sdlc::canonical_hash(&raw).unwrap().into(), raw.into(),
                json_value(&held).unwrap(), after.into(), after_blocker.into()]).await.is_err());
        tx.rollback().await.unwrap();
        assert_eq!(snapshot(f, r.task).await, before);
    }

    let tx = f.db.begin().await.unwrap();
    exec(&tx, "INSERT INTO sdlc_critical_pm_returns(return_id,task_id,blocker_id,assignment_id,actor_subject,
        idempotency_key,request_sha256,command,before_receipt,after_receipt,after_blocker)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)", vec![Uuid::now_v7().into(), r.task.into(),
            original.blocker_id.into(), command.review.fence.assignment_id.into(), f.writer.subject.clone().into(),
            command.review.idempotency_key.clone().into(), app::sdlc::canonical_hash(&command).unwrap().into(),
            json_value(&command).unwrap(), json_value(&held).unwrap(), json_value(&plan.lifecycle).unwrap(),
            json_value(&plan.blocker).unwrap()]).await.unwrap();
    assert!(
        tx.commit().await.is_err(),
        "journal-only return cannot commit"
    );
    assert_eq!(snapshot(f, r.task).await, before);

    // Fail after archive, release, canonical and issue projection: rollback must restore all of them.
    f.db.execute_unprepared("CREATE FUNCTION critical_test_reject_return() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN IF NEW.event_type='critical.returned_to_pm' THEN RAISE EXCEPTION 'synthetic late return failure'; END IF;
        RETURN NEW; END $$; CREATE TRIGGER critical_test_reject_return BEFORE INSERT ON sdlc_outbox
        FOR EACH ROW EXECUTE FUNCTION critical_test_reject_return();").await.unwrap();
    assert!(
        f.repo
            .return_critical_to_pm_with_terminal(r.task, &f.writer, command.clone())
            .await
            .is_err()
    );
    assert_eq!(snapshot(f, r.task).await, before);
    f.db.execute_unprepared("DROP TRIGGER critical_test_reject_return ON sdlc_outbox; DROP FUNCTION critical_test_reject_return();")
        .await.unwrap();

    let (first, second) = tokio::join!(
        f.repo
            .return_critical_to_pm_with_terminal(r.task, &f.writer, command.clone()),
        f.repo
            .return_critical_to_pm_with_terminal(r.task, &f.writer, command.clone()),
    );
    let (first, replay_one) = first.unwrap();
    let (second, replay_two) = second.unwrap();
    assert_eq!(first, second);
    assert_eq!(first, plan);
    assert_ne!(
        replay_one, replay_two,
        "one task-lock winner, one immutable original-key replay"
    );
    let actual = f
        .repo
        .lifecycle_read(r.task, &f.reader)
        .await
        .unwrap()
        .receipt;
    assert_eq!(actual, plan.lifecycle);
    assert_eq!(actual.task.stage, LifecycleStage::Draft);
    assert_eq!(actual.task.status, LifecycleStatus::Ready);
    assert_eq!(actual.task.runtime, RuntimeState::Terminal);
    assert_eq!(
        actual.task.waiting_reason.as_deref(),
        Some(PM_REVIEW_REASON)
    );
    assert_eq!(actual.task.version, held.task.version + 1);
    assert_eq!(actual.task.routing, held.task.routing);
    assert_eq!(actual.task.cycle, held.task.cycle);
    assert_eq!(actual.task.transient_retries, held.task.transient_retries);
    assert_eq!(actual.task.previous_result_id, None);
    assert_eq!(actual.task.resume_chat_id, None);
    assert_eq!(actual.assignment, None);
    assert_eq!(actual.lease, None);
    assert_eq!(actual.binding, None);
    assert_eq!(actual.result, None);
    assert_eq!(plan.blocker.version, 2);
    assert_eq!(plan.blocker.state, CriticalBlockerState::AwaitingPmReview);
    assert_eq!(plan.blocker.proposal, *original_body);
    assert_eq!(
        plan.blocker.technical_terminal,
        Some(command.terminal.clone())
    );
    assert_eq!(plan.blocker.pm_review, None);
    assert_eq!(plan.blocker.resolution, None);
    for reader in [&f.owner, &f.reader] {
        assert_eq!(
            f.repo
                .critical_blocker(r.task, reader, original.blocker_id)
                .await
                .unwrap(),
            plan.blocker
        );
    }
    let row = f.db.query_one(statement(
        "SELECT r.before_receipt,r.command,r.after_receipt,r.after_blocker,b.payload,
            (SELECT count(*) FROM sdlc_outbox e WHERE e.task_id=r.task_id AND e.event_type='critical.returned_to_pm') AS events
         FROM sdlc_critical_pm_returns r JOIN sdlc_critical_blockers b USING(blocker_id,task_id) WHERE r.task_id=$1",
        vec![r.task.into()],
    )).await.unwrap().unwrap();
    assert_eq!(
        row.try_get::<Json>("", "before_receipt").unwrap(),
        serde_json::to_value(&held).unwrap()
    );
    assert_eq!(
        row.try_get::<Json>("", "command").unwrap(),
        serde_json::to_value(&command).unwrap()
    );
    assert_eq!(
        row.try_get::<Json>("", "after_receipt").unwrap(),
        serde_json::to_value(&actual).unwrap()
    );
    assert_eq!(
        row.try_get::<Json>("", "after_blocker").unwrap(),
        serde_json::to_value(&plan.blocker).unwrap()
    );
    assert_eq!(
        row.try_get::<Json>("", "payload").unwrap(),
        serde_json::to_value(original).unwrap()
    );
    assert_eq!(row.try_get::<i64>("", "events").unwrap(), 1);
    let returned = snapshot(f, r.task).await;
    assert_eq!(returned["returns"], json!(1));
    assert_eq!(returned["holds"], json!(0));
    assert_eq!(returned["permits"], json!(0));
    for retained in [
        "blockers",
        "operations",
        "heads",
        "assignments",
        "results",
        "proposals",
        "reconciliations",
    ] {
        assert_eq!(returned[retained], before[retained]);
    }
    // The retired child may recover only its ORIGINAL Held operation, never fresh work/detail.
    assert_eq!(
        f.repo
            .critical_blocker_operation(r.task, &r.producer, &original_body.idempotency_key)
            .await
            .unwrap(),
        *original
    );
    assert_eq!(
        f.repo
            .submit_critical_blocker_with_storage(
                r.task,
                &r.producer,
                original_body.clone(),
                &CorruptBytes
            )
            .await
            .unwrap(),
        (original.clone(), true)
    );
    assert!(
        f.repo
            .critical_blocker(r.task, &r.producer, original.blocker_id)
            .await
            .is_err()
    );
    let mut fresh = original_body.clone();
    fresh.idempotency_key = "post-after-return".into();
    assert!(
        f.repo
            .submit_critical_blocker_with_storage(r.task, &r.producer, fresh, &CorruptBytes)
            .await
            .is_err()
    );
    assert_eq!(
        f.repo
            .return_critical_to_pm_with_terminal(r.task, &f.writer, command.clone())
            .await
            .unwrap(),
        (plan, true)
    );
    let mut changed = command.clone();
    changed.terminal.receipt.sha256 = "e".repeat(64);
    assert!(matches!(
        f.repo
            .return_critical_to_pm_with_terminal(r.task, &f.writer, changed)
            .await,
        Err(AppError::Conflict(_))
    ));
    let mut duplicate = command;
    duplicate.review.idempotency_key = "second-return".into();
    assert!(
        f.repo
            .return_critical_to_pm_with_terminal(r.task, &f.writer, duplicate)
            .await
            .is_err()
    );
    for command in [
        LifecycleCommand::EnsureQueued {
            idempotency_key: "awaiting-queue".into(),
        },
        LifecycleCommand::Reserve {
            expected_task_version: actual.task.version,
            idempotency_key: "awaiting-reserve".into(),
        },
    ] {
        assert!(matches!(
            f.repo.lifecycle_command(r.task, &f.writer, command).await,
            Err(AppError::Conflict(_))
        ));
    }
    for sql in [
        "UPDATE sdlc_critical_pm_returns SET after_blocker='{}' WHERE task_id=$1",
        "DELETE FROM sdlc_critical_pm_returns WHERE task_id=$1",
    ] {
        let tx = f.db.begin().await.unwrap();
        assert!(exec(&tx, sql, vec![r.task.into()]).await.is_err());
        tx.rollback().await.unwrap();
    }
    assert_eq!(snapshot(f, r.task).await, returned);
}
