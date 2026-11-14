//! Existing result/proposal transactions with synthetic immutable runtime proof.
//! No live Workflow/Fleet execution, attachment bytes or business acceptance claimed.
use super::*;

#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn successful_terminal_finalizes_original_result_without_restoring_assignment() {
    let steps = migration::InstalledOwnershipMigrator::migrations().iter()
        .position(|step| step.name() == "m20261114_0000121_explicit_recovery").unwrap() + 1;
    let (mut f, mut queued) = ReworkFixture::with_draft_protocol(None, None, "owner", "scheduler",
        Some(InteractionProtocol::ConfiguredCliV1), Some(steps.try_into().unwrap()), true, false).await;
    for (index, outcome) in ["success", "failed", "cancelled", "newer_head", "newer_assignment", "race"].into_iter().enumerate() {
        if index != 0 {
            let project = queued.task.routing.project_id;
            let policy = f.repo.routing_policy(project, &f.owner, None).await.unwrap();
            let draft = f.repo.create_draft(project, &f.owner, CreateDraftCommand {
                interaction_protocol: Some(InteractionProtocol::ConfiguredCliV1),
                flow: Some(DraftFlow::SingleTaskV1), title: format!("Terminal result {outcome}"),
                description: "Synthetic original-custody transaction fixture".into(),
                idempotency_key: format!("terminal:{outcome}:draft"),
            }).await.unwrap().0;
            f.task = draft.task_id;
            queued = complete_initial_pm(&f.repo, f.task, &f.owner, &f.reader,
                policy.routes.project_manager.agent_id, None).await;
        }
        let producer = f.producers["analyst"].clone();
        let reserve = LifecycleCommand::Reserve {
            expected_task_version: queued.task.version, machine_subject: Some(producer.subject.clone()),
            idempotency_key: format!("terminal:{outcome}:reserve"),
        };
        let reserved = f.repo.lifecycle_command(f.task, &f.scheduler, reserve.clone()).await.unwrap().0;
        let fence = delivery_fence(&reserved);
        let assignment = reserved.assignment.as_ref().unwrap().clone();
        let binding = RunBinding { chat_id: Uuid::new_v4(), run_id: Uuid::new_v4(),
            machine_subject: producer.subject.clone(), dispatch_receipt: artifact("terminal-dispatch") };
        let bind = LifecycleCommand::BindRun { fence: fence.clone(), binding: binding.clone(),
            idempotency_key: format!("terminal:{outcome}:bind") };
        let bound = f.repo.lifecycle_command(f.task, &f.scheduler, bind.clone()).await.unwrap().0;
        let output = DeliveryResult::Analyst { analysis: artifact("original-analysis") };
        f.repo.start_stage(f.task, &producer, StartStage {
            fence: StageStartFence::Delivery(fence.clone()),
            idempotency_key: format!("terminal:{outcome}:start"),
        }).await.unwrap();
        let started = f.repo.lifecycle_read(f.task, &f.reader).await.unwrap().receipt;
        assert_eq!(started.task.status, LifecycleStatus::InProgress);
        crate::sdlc::configured_test_support::delivery_comment(
            &f.repo, f.task, &producer, &assignment, &format!("terminal:{outcome}:comment"),
        ).await;
        let observation = RuntimeTerminalObservation {
            assignment_hash: assignment.assignment_hash.clone(), chat_id: binding.chat_id, run_id: binding.run_id,
            state: match outcome { "failed" => RuntimeTerminalState::Failed,
                "cancelled" => RuntimeTerminalState::Cancelled, _ => RuntimeTerminalState::Succeeded },
            receipt: artifact("original-native-terminal"),
        };
        let stop = LifecycleCommand::Reconcile { fence: fence.clone(), terminal: Some(observation.clone()),
            never_submitted: None, recovery: RecoveryDisposition::Stop,
            idempotency_key: format!("fleet-delivery-stop:{}", binding.run_id) };
        let stopped = f.repo.lifecycle_command(f.task, &f.scheduler, stop.clone()).await.unwrap().0;
        assert_eq!(stopped.task.version, started.task.version + 1);
        assert!(stopped.assignment.is_none() && stopped.lease.is_none() && stopped.binding.is_none());
        let original_history = f.journal_snapshot().await;
        if index == 0 {
            let ledger_sql = "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS ledger
                FROM seaql_migrations m WHERE version<>'m20261114_0000122_terminal_result_custody'";
            let old_ledger: Json = f.db.query_one(statement(ledger_sql, vec![])).await.unwrap().unwrap()
                .try_get("", "ledger").unwrap();
            let reconciliation_sql = "SELECT payload FROM sdlc_delivery_reconciliations WHERE assignment_id=$1";
            let original_reconciliation: Json = f.db.query_one(statement(reconciliation_sql,
                vec![assignment.assignment_id.into()])).await.unwrap().unwrap().try_get("", "payload").unwrap();
            let pm_before = pm_history(&f.db, f.task).await;
            migration::Migrator::up(&f.db, None).await.unwrap();
            assert_eq!(f.journal_snapshot().await, original_history);
            assert_eq!(pm_history(&f.db, f.task).await, pm_before);
            let after: Json = f.db.query_one(statement(ledger_sql, vec![])).await.unwrap().unwrap()
                .try_get("", "ledger").unwrap();
            assert_eq!(after, old_ledger);
            let after: Json = f.db.query_one(statement(reconciliation_sql,
                vec![assignment.assignment_id.into()])).await.unwrap().unwrap().try_get("", "payload").unwrap();
            assert_eq!(after, original_reconciliation);
            migration::Migrator::up(&f.db, None).await.unwrap();
            assert_eq!(f.journal_snapshot().await, original_history);
        }
        let proposal = SubmitDeliveryProposal { fence: fence.clone(), workflow_receipt: artifact("original-workflow"),
            result: output.clone(), idempotency_key: format!("delivery-result:{}", assignment.assignment_id) };
        if outcome == "failed" || outcome == "cancelled" {
            assert!(f.repo.delivery_proposal_context(f.task, &producer, &fence).await.is_err());
            assert!(f.repo.submit_delivery_proposal(f.task, &producer, proposal).await.is_err());
            assert_eq!(f.journal_snapshot().await, original_history);
            continue;
        }
        let context = f.repo.delivery_proposal_context(f.task, &producer, &fence).await.unwrap();
        assert_eq!(context.assignment.as_ref(), Some(&assignment));
        assert_eq!(context.binding.as_ref(), Some(&binding));
        assert_eq!(context.task, stopped.task);
        assert!(f.repo.context(f.task, &producer).await.unwrap().assignment.is_none());
        let foreign = &f.producers["architect"];
        assert!(f.repo.delivery_proposal_context(f.task, foreign, &fence).await.is_err());
        let mut stale = proposal.clone();
        stale.fence.fencing_token += 1;
        assert!(f.repo.submit_delivery_proposal(f.task, &producer, stale).await.is_err());
        assert_eq!(f.journal_snapshot().await, original_history);
        if outcome == "newer_head" || outcome == "newer_assignment" || outcome == "race" {
            let queue = LifecycleCommand::EnsureQueued { idempotency_key: format!("terminal:{outcome}:queue") };
            let next = if outcome == "race" {
                let (submitted, advanced) = tokio::join!(
                    f.repo.submit_delivery_proposal(f.task, &producer, proposal.clone()),
                    f.repo.lifecycle_command(f.task, &f.scheduler, queue));
                // Either lock order is legitimate. No result can be accepted after the head advances.
                if let Ok((receipt, replayed)) = submitted {
                    assert!(!replayed);
                    assert_eq!(receipt.assignment_id, assignment.assignment_id);
                }
                advanced.unwrap().0
            } else { f.repo.lifecycle_command(f.task, &f.scheduler, queue).await.unwrap().0 };
            assert!(next.task.version > stopped.task.version);
            if outcome == "newer_assignment" {
                f.repo.lifecycle_command(f.task, &f.scheduler, LifecycleCommand::Reserve {
                    expected_task_version: next.task.version, machine_subject: Some(producer.subject.clone()),
                    idempotency_key: "terminal:newer:reserve".into(),
                }).await.unwrap();
            }
            let before = f.journal_snapshot().await;
            assert!(f.repo.delivery_proposal_context(f.task, &producer, &fence).await.is_err());
            if outcome != "race" {
                assert!(f.repo.submit_delivery_proposal(f.task, &producer, proposal.clone()).await.is_err());
            }
            let complete = LifecycleCommand::Complete { fence, result: output,
                terminal: TerminalReceipt { chat_id: binding.chat_id,run_id: binding.run_id,
                    fleet_receipt: observation.receipt, workflow_receipt: artifact("original-workflow"),
                    output_sha256: app::sdlc::canonical_hash(&proposal.result).unwrap() },
                idempotency_key: format!("fleet-delivery-complete:{}", binding.run_id) };
            assert!(f.repo.lifecycle_command(f.task, &f.scheduler, complete).await.is_err());
            assert_eq!(f.journal_snapshot().await, before);
            continue;
        }
        let (left, right) = tokio::join!(
            f.repo.submit_delivery_proposal(f.task, &producer, proposal.clone()),
            f.repo.submit_delivery_proposal(f.task, &producer, proposal.clone()));
        let (left, right) = (left.unwrap(), right.unwrap());
        assert_eq!(serde_json::to_value(&left.0).unwrap(), serde_json::to_value(&right.0).unwrap());
        assert_ne!(left.1, right.1);
        assert_eq!(f.repo.lifecycle_read(f.task, &f.reader).await.unwrap().receipt, stopped);
        assert_eq!(serde_json::to_value(f.repo.delivery_proposal_operation(f.task, &f.reader, &proposal.idempotency_key).await.unwrap()).unwrap(),
            serde_json::to_value(&left.0).unwrap());
        let complete = LifecycleCommand::Complete { fence: fence.clone(), result: output,
            terminal: TerminalReceipt { chat_id: binding.chat_id,run_id: binding.run_id,
                fleet_receipt: observation.receipt, workflow_receipt: proposal.workflow_receipt.clone(),
                output_sha256: left.0.output_sha256.clone() },
            idempotency_key: format!("fleet-delivery-complete:{}", binding.run_id) };
        let before_bad = f.journal_snapshot().await;
        let mut forged = complete.clone();
        if let LifecycleCommand::Complete { terminal, .. } = &mut forged {
            terminal.fleet_receipt = artifact("foreign-terminal");
        }
        assert!(f.repo.lifecycle_command(f.task, &f.scheduler, forged).await.is_err());
        assert_eq!(f.journal_snapshot().await, before_bad);
        let (left, right) = tokio::join!(
            f.repo.lifecycle_command(f.task, &f.scheduler, complete.clone()),
            f.repo.lifecycle_command(f.task, &f.scheduler, complete.clone()));
        let (left, right) = (left.unwrap(), right.unwrap());
        assert_eq!(left.0, right.0);
        assert_ne!(left.1, right.1);
        assert_eq!(left.0.task.stage, LifecycleStage::Architecture);
        assert_eq!(left.0.task.runtime, RuntimeState::Queued);
        assert_eq!(left.0.result.as_ref().unwrap().assignment_id, assignment.assignment_id);
        assert!(left.0.assignment.is_none() && left.0.lease.is_none() && left.0.binding.is_none());
        assert!(f.repo.submit_delivery_proposal(f.task, &producer, proposal).await.unwrap().1);
        assert_eq!(f.repo.lifecycle_command(f.task, &f.scheduler, stop).await.unwrap(), (stopped, true));
        assert_eq!(f.repo.lifecycle_command(f.task, &f.scheduler, reserve).await.unwrap(), (reserved, true));
        assert_eq!(f.repo.lifecycle_command(f.task, &f.scheduler, bind).await.unwrap(), (bound, true));
        let settled = f.journal_snapshot().await;
        assert_eq!(settled["assignments"], original_history["assignments"]);
        assert_eq!(settled["bindings"], original_history["bindings"]);
        assert_eq!(settled["holds"], json!([]));
    }
    println!("TRACKER_TERMINAL_RESULT_122=original_success_replayed_no_active_restore_foreign_failed_cancelled_newer_head_and_race_denied");
}
