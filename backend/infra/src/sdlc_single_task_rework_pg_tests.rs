//! Child of lifecycle::regression_tests; main owns registration and PostgreSQL QA.
//! Real repository transactions, synthetic role artifacts and trusted terminal receipts.
//! No live Fleet/Workflow execution or attachment-byte verification is claimed.
use super::*;
#[path = "decomposition_architect_pg_tests.rs"]
mod decomposition_tests;
#[path = "reviewer_handoff_pg_tests.rs"]
mod reviewer_handoff_tests;
#[path = "untimed_delivery_binding_pg_tests.rs"]
mod untimed_binding_tests;

struct ReworkFixture {
    db: DatabaseConnection,
    boards: std::sync::Arc<dyn domain::BoardRepository>,
    repo: PostgresSdlcRepository,
    task: Uuid,
    owner: Principal,
    scheduler: Principal,
    reader: Principal,
    frozen: TaskRoutingSnapshot,
    pm_history: Json,
    producers: std::collections::HashMap<String, Principal>,
}

struct OriginalStep {
    reserve: LifecycleCommand,
    reserved: LifecycleReceipt,
    start: StartStage,
    started: StageStartReceipt,
    complete: LifecycleCommand,
    completed: LifecycleReceipt,
    producer: Principal,
    proposal_command: SubmitDeliveryProposal,
    proposal: Json,
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn human_stop_of_unsent_reservation_releases_custody_once_without_native_or_retry() {
    let (f, queued) = ReworkFixture::new().await;
    let reserve = LifecycleCommand::Reserve {
        expected_task_version: queued.task.version,
        machine_subject: Some(f.producers["analyst"].subject.clone()),
        idempotency_key: "unsent:reserve".into(),
    };
    let (reserved, _) = f.repo.lifecycle_command(f.task, &f.scheduler, reserve.clone()).await.unwrap();
    let assignment = reserved.assignment.as_ref().unwrap();
    let stop = LifecycleCommand::Reconcile {
        fence: delivery_fence(&reserved), terminal: None,
        never_submitted: Some(NeverSubmittedObservation {
            assignment_hash: assignment.assignment_hash.clone(),
            reservation_key: reserve.key().into(),
            reservation_payload_sha256: app::sdlc::canonical_hash(&reserve).unwrap(),
        }), recovery: RecoveryDisposition::Stop, idempotency_key: "unsent:stop".into(),
    };
    let original = f.journal_snapshot().await;
    for field in ["assignment_hash", "reservation_key", "reservation_payload_sha256"] {
        let mut raw = serde_json::to_value(&stop).unwrap();
        raw["never_submitted"][field] = json!(if field == "reservation_key" { "foreign:reserve".into() } else { "b".repeat(64) });
        assert!(f.repo.lifecycle_command(f.task, &f.scheduler, parse(raw)).await.is_err());
        assert_eq!(f.journal_snapshot().await, original);
    }
    let foreign = actor("foreign-scheduler", false, vec![]);
    assert!(matches!(f.repo.lifecycle_command(f.task, &foreign, stop.clone()).await, Err(AppError::Forbidden)));
    let no_op = LifecycleCommand::EnsureQueued { idempotency_key: "unsent:reserved-no-op".into() };
    let (same, _) = f.repo.lifecycle_command(f.task, &f.scheduler, no_op.clone()).await.unwrap();
    assert_eq!(same, reserved);
    let mut wrong = stop.clone();
    if let LifecycleCommand::Reconcile { never_submitted: Some(proof), .. } = &mut wrong {
        proof.reservation_key = no_op.key().into();
        proof.reservation_payload_sha256 = app::sdlc::canonical_hash(&no_op).unwrap();
    }
    let before_wrong = f.journal_snapshot().await;
    assert!(f.repo.lifecycle_command(f.task, &f.scheduler, wrong).await.is_err());
    assert_eq!(f.journal_snapshot().await, before_wrong);
    f.assert_current(&reserved).await;
    let (stopped, duplicate) = f.repo.lifecycle_command(f.task, &f.scheduler, stop.clone()).await.unwrap();
    assert!(!duplicate);
    assert_eq!(stopped.task.runtime, RuntimeState::Terminal);
    assert_eq!(stopped.task.waiting_reason.as_deref(), Some("cancelled_before_submission"));
    assert_eq!(stopped.task.stage, queued.task.stage);
    assert_eq!(stopped.task.routing, queued.task.routing);
    assert_eq!(stopped.task.previous_result_id, queued.task.previous_result_id);
    assert_eq!(stopped.task.transient_retries, queued.task.transient_retries);
    assert!(stopped.assignment.is_none() && stopped.lease.is_none() && stopped.binding.is_none() && stopped.result.is_none());
    assert_eq!(count(&f.db, "sdlc_delivery_holds").await, 0);
    assert_eq!(count(&f.db, "sdlc_delivery_run_bindings").await, 0);
    assert_eq!(count(&f.db, "sdlc_delivery_reconciliations").await, 1);
    let after = f.journal_snapshot().await;
    let (same, duplicate) = f.repo.lifecycle_command(f.task, &f.scheduler, stop).await.unwrap();
    assert!(duplicate);
    assert_eq!(same, stopped);
    let (old, duplicate) = f.repo.lifecycle_command(f.task, &f.scheduler, reserve).await.unwrap();
    assert!(duplicate);
    assert_eq!(old, reserved);
    assert_eq!(f.journal_snapshot().await, after);
    assert!(f.repo.lifecycle_command(f.task, &f.scheduler, LifecycleCommand::Reserve {
        expected_task_version: stopped.task.version,
        machine_subject: Some(f.producers["analyst"].subject.clone()),
        idempotency_key: "unsent:cannot-redispatch".into(),
    }).await.is_err());
    f.assert_current(&stopped).await;
    // A separate, explicit intervention may queue work; cancellation itself did not.
    let (queued, _) = f.repo.lifecycle_command(f.task, &f.scheduler, LifecycleCommand::EnsureQueued {
        idempotency_key: "unsent:explicit-queue".into(),
    }).await.unwrap();
    let reserve = LifecycleCommand::Reserve {
        expected_task_version: queued.task.version,
        machine_subject: Some(f.producers["analyst"].subject.clone()),
        idempotency_key: "unsent:second-reserve".into(),
    };
    let (reserved, _) = f.repo.lifecycle_command(f.task, &f.scheduler, reserve.clone()).await.unwrap();
    let (bound, _) = f.repo.lifecycle_command(f.task, &f.scheduler, LifecycleCommand::BindRun {
        fence: delivery_fence(&reserved), binding: RunBinding {
            chat_id: Uuid::new_v4(), run_id: Uuid::new_v4(),
            machine_subject: f.producers["analyst"].subject.clone(), dispatch_receipt: artifact("accepted-native"),
        }, idempotency_key: "unsent:known-bind".into(),
    }).await.unwrap();
    let wrong = LifecycleCommand::Reconcile {
        fence: delivery_fence(&bound), terminal: None,
        never_submitted: Some(NeverSubmittedObservation {
            assignment_hash: reserved.assignment.as_ref().unwrap().assignment_hash.clone(),
            reservation_key: reserve.key().into(),
            reservation_payload_sha256: app::sdlc::canonical_hash(&reserve).unwrap(),
        }), recovery: RecoveryDisposition::Stop, idempotency_key: "unsent:bound-cannot-stop-as-unsent".into(),
    };
    let before_wrong = f.journal_snapshot().await;
    assert!(f.repo.lifecycle_command(f.task, &f.scheduler, wrong).await.is_err());
    assert_eq!(f.journal_snapshot().await, before_wrong);
    assert_eq!(count(&f.db, "sdlc_delivery_holds").await, 1);
    f.assert_current(&bound).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn unsent_stop_and_bind_run_serialize_without_releasing_accepted_custody() {
    let (f, queued) = ReworkFixture::new().await;
    let reserve = LifecycleCommand::Reserve {
        expected_task_version: queued.task.version,
        machine_subject: Some(f.producers["analyst"].subject.clone()),
        idempotency_key: "unsent-race:reserve".into(),
    };
    let (reserved, _) = f.repo.lifecycle_command(f.task, &f.scheduler, reserve.clone()).await.unwrap();
    let stop = LifecycleCommand::Reconcile {
        fence: delivery_fence(&reserved), terminal: None,
        never_submitted: Some(NeverSubmittedObservation {
            assignment_hash: reserved.assignment.as_ref().unwrap().assignment_hash.clone(),
            reservation_key: reserve.key().into(),
            reservation_payload_sha256: app::sdlc::canonical_hash(&reserve).unwrap(),
        }), recovery: RecoveryDisposition::Stop, idempotency_key: "unsent-race:stop".into(),
    };
    let bind = LifecycleCommand::BindRun {
        fence: delivery_fence(&reserved),
        binding: RunBinding {
            chat_id: Uuid::new_v4(), run_id: Uuid::new_v4(),
            machine_subject: f.producers["analyst"].subject.clone(), dispatch_receipt: artifact("race-native"),
        }, idempotency_key: "unsent-race:bind".into(),
    };
    let (stopped, bound) = tokio::join!(
        f.repo.lifecycle_command(f.task, &f.scheduler, stop.clone()),
        f.repo.lifecycle_command(f.task, &f.scheduler, bind.clone()),
    );
    assert_ne!(stopped.is_ok(), bound.is_ok());
    if let Ok((running, _)) = bound {
        assert_eq!(running.task.runtime, RuntimeState::Running);
        assert!(f.repo.lifecycle_command(f.task, &f.scheduler, stop).await.is_err());
        assert_eq!(count(&f.db, "sdlc_delivery_holds").await, 1);
        assert_eq!(count(&f.db, "sdlc_delivery_reconciliations").await, 0);
        f.assert_current(&running).await;
    } else {
        assert!(f.repo.lifecycle_command(f.task, &f.scheduler, bind).await.is_err());
        assert_eq!(count(&f.db, "sdlc_delivery_holds").await, 0);
        assert_eq!(count(&f.db, "sdlc_delivery_run_bindings").await, 0);
    }
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn assignment_persists_across_old_deadline_and_restart_until_result_or_known_terminal_stop() {
    let (mut f, queued) = ReworkFixture::configured_before_persistent_ownership().await;
    let output = DeliveryResult::Analyst { analysis: artifact("persistent-analysis") };
    let reserve = LifecycleCommand::Reserve {
        expected_task_version: queued.task.version,
        machine_subject: Some(f.producers["analyst"].subject.clone()),
        idempotency_key: "persistent:reserve".into(),
    };
    // Publish one legitimate predecessor reservation through the existing owner
    // transaction helpers, with its original TTL30 metadata before first save.
    let tx = f.db.begin().await.unwrap();
    let state = f.repo.load(&tx, f.task, &f.scheduler).await.unwrap();
    let mut historical = current(&tx, f.task).await.unwrap().unwrap();
    f.repo.delivery_reserve(&tx, &f.scheduler, &mut historical, queued.task.version,
        Some(f.producers["analyst"].subject.as_str())).await.unwrap();
    let lease = historical.lease.as_mut().unwrap();
    lease.heartbeat_at = Some(lease.claimed_at);
    lease.expires_at = Some(lease.claimed_at + chrono::Duration::seconds(30));
    PostgresSdlcRepository::save_delivery_projection(&tx, &f.scheduler, &historical, false).await.unwrap();
    let event_id = Uuid::now_v7();
    let hash = app::sdlc::canonical_hash(&reserve).unwrap();
    exec(&tx, "INSERT INTO sdlc_delivery_operations(task_id,actor_subject,idempotency_key,payload_hash,result,event_id)
        VALUES($1,$2,$3,$4,$5,$6)", vec![f.task.into(),f.scheduler.subject.clone().into(),reserve.key().into(),
        hash.into(),json_value(&historical).unwrap(),event_id.into()]).await.unwrap();
    let event = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
        "project_id":state.project_id,"task_id":f.task,"root_task_id":f.task,"owner_subject":state.owner_subject,
        "stage":historical.task.stage,"requirement_revision":historical.task.routing.requirement_revision,"result":historical});
    exec(&tx, "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,'delivery.assignment_reserved',$3)",
        vec![event_id.into(),f.task.into(),event.into()]).await.unwrap();
    tx.commit().await.unwrap();
    let original_json: Json = f.db.query_one(statement("SELECT result FROM sdlc_delivery_operations WHERE task_id=$1 AND idempotency_key=$2",
        vec![f.task.into(),reserve.key().into()])).await.unwrap().unwrap().try_get("", "result").unwrap();
    let assignment = historical.assignment.as_ref().unwrap().clone();
    // Advance the real database clock past the old TTL; never UPDATE custody dates.
    f.db.execute_unprepared("SELECT pg_sleep(31)").await.unwrap();
    migration::Migrator::up(&f.db, None).await.unwrap();
    let url = std::env::var("TT_LIFECYCLE_TEST_DATABASE_URL").unwrap();
    f.repo = PostgresSdlcRepository::connect(&url, f.repo.config.clone()).await.unwrap();
    let reserved = f.repo.lifecycle_read(f.task, &f.reader).await.unwrap().receipt;
    let mut expected = historical.clone();
    expected.lease.as_mut().unwrap().expires_at = None;
    assert_eq!(reserved, expected);
    assert_eq!(reserved.assignment.as_ref(), Some(&assignment));
    f.assert_current(&reserved).await;
    let (replayed, duplicate) = f.repo.lifecycle_command(f.task, &f.scheduler, reserve.clone()).await.unwrap();
    assert!(duplicate);
    assert_eq!(replayed, historical);
    let unchanged: Json = f.db.query_one(statement("SELECT result FROM sdlc_delivery_operations WHERE task_id=$1 AND idempotency_key=$2",
        vec![f.task.into(),reserve.key().into()])).await.unwrap().unwrap().try_get("", "result").unwrap();
    assert_eq!(unchanged, original_json);
    assert_eq!(count(&f.db, "sdlc_delivery_holds").await, 1);
    let heartbeat = LifecycleCommand::Heartbeat { fence: delivery_fence(&reserved), idempotency_key: "persistent:no-renew".into() };
    let retry = LifecycleCommand::Reconcile {
        fence: delivery_fence(&reserved), terminal: None,
        never_submitted: Some(NeverSubmittedObservation {
            assignment_hash: assignment.assignment_hash.clone(), reservation_key: reserve.key().into(),
            reservation_payload_sha256: app::sdlc::canonical_hash(&reserve).unwrap(),
        }), recovery: RecoveryDisposition::RetryUnsubmitted, idempotency_key: "persistent:no-expiry-retry".into(),
    };
    for command in [heartbeat, retry, LifecycleCommand::Reserve {
        expected_task_version: reserved.task.version,
        machine_subject: Some(f.producers["analyst"].subject.clone()), idempotency_key: "persistent:no-duplicate".into(),
    }] {
        assert!(f.repo.lifecycle_command(f.task, &f.scheduler, command).await.is_err());
    }
    let foreign = actor("foreign-scheduler", false, vec![]);
    let binding = RunBinding {
        chat_id: Uuid::new_v4(), run_id: Uuid::new_v4(),
        machine_subject: f.producers["analyst"].subject.clone(), dispatch_receipt: artifact("persistent-native"),
    };
    let bind = LifecycleCommand::BindRun { fence: delivery_fence(&reserved), binding: binding.clone(), idempotency_key: "persistent:foreign-bind".into() };
    assert!(matches!(f.repo.lifecycle_command(f.task, &foreign, bind).await, Err(AppError::Forbidden)));
    let mut stale = delivery_fence(&reserved);
    stale.fencing_token += 1;
    assert!(f.repo.lifecycle_command(f.task, &f.scheduler, LifecycleCommand::BindRun {
        fence: stale, binding, idempotency_key: "persistent:stale-bind".into(),
    }).await.is_err());
    f.assert_current(&reserved).await;
    assert_eq!(count(&f.db, "sdlc_delivery_reconciliations").await, 0);
    // The same assignment can bind, start and complete after the former deadline.
    let initial = f.finish_reserved(&queued, output, 0, reserve.clone(), reserved.clone(), false).await;
    let completed = initial.completed.clone();
    assert_eq!(count(&f.db, "sdlc_delivery_holds").await, 0);
    assert_eq!(completed.task.stage, LifecycleStage::Architecture);
    // Exercise a non-empty predecessor through real Complete and the existing
    // public result read model, rather than only the root result's None case.
    let architecture = f.step(&completed, DeliveryResult::Architect {
        design: artifact("persistent-design"),
    }, 1).await;
    assert_eq!(architecture.reserved.assignment.as_ref().unwrap().previous_result_id,
        Some(initial.completed.result.as_ref().unwrap().result_id));
    let history_before_read = f.journal_snapshot().await;
    for original in [&initial, &architecture] {
        let assignment = original.reserved.assignment.as_ref().unwrap();
        let accepted = original.completed.result.as_ref().unwrap();
        assert_eq!(accepted.previous_result_id, assignment.previous_result_id);
        let DeliveryInput::Delivery(actual) = f.repo
            .lifecycle_result(f.task, &f.reader, accepted.result_id).await.unwrap()
        else { panic!("expected retained delivery result"); };
        assert_eq!(&actual.result, accepted);
        assert_eq!(actual.previous_result_id, assignment.previous_result_id);
        let mut expected_projection = serde_json::to_value(accepted).unwrap();
        expected_projection["previous_result_id"] = json!(assignment.previous_result_id);
        assert_eq!(serde_json::to_value(&actual).unwrap(), expected_projection);
    }
    assert_eq!(f.journal_snapshot().await, history_before_read);
    let (next, _) = f.repo.lifecycle_command(f.task, &f.scheduler, LifecycleCommand::Reserve {
        expected_task_version: architecture.completed.task.version,
        machine_subject: Some(f.producers["developer"].subject.clone()), idempotency_key: "persistent:next-stage".into(),
    }).await.unwrap();
    assert_ne!(next.assignment.as_ref().unwrap().assignment_id, assignment.assignment_id);
    let binding = RunBinding {
        chat_id: Uuid::new_v4(), run_id: Uuid::new_v4(),
        machine_subject: f.producers["developer"].subject.clone(), dispatch_receipt: artifact("persistent-stop-native"),
    };
    let (running, _) = f.repo.lifecycle_command(f.task, &f.scheduler, LifecycleCommand::BindRun {
        fence: delivery_fence(&next), binding: binding.clone(), idempotency_key: "persistent:stop-bind".into(),
    }).await.unwrap();
    let terminal = RuntimeTerminalObservation {
        assignment_hash: next.assignment.as_ref().unwrap().assignment_hash.clone(),
        chat_id: binding.chat_id, run_id: binding.run_id, state: RuntimeTerminalState::Cancelled,
        receipt: artifact("persistent-known-terminal"),
    };
    let stop = LifecycleCommand::Reconcile { fence: delivery_fence(&running), terminal: Some(terminal),
        never_submitted: None, recovery: RecoveryDisposition::Stop, idempotency_key: "persistent:known-stop".into() };
    let mut foreign_terminal = stop.clone();
    if let LifecycleCommand::Reconcile { terminal: Some(terminal), idempotency_key, .. } = &mut foreign_terminal {
        terminal.run_id = Uuid::new_v4();
        *idempotency_key = "persistent:foreign-terminal".into();
    }
    assert!(f.repo.lifecycle_command(f.task, &f.scheduler, foreign_terminal).await.is_err());
    f.assert_current(&running).await;
    assert_eq!(count(&f.db, "sdlc_delivery_holds").await, 1);
    let (stopped, replayed) = f.repo.lifecycle_command(f.task, &f.scheduler, stop.clone()).await.unwrap();
    assert!(!replayed);
    assert_eq!(stopped.task.stage, next.task.stage);
    assert_eq!(stopped.task.routing, next.task.routing);
    assert_eq!(stopped.task.transient_retries, 0);
    assert_eq!(stopped.task.runtime, RuntimeState::Terminal);
    assert!(stopped.assignment.is_none() && stopped.binding.is_none() && stopped.lease.is_none());
    assert_eq!(count(&f.db, "sdlc_delivery_holds").await, 0);
    let (same, duplicate) = f.repo.lifecycle_command(f.task, &f.scheduler, stop).await.unwrap();
    assert!(duplicate);
    assert_eq!(same, stopped);
    let (old, duplicate) = f.repo.lifecycle_command(f.task, &f.scheduler, reserve).await.unwrap();
    assert!(duplicate);
    assert_eq!(old, historical);
    f.assert_current(&stopped).await;
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
         'configured_completions',(SELECT jsonb_agg(to_jsonb(c) ORDER BY completion_id) FROM sdlc_configured_pm_completions c WHERE c.task_id=t.task_id),
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
    reader: &Principal,
    agent: Uuid,
    document: Option<RequirementsDocument>,
) -> LifecycleReceipt {
    let document: RequirementsDocument = document.unwrap_or_else(|| parse(json!({
        "goal":"One task with actual persisted rework","scope":["One root task"],"exclusions":["No children"],
        "scenarios":["Review and testing reject work"],
        "acceptance_criteria":["Rework repeats review and testing before deployment"],
        "constraints":[],"dependencies":[],"assumptions":[],"checklist":["check"],"prerequisites":["prerequisite"]})));
    let (reserved, completed) = crate::sdlc::configured_test_support::confirmed_pm(
        repo, task, owner, agent, document, true,
    )
    .await;
    let completed = completed.unwrap();
    assert_eq!(
        completed.fence.execution_id,
        reserved.assignment.execution_id
    );
    assert_eq!(completed.stage, Stage::Draft);
    assert_eq!(completed.status, LifecycleStatus::Completed);
    let after_pm = repo.read(task, owner).await.unwrap();
    assert_eq!(after_pm.stage, Stage::Analysis);
    assert_eq!(after_pm.status, Some(LifecycleStatus::Ready));
    assert_eq!(after_pm.owner_subject, owner.subject);
    assert_eq!(after_pm.root_task_id, task);
    let queued = repo.lifecycle_read(task, reader).await.unwrap().receipt;
    assert_eq!(queued.task.stage, LifecycleStage::Analysis);
    assert_eq!(queued.task.status, LifecycleStatus::Ready);
    assert_eq!(queued.task.runtime, RuntimeState::Queued);
    assert_eq!(queued.task.owner_subject, owner.subject);
    assert!(queued.assignment.is_none() && queued.lease.is_none() && queued.binding.is_none());
    let consent = after_pm.current_confirmation().unwrap();
    assert_eq!(queued.task.routing.confirmation_id, consent.id);
    assert_eq!(queued.task.routing.requirement_revision, consent.revision);
    assert_eq!(queued.task.routing.content_hash, consent.content_hash);
    queued
}


#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn explicit_routing_resume_preserves_confirmed_source_history_and_selects_new_assignment() {
    use domain::sdlc_source::TaskSourceRepository;
    use domain::sdlc_candidates::{DeliveryCandidatesQuery,CandidateOperation};
    let (f,queued) = ReworkFixture::with_schema_tail(None,None,"owner","scheduler",
        Some("m20261114_0000119_current_delivery_contract")).await;
    let old_source:Json = f.db.query_one(statement(
        "SELECT payload FROM sdlc_task_source_bindings WHERE task_id=$1",vec![f.task.into()]))
        .await.unwrap().unwrap().try_get("","payload").unwrap();
    let reserve = LifecycleCommand::Reserve { expected_task_version:queued.task.version,
        machine_subject:Some(f.producers["analyst"].subject.clone()),idempotency_key:"routing:old-reserve".into() };
    let queued_before=f.journal_snapshot().await;
    assert!(f.repo.lifecycle_command(f.task,&f.scheduler,LifecycleCommand::ResumeWithCurrentRouting {
        expected_task_version:queued.task.version,expected_routing_policy_version:2,
        idempotency_key:"routing:busy-queued".into() }).await.is_err());
    assert_eq!(f.journal_snapshot().await,queued_before);
    let reserved=f.repo.lifecycle_command(f.task,&f.scheduler,reserve.clone()).await.unwrap().0;
    let reserved_before=f.journal_snapshot().await;
    assert!(f.repo.lifecycle_command(f.task,&f.scheduler,LifecycleCommand::ResumeWithCurrentRouting {
        expected_task_version:reserved.task.version,expected_routing_policy_version:2,
        idempotency_key:"routing:busy-reserved".into() }).await.is_err());
    assert_eq!(f.journal_snapshot().await,reserved_before);
    let binding=RunBinding { chat_id:Uuid::new_v4(),run_id:Uuid::new_v4(),machine_subject:f.producers["analyst"].subject.clone(),
        dispatch_receipt:artifact("routing-original-dispatch") };
    let running=f.repo.lifecycle_command(f.task,&f.scheduler,LifecycleCommand::BindRun {
        fence:delivery_fence(&reserved),binding:binding.clone(),idempotency_key:"routing:old-bind".into() }).await.unwrap().0;
    for current in [&running] {
        let before=f.journal_snapshot().await;
        assert!(f.repo.lifecycle_command(f.task,&f.scheduler,LifecycleCommand::ResumeWithCurrentRouting {
            expected_task_version:current.task.version,expected_routing_policy_version:2,
            idempotency_key:format!("routing:busy:{}",current.task.version) }).await.is_err());
        assert_eq!(f.journal_snapshot().await,before);
    }
    let stop=LifecycleCommand::Reconcile { fence:delivery_fence(&running),never_submitted:None,
        terminal:Some(RuntimeTerminalObservation { assignment_hash:running.assignment.as_ref().unwrap().assignment_hash.clone(),
            chat_id:binding.chat_id,run_id:binding.run_id,state:RuntimeTerminalState::Failed,
            receipt:artifact("routing-known-terminal") }),recovery:RecoveryDisposition::Stop,idempotency_key:"routing:stop".into() };
    let stopped=f.repo.lifecycle_command(f.task,&f.scheduler,stop.clone()).await.unwrap().0;
    assert_eq!(stopped.task.runtime,RuntimeState::Terminal);
    let old_history=f.journal_snapshot().await;
    let old_ledger:Json=f.db.query_one(statement("SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS payload FROM seaql_migrations m",vec![]))
        .await.unwrap().unwrap().try_get("","payload").unwrap();
    migration::Migrator::up(&f.db,None).await.unwrap();
    assert_eq!(f.journal_snapshot().await,old_history);
    let old_ledger_after:Json=f.db.query_one(statement(
        "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS payload FROM seaql_migrations m WHERE version<>'m20261114_0000120_routing_resume'",vec![]))
        .await.unwrap().unwrap().try_get("","payload").unwrap();
    assert_eq!(old_ledger_after,old_ledger);
    let mut routes=serde_json::to_value(&f.frozen.policy.routes).unwrap();
    for (role,_) in f.frozen.policy.routes.entries() {
        routes[role]["fleet_config_revision"]=json!(3);
        routes[role]["instruction_manifest_sha256"]=json!("c".repeat(64));
        routes[role]["workflow_catalog_sha256"]=json!("d".repeat(64));
    }
    let policy=f.repo.set_routing_policy(f.frozen.project_id,&f.owner,SetRoutingPolicy {
        expected_version:Some(2),routes:parse(routes),idempotency_key:"routing:current-policy".into() }).await.unwrap().0;
    let command=LifecycleCommand::ResumeWithCurrentRouting { expected_task_version:stopped.task.version,
        expected_routing_policy_version:policy.version,idempotency_key:"routing:resume".into() };
    let before=f.journal_snapshot().await;
    let foreign=actor("foreign-scheduler",false,vec![]);
    for actor in [&f.owner,&f.producers["analyst"],&foreign] {
        assert!(matches!(f.repo.lifecycle_command(f.task,actor,command.clone()).await,Err(AppError::Forbidden)));
        assert_eq!(f.journal_snapshot().await,before);
    }
    for field in ["expected_task_version","expected_routing_policy_version"] {
        let mut value=serde_json::to_value(&command).unwrap();value[field]=json!(1);
        assert!(f.repo.lifecycle_command(f.task,&f.scheduler,parse(value)).await.is_err());
        assert_eq!(f.journal_snapshot().await,before);
    }
    let (resumed,replay)=f.repo.lifecycle_command(f.task,&f.scheduler,command.clone()).await.unwrap();
    assert!(!replay);
    assert_eq!(resumed.task.routing.policy,policy);
    assert_ne!(resumed.task.routing.snapshot_id,f.frozen.snapshot_id);
    assert_eq!(resumed.task.routing.confirmation_id,f.frozen.confirmation_id);
    assert_eq!(resumed.task.routing.requirement_revision,f.frozen.requirement_revision);
    assert_eq!(resumed.task.routing.content_hash,f.frozen.content_hash);
    assert_eq!(resumed.task.stage,stopped.task.stage);
    assert_eq!(resumed.task.cycle,stopped.task.cycle);
    assert_eq!(resumed.task.resume_chat_id,Some(binding.chat_id));
    assert_eq!(resumed.task.previous_result_id,stopped.task.previous_result_id);
    assert_eq!(resumed.task.transient_retries,0);
    assert_eq!(f.repo.lifecycle_command(f.task,&f.scheduler,command.clone()).await.unwrap(),(resumed.clone(),true));
    assert_eq!(f.repo.lifecycle_operation(f.task,&f.reader,command.key()).await.unwrap(),resumed);
    assert_eq!(f.repo.lifecycle_command(f.task,&f.scheduler,stop).await.unwrap(),(stopped.clone(),true));
    assert_eq!(f.repo.lifecycle_command(f.task,&f.scheduler,reserve).await.unwrap(),(reserved.clone(),true));
    let snapshot=f.repo.task_routing_snapshot(f.task,&f.owner).await.unwrap();
    assert_eq!(snapshot,resumed.task.routing);
    let candidates=f.repo.delivery_candidates(&f.reader,DeliveryCandidatesQuery {
        project_id:f.frozen.project_id,after:None,limit:10 }).await.unwrap();
    assert_eq!(candidates.items.len(),1);
    assert_eq!(candidates.items[0].next_operation,CandidateOperation::Reserve);
    let mut next_reserve=LifecycleCommand::Reserve { expected_task_version:resumed.task.version,
        machine_subject:Some(f.producers["analyst"].subject.clone()),idempotency_key:"routing:new-reserve".into() };
    let mut new_reserved=f.repo.lifecycle_command(f.task,&f.scheduler,next_reserve.clone()).await.unwrap().0;
    assert_eq!(new_reserved.assignment.as_ref().unwrap().routing,resumed.task.routing);
    assert_eq!(new_reserved.assignment.as_ref().unwrap().agent_id,policy.routes.analyst.agent_id);
    assert_ne!(new_reserved.assignment.as_ref().unwrap().assignment_id,reserved.assignment.as_ref().unwrap().assignment_id);
    let source=f.repo.task_source(f.task,&f.producers["analyst"]).await.unwrap();
    let source_value=serde_json::to_value(&source).unwrap();
    assert_eq!(source_value["binding"],old_source);
    let retained:Json=f.db.query_one(statement("SELECT payload FROM sdlc_task_source_bindings WHERE task_id=$1",
        vec![f.task.into()])).await.unwrap().unwrap().try_get("","payload").unwrap();
    assert_eq!(retained,old_source);
    // More deliberate interventions than the automatic retry budget remain valid.
    for index in 0..=MAX_TRANSIENT_RETRIES + 1 {
        let assignment=new_reserved.assignment.as_ref().unwrap();
        let stop=LifecycleCommand::Reconcile { fence:delivery_fence(&new_reserved),terminal:None,
            never_submitted:Some(NeverSubmittedObservation { assignment_hash:assignment.assignment_hash.clone(),
                reservation_key:next_reserve.key().into(),reservation_payload_sha256:app::sdlc::canonical_hash(&next_reserve).unwrap() }),
            recovery:RecoveryDisposition::Stop,idempotency_key:format!("routing:manual-stop:{index}") };
        f.repo.lifecycle_command(f.task,&f.scheduler,stop).await.unwrap();
        let queue=LifecycleCommand::EnsureQueued { idempotency_key:format!("routing:manual-queue:{index}") };
        let (next,replayed)=f.repo.lifecycle_command(f.task,&f.scheduler,queue.clone()).await.unwrap();
        assert!(!replayed);assert_eq!(next.task.transient_retries,0);
        assert_eq!(f.repo.lifecycle_command(f.task,&f.scheduler,queue).await.unwrap(),(next.clone(),true));
        next_reserve=LifecycleCommand::Reserve { expected_task_version:next.task.version,
            machine_subject:Some(f.producers["analyst"].subject.clone()),idempotency_key:format!("routing:manual-reserve:{index}") };
        new_reserved=f.repo.lifecycle_command(f.task,&f.scheduler,next_reserve.clone()).await.unwrap().0;
        assert_eq!(new_reserved.assignment.as_ref().unwrap().attempt,1);
    }
    assert_eq!(pm_history(&f.db,f.task).await,f.pm_history);
    assert_eq!(count(&f.db,"sdlc_task_routing_snapshots").await,1);
    assert_eq!(count(&f.db,"sdlc_task_routing_generations").await,1);
    println!("TRACKER_ROUTING_RESUME_120=closed_current_policy_new_assignment_original_source_and_history_preserved");
}

impl ReworkFixture {
    async fn new() -> (Self, LifecycleReceipt) {
        Self::with_draft_protocol(None, None, "owner", "scheduler",
            Some(InteractionProtocol::ConfiguredCliV1), None, false, false).await
    }

    async fn configured_before_persistent_ownership() -> (Self, LifecycleReceipt) {
        Self::with_draft_protocol(
            None,
            None,
            "owner",
            "scheduler",
            Some(InteractionProtocol::ConfiguredCliV1),
            Some(migration::InstalledOwnershipMigrator::migrations().iter().position(|step|
                step.name() == "m20261112_0000117_persistent_delivery_ownership").unwrap().try_into().unwrap()),
            true,
            false,
        )
        .await
    }

    async fn with_architect_inputs(
        source: Option<domain::sdlc_source::VerifiedSource>,
        document: Option<RequirementsDocument>,
    ) -> (Self, LifecycleReceipt) {
        Self::with_architect_owner_inputs(source, document, "owner", "scheduler").await
    }

    async fn with_architect_owner_inputs(
        source: Option<domain::sdlc_source::VerifiedSource>,
        document: Option<RequirementsDocument>,
        owner_subject: &str,
        scheduler_subject: &str,
    ) -> (Self, LifecycleReceipt) {
        Self::with_schema_tail(source, document, owner_subject, scheduler_subject, None).await
    }

    async fn with_schema_tail(
        source: Option<domain::sdlc_source::VerifiedSource>,
        document: Option<RequirementsDocument>,
        owner_subject: &str,
        scheduler_subject: &str,
        tail: Option<&str>,
    ) -> (Self, LifecycleReceipt) {
        let name = tail.unwrap_or("m20261108_0000116_reviewer_terminal");
        let registered = migration::Migrator::migrations();
        let steps = registered.iter().position(|m| m.name() == name).unwrap() + 1;
        Self::with_draft_protocol(source, document, owner_subject, scheduler_subject,
            Some(InteractionProtocol::ConfiguredCliV1), Some(steps.try_into().unwrap()), false, tail.is_none()).await
    }

    async fn with_draft_protocol(
        source: Option<domain::sdlc_source::VerifiedSource>,
        document: Option<RequirementsDocument>,
        owner_subject: &str,
        scheduler_subject: &str,
        interaction_protocol: Option<InteractionProtocol>,
        migration_steps: Option<u32>,
        installed_ownership: bool,
        upgrade_after_pm: bool,
    ) -> (Self, LifecycleReceipt) {
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
        if installed_ownership {
            migration::InstalledOwnershipMigrator::up(&db, migration_steps).await.unwrap();
        } else {
            migration::Migrator::up(&db, migration_steps).await.unwrap();
        }
        let owner_id = Uuid::new_v4();
        let project = Uuid::new_v4();
        for (subject, id) in [
            (owner_subject, owner_id),
            ("pm", Uuid::new_v4()),
            (scheduler_subject, Uuid::new_v4()),
            ("verifier", Uuid::new_v4()),
        ] {
            exec(&db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,$3,'!',$3,true)",
                vec![id.into(),format!("rework-{subject}@example.test").into(),subject.into()]).await.unwrap();
        }
        exec(&db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'RW','One-task rework',$2,$3)",
            vec![project.into(),owner_id.into(),Uuid::new_v4().into()]).await.unwrap();
        exec(&db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Rework','[]' FROM projects WHERE id=$1",
            vec![project.into()]).await.unwrap();
        exec(&db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub<>$2",
            vec![project.into(), owner_subject.into()]).await.unwrap();
        let repo = PostgresSdlcRepository::connect(
            &url,
            SdlcConfig {
                instance_id: "single-task-rework-test".into(),
                orchestrator_subject: "pm".into(),
                verifier_subject: "verifier".into(),
                reservation_scheduler_subject: scheduler_subject.into(),
            },
        )
        .await
        .unwrap();
        let owner = actor(owner_subject, true, vec![]);
        crate::sdlc::source::configure_test_source_verified(&repo, project, &owner, source).await;
        let scheduler = actor(scheduler_subject, false, vec![]);
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
                    interaction_protocol,
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
        let automatic = complete_initial_pm(
            &repo,
            task,
            &owner,
            &reader,
            routes.project_manager.agent_id,
            document,
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
        assert_eq!(queued, automatic);
        assert_eq!(queued.task.stage, LifecycleStage::Analysis);
        let frozen = queued.task.routing.clone();
        assert_eq!(frozen.policy.version, 1);
        assert_eq!(frozen.policy.routes, routes);
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
        assert_eq!(
            pm_history["configured_completions"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        // Upgrade populated PM/history/routing data, without rewriting old receipts.
        if upgrade_after_pm {
            migration::Migrator::up(&db, None).await.unwrap();
        }
        assert_eq!(self::pm_history(&db, task).await, pm_history);
        let mut producers = std::collections::HashMap::new();
        for role in [
            "analyst",
            "architect",
            "developer",
            "reviewer",
            "tester",
            "devops",
        ] {
            producers.insert(
                role.into(),
                crate::sdlc::configured_test_support::service(&repo, project).await,
            );
        }
        (
            Self {
                db,
                boards: crate::repos::SeaOrmRepositories::new(
                    Database::connect(&url).await.unwrap(),
                )
                .boards,
                repo,
                task,
                owner,
                scheduler,
                reader,
                frozen,
                pm_history,
                producers,
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
        let board = self
            .boards
            .sdlc_tasks(
                shared::ProjectId::from_uuid(canonical.project_id),
                &[shared::IssueId::from_uuid(self.task)],
            )
            .await
            .unwrap();
        assert_eq!(board.len(), 1);
        assert_eq!(board[0].stage, expected.task.stage);
        assert_eq!(board[0].status, Some(expected.task.status));
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
        self.step_with_released_review(queued, output, index, false)
            .await
    }

    async fn step_with_released_review(
        &self,
        queued: &LifecycleReceipt,
        output: DeliveryResult,
        index: usize,
        released_review: bool,
    ) -> OriginalStep {
        assert_eq!(queued.task.status, LifecycleStatus::Ready);
        assert_eq!(queued.task.runtime, RuntimeState::Queued);
        assert!(queued.assignment.is_none());
        assert!(queued.lease.is_none());
        assert!(queued.binding.is_none());
        self.assert_current(queued).await;
        let producer = self.producers[output.role().key()].clone();
        let reserve = LifecycleCommand::Reserve {
            expected_task_version: queued.task.version,
            machine_subject: Some(producer.subject.clone()),
            idempotency_key: format!("rework:{index}:reserve"),
        };
        let (reserved, replayed) = self
            .repo
            .lifecycle_command(self.task, &self.scheduler, reserve.clone())
            .await
            .unwrap();
        assert!(!replayed);
        self.finish_reserved(queued, output, index, reserve, reserved, released_review).await
    }

    async fn finish_reserved(
        &self,
        queued: &LifecycleReceipt,
        output: DeliveryResult,
        index: usize,
        reserve: LifecycleCommand,
        reserved: LifecycleReceipt,
        released_review: bool,
    ) -> OriginalStep {
        let producer = self.producers[output.role().key()].clone();
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
            machine_subject: producer.subject.clone(),
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
        assert_eq!(running.task.status, LifecycleStatus::Ready);
        assert_eq!(running.task.runtime, RuntimeState::Running);
        assert_eq!(running.task.stage, queued.task.stage);
        assert_eq!(running.task.cycle, queued.task.cycle);
        assert_eq!(running.task.version, reserved.task.version + 1);
        self.assert_current(&running).await;
        let (start, started_receipt, started) = start_delivery(
            &self.repo,
            self.task,
            &producer,
            &running,
            &output,
            &format!("rework:{index}:start"),
        )
        .await;
        self.assert_current(&started).await;
        let output = if matches!(output, DeliveryResult::Architect { .. }) {
            DeliveryResult::Architect {
                design: crate::sdlc::configured_test_support::save_architect_plan(
                    &self.repo, self.task, &producer, &started, &format!("rework:{index}:plan"),
                ).await,
            }
        } else { output };
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
            assert_eq!(&actual.result, previous);
        }
        let proposal_command = SubmitDeliveryProposal {
            fence: fence.clone(),
            workflow_receipt: artifact(&format!("rework-{index}-workflow")),
            result: output.clone(),
            idempotency_key: format!("rework:{index}:proposal"),
        };
        let foreign_role = if output.role().key() == "analyst" {
            "architect"
        } else {
            "analyst"
        };
        let foreign_actor = self.producers[foreign_role].clone();
        assert!(matches!(
            self.repo
                .submit_delivery_proposal(self.task, &foreign_actor, proposal_command.clone())
                .await,
            Err(AppError::Forbidden)
        ));
        crate::sdlc::configured_test_support::delivery_comment(
            &self.repo,
            self.task,
            &producer,
            assignment,
            &format!("rework:{index}:final-comment"),
        )
        .await;
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
        // Proposal records the agent's Completed, not a handoff or native terminal result.
        let proposed = self
            .repo
            .lifecycle_read(self.task, &self.reader)
            .await
            .unwrap()
            .receipt;
        assert_eq!(proposed.task.stage, started.task.stage);
        assert_eq!(proposed.task.cycle, started.task.cycle);
        assert_eq!(proposed.task.status, LifecycleStatus::Completed);
        assert_eq!(proposed.task.runtime, RuntimeState::Running);
        assert_eq!(proposed.task.version, started.task.version + 1);
        assert_eq!(proposed.assignment, started.assignment);
        assert_eq!(proposed.lease, started.lease);
        assert_eq!(proposed.binding, started.binding);
        assert!(proposed.result.is_none());
        self.assert_current(&proposed).await;
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
        if matches!(output, DeliveryResult::Reviewer { .. }) {
            let LifecycleCommand::Complete { terminal, .. } = &complete else {
                unreachable!()
            };
            for next in ["Review", "Rework"] {
                let result_id = Uuid::new_v4();
                let invalid = json!({"result_id":result_id,"assignment_id":assignment.assignment_id,
                    "task_id":self.task,"terminal":terminal,"output":output,
                    "previous_stage":"Review","next_stage":next,"next_status":"Ready",
                    "cycle":proposed.task.cycle,"created_at":Utc::now()});
                let rejected = self.db.execute(statement(
                    "INSERT INTO sdlc_delivery_results(result_id,assignment_id,task_id,payload) VALUES($1,$2,$3,$4)",
                    vec![result_id.into(),assignment.assignment_id.into(),self.task.into(),invalid.into()],
                )).await.unwrap_err();
                assert!(
                    rejected
                        .to_string()
                        .contains("SDLC result requires exact live bound execution")
                );
            }
        }
        assert!(matches!(
            self.repo
                .lifecycle_command(self.task, &producer, complete.clone())
                .await,
            Err(AppError::Forbidden)
        ));
        let (completed, replayed) = if released_review {
            (
                reviewer_handoff_tests::complete_released_review(self, &proposed, &complete).await,
                false,
            )
        } else {
            self.repo
                .lifecycle_command(self.task, &self.scheduler, complete.clone())
                .await
                .unwrap_or_else(|error| panic!("Complete step {index} failed: {error}"))
        };
        assert!(!replayed);
        assert_eq!(completed.task.version, proposed.task.version + 1);
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
            start,
            started: started_receipt,
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
             'idempotency',(SELECT jsonb_agg(to_jsonb(o) ORDER BY actor_subject,idempotency_key) FROM sdlc_idempotency o WHERE task_id=$1),
             'results',(SELECT jsonb_agg(to_jsonb(r) ORDER BY result_id) FROM sdlc_delivery_results r WHERE task_id=$1),
             'operations',(SELECT jsonb_agg(to_jsonb(o) ORDER BY idempotency_key) FROM sdlc_delivery_operations o WHERE task_id=$1),
             'outbox',(SELECT jsonb_agg(to_jsonb(o) ORDER BY sequence) FROM sdlc_outbox o WHERE task_id=$1),
             'status_history',(SELECT jsonb_agg(to_jsonb(h) ORDER BY id) FROM issue_status_history h WHERE issue_id=$1)) AS snapshot",
            vec![self.task.into()],
        )).await.unwrap().unwrap().try_get("","snapshot").unwrap()
    }

    async fn assert_original_replays(&self, original: &OriginalStep) {
        assert_eq!(
            self.repo
                .stage_start_operation(
                    self.task,
                    &original.producer,
                    &original.start.idempotency_key,
                )
                .await
                .unwrap(),
            original.started
        );
        assert_eq!(
            self.repo
                .start_stage(self.task, &original.producer, original.start.clone(),)
                .await
                .unwrap(),
            (original.started.clone(), true)
        );
        let mut changed_start = original.start.clone();
        let StageStartFence::Delivery(fence) = &mut changed_start.fence else {
            unreachable!()
        };
        fence.expected_lease_version += 1;
        assert!(matches!(
            self.repo
                .start_stage(self.task, &original.producer, changed_start,)
                .await,
            Err(AppError::Conflict(_))
        ));
        let mut stale_start = original.start.clone();
        stale_start.idempotency_key.push_str(":after-handoff");
        assert!(
            self.repo
                .start_stage(self.task, &original.producer, stale_start)
                .await
                .is_err()
        );
        let expected_result = original.completed.result.as_ref().unwrap();
        let DeliveryInput::Delivery(actual_result) = self
            .repo
            .lifecycle_result(self.task, &self.reader, expected_result.result_id)
            .await
            .unwrap()
        else {
            panic!("expected retained delivery result");
        };
        assert_eq!(&actual_result.result, expected_result);
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

async fn through_review(
    fixture: &ReworkFixture,
    mut cursor: LifecycleReceipt,
) -> (LifecycleReceipt, Vec<OriginalStep>) {
    let mut originals = Vec::new();
    for (index, output) in [
        DeliveryResult::Analyst {
            analysis: artifact("review-analysis"),
        },
        DeliveryResult::Architect {
            design: artifact("review-design"),
        },
        developer(),
    ]
    .into_iter()
    .enumerate()
    {
        let original = fixture.step(&cursor, output, index).await;
        cursor = original.completed.clone();
        originals.push(original);
    }
    assert_eq!(cursor.task.stage, LifecycleStage::Review);
    (cursor, originals)
}

async fn assert_linear_delivery(verdict: ReviewVerdict, testing: TestVerdict) {
    let (fixture, queued) = ReworkFixture::new().await;
    let (mut cursor, mut originals) = through_review(&fixture, queued).await;
    let queued_before: i64 = fixture.db.query_one(statement(
        "SELECT count(*) AS n FROM sdlc_outbox WHERE task_id=$1 AND event_type='delivery.queued'",
        vec![fixture.task.into()],
    )).await.unwrap().unwrap().try_get("", "n").unwrap();
    let original = fixture.step(&cursor, reviewer(verdict), 3).await;
    cursor = original.completed.clone();
    originals.push(original);
    assert_eq!(cursor.task.stage, LifecycleStage::Testing);
    assert_eq!(cursor.task.status, LifecycleStatus::Ready);
    assert_eq!(cursor.task.runtime, RuntimeState::Queued);
    assert_eq!(
        cursor.task.waiting_reason.as_deref(),
        Some("queued_for_assignment")
    );
    assert_eq!(cursor.task.cycle, 0);
    assert!(cursor.assignment.is_none() && cursor.lease.is_none() && cursor.binding.is_none());
    let tested = fixture
        .step(
            &cursor,
            DeliveryResult::Tester {
                verdict: testing,
                report: artifact("linear-test-report"),
            },
            4,
        )
        .await;
    cursor = tested.completed.clone();
    originals.push(tested);
    assert_eq!(cursor.task.stage, LifecycleStage::Deployment);
    assert_eq!(cursor.task.status, LifecycleStatus::Ready);
    assert_eq!(cursor.task.runtime, RuntimeState::Queued);
    assert_eq!(cursor.task.cycle, 0);
    let deployed = fixture
        .step(
            &cursor,
            DeliveryResult::DevOps {
                deployment: artifact("linear-deployment"),
                verification: artifact("linear-verification"),
            },
            5,
        )
        .await;
    cursor = deployed.completed.clone();
    originals.push(deployed);
    assert_eq!(cursor.task.stage, LifecycleStage::Deployment);
    assert_eq!(cursor.task.status, LifecycleStatus::Completed);
    assert_eq!(cursor.task.runtime, RuntimeState::Terminal);
    assert_eq!(cursor.task.waiting_reason, None);
    assert_eq!(cursor.task.cycle, 0);
    assert!(cursor.assignment.is_none() && cursor.lease.is_none() && cursor.binding.is_none());
    let queued_after: i64 = fixture.db.query_one(statement(
        "SELECT count(*) AS n FROM sdlc_outbox WHERE task_id=$1 AND event_type='delivery.queued'",
        vec![fixture.task.into()],
    )).await.unwrap().unwrap().try_get("", "n").unwrap();
    assert_eq!(queued_after, queued_before);
    let accepted_events: i64 = fixture.db.query_one(statement(
        "SELECT count(*) AS n FROM sdlc_outbox WHERE task_id=$1 AND event_type='delivery.result_accepted'",
        vec![fixture.task.into()],
    )).await.unwrap().unwrap().try_get("", "n").unwrap();
    assert_eq!(accepted_events, i64::try_from(originals.len()).unwrap());
    assert_settled_replays(&fixture, &cursor, &originals).await;
}

async fn assert_settled_replays(
    fixture: &ReworkFixture,
    cursor: &LifecycleReceipt,
    originals: &[OriginalStep],
) {
    let settled = fixture.journal_snapshot().await;
    // A fresh repository instance reads durable state and replays the original operation.
    let restarted = PostgresSdlcRepository::connect(
        &std::env::var("TT_LIFECYCLE_TEST_DATABASE_URL").unwrap(),
        fixture.repo.config.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        restarted
            .lifecycle_read(fixture.task, &fixture.reader)
            .await
            .unwrap()
            .receipt,
        *cursor
    );
    for original in originals {
        fixture.assert_original_replays(original).await;
    }
    for role in ["reviewer", "developer", "tester", "devops"] {
        assert!(matches!(
            restarted
                .lifecycle_command(
                    fixture.task,
                    &fixture.scheduler,
                    LifecycleCommand::Reserve {
                        expected_task_version: cursor.task.version,
                        machine_subject: Some(fixture.producers[role].subject.clone()),
                        idempotency_key: format!("review:after-completed:{role}"),
                    }
                )
                .await,
            Err(AppError::Conflict(_))
        ));
    }
    assert_eq!(fixture.journal_snapshot().await, settled);
    fixture.assert_current(cursor).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn review_and_test_findings_advance_to_devops_without_rework() {
    assert_linear_delivery(ReviewVerdict::Rework, TestVerdict::Rework).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn approved_review_and_passed_testing_finish_after_devops_and_replay() {
    assert_linear_delivery(ReviewVerdict::Approved, TestVerdict::Passed).await;
}
