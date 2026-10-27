//! Child of sdlc_critical_pm_lifecycle_pg_tests; main owns that module registration.
//! One existing fresh-DB fixture. All native/Workflow observations are explicitly synthetic.
use super::*;
use app::sdlc_critical_pm_lifecycle::prepare_critical_pm_completion;
use domain::sdlc_critical_pm_lifecycle::{
    CriticalPmCompletionAllocation, CriticalPmCompletionPlan, CriticalPmExecutionContext,
    CriticalPmLifecycleSource,
};
use domain::sdlc_routing::TaskRoutingSnapshot;
use sea_orm::{DbErr, ExecResult};

struct Packet {
    command: CompletePmDraft,
    hash: String,
    operation_id: Uuid,
    lease: ExecutionLeaseReceipt,
    plan: CriticalPmCompletionPlan,
}

async fn actual_snapshot(r: &ReviewFixture) -> Json {
    let rows=r.f.db.query_one(statement("SELECT jsonb_build_object(
        'cursor',(SELECT to_jsonb(t) FROM sdlc_tasks t WHERE task_id=$1),
        'journal',(SELECT jsonb_agg(to_jsonb(j) ORDER BY operation_id) FROM sdlc_critical_pm_lifecycle_operations j WHERE task_id=$1),
        'generations',(SELECT jsonb_agg(to_jsonb(g) ORDER BY generation) FROM sdlc_task_routing_generations g WHERE task_id=$1),
        'sources',(SELECT jsonb_agg(to_jsonb(s) ORDER BY requirement_revision) FROM sdlc_task_source_bindings s WHERE task_id=$1),
        'completions',(SELECT jsonb_agg(to_jsonb(c) ORDER BY completion_id) FROM sdlc_pm_completions c WHERE task_id=$1),
        'completion_operations',(SELECT jsonb_agg(to_jsonb(o) ORDER BY idempotency_key) FROM sdlc_pm_completion_operations o WHERE task_id=$1),
        'intents',(SELECT jsonb_agg(to_jsonb(i) ORDER BY intent_id) FROM sdlc_analysis_intents i WHERE task_id=$1),
        'blocker_heads',(SELECT jsonb_agg(to_jsonb(h) ORDER BY task_id) FROM sdlc_critical_blocker_heads h WHERE task_id=$1)) AS value",
        vec![r.task.into()])).await.unwrap().unwrap().try_get::<Json>("","value").unwrap();
    json!({"rows":rows,"existing":r.f.snapshot().await,"frozen":frozen_history(&r.f,r.task).await})
}

async fn prepared(
    r: &ReviewFixture,
    tx: &DatabaseTransaction,
    c: &CompletePmDraft,
    snapshot_id: Uuid,
) -> Packet {
    let state = r.f.repo.load(tx, r.task, &r.f.writer).await.unwrap();
    let lifecycle = crate::sdlc::lifecycle::current(tx, r.task)
        .await
        .unwrap()
        .unwrap();
    let lease = PostgresSdlcRepository::lease_current(tx, &r.review.reservation)
        .await
        .unwrap()
        .unwrap();
    let blocker: CriticalBlockerReceipt = parse(
        tx.query_one(statement(
            "SELECT sdlc_critical_pm_detail($1) AS payload",
            vec![r.review.context.blocker_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "payload")
        .unwrap(),
    );
    let initial_pm: PmAssignment = parse(
        tx.query_one(statement(
            "SELECT payload FROM sdlc_assignments WHERE task_id=$1 ORDER BY version LIMIT 1",
            vec![r.task.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "payload")
        .unwrap(),
    );
    let initial_routing: TaskRoutingSnapshot = parse(
        tx.query_one(statement(
            "SELECT payload FROM sdlc_task_routing_snapshots WHERE task_id=$1",
            vec![r.task.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "payload")
        .unwrap(),
    );
    // Same prepared-receipt boundary as the existing parent fixture, never a committed past completion.
    let receipt = PmDraftCompletionReceipt {
        contract_version: 1,
        completion_id: Uuid::new_v4(),
        tracker_instance_id: state.tracker_instance_id.clone(),
        project_id: state.project_id,
        task_id: r.task,
        root_task_id: r.task,
        owner_subject: state.owner_subject.clone(),
        fence: c.fence.clone(),
        owner_version: r.review.reservation.owner_cas.version,
        lease_id: c.lease_id,
        lease_version: c.lease_version,
        requirement_revision: c.requirement_revision,
        content_hash: c.content_hash.clone(),
        terminal_receipt_ref: c.terminal_receipt_ref.clone(),
        terminal_receipt_sha256: c.terminal_receipt_sha256.clone(),
        workflow_receipt_ref: c.workflow_receipt_ref.clone(),
        workflow_receipt_sha256: c.workflow_receipt_sha256.clone(),
        previous_pm_admission_state: "reserved".into(),
        next_pm_admission_state: "completed".into(),
        idempotency_key: c.idempotency_key.clone(),
        created_at: crate::sdlc::execution_lease::clock(tx).await.unwrap(),
    };
    let plan = prepare_critical_pm_completion(
        CriticalPmLifecycleSource {
            state: &state,
            lifecycle: &lifecycle,
            review: &r.review,
            blocker: &blocker,
            initial_pm_assignment: &initial_pm,
            initial_routing: &initial_routing,
            execution: CriticalPmExecutionContext::Finalizing {
                lease: &lease,
                completion: &receipt,
            },
        },
        &r.f.writer,
        &r.f.repo.config,
        c,
        CriticalPmCompletionAllocation {
            snapshot_id,
            analysis_intent_id: Uuid::new_v4(),
        },
    )
    .unwrap();
    Packet {
        command: c.clone(),
        hash: app::sdlc::canonical_hash(&json!({"operation":"complete_pm_draft","payload":c}))
            .unwrap(),
        operation_id: Uuid::new_v4(),
        lease,
        plan,
    }
}

fn head_json(receipt: &LifecycleReceipt) -> Json {
    let mut value = json!(receipt);
    value.as_object_mut().unwrap().remove("task");
    value
}

// Deliberately partial packets below use actual locked source + the production pure planner.
// No lease/current projection is seeded; all partial writes must roll back under real SQL guards.
async fn insert_journal(tx: &DatabaseTransaction, r: &ReviewFixture, p: &Packet) {
    let b = &p.plan.before;
    let a = &p.plan.after;
    let c = &p.plan.completion;
    exec(tx,"INSERT INTO sdlc_critical_pm_lifecycle_operations(operation_id,task_id,blocker_id,execution_id,owner_version,
        actor_subject,kind,idempotency_key,request_sha256,command,result,lease_id,lease_version,
        before_state,after_state,before_execution,after_execution,before_blocker,after_blocker,outbox_event_id,completion_id)
        VALUES($1,$2,$3,$4,$5,$6,'complete',$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$19)",
        vec![p.operation_id.into(),r.task.into(),b.blocker.blocker_id.into(),c.fence.execution_id.into(),p.lease.owner_version.into(),
            r.f.writer.subject.clone().into(),p.command.idempotency_key.clone().into(),p.hash.clone().into(),json_value(&p.command).unwrap(),json_value(c).unwrap(),
            c.lease_id.into(),c.lease_version.into(),json_value(&b.state).unwrap(),json_value(&a.state).unwrap(),head_json(&b.lifecycle).into(),
            head_json(&a.lifecycle).into(),json_value(&b.blocker).unwrap(),json_value(&a.blocker).unwrap(),c.completion_id.into()]).await.unwrap();
}

async fn insert_completion(tx: &DatabaseTransaction, p: &Packet) {
    let c = &p.plan.completion;
    exec(tx,"INSERT INTO sdlc_pm_completions(completion_id,task_id,execution_id,assignment_id,assignment_version,lease_id,lease_version,
        requirement_revision,content_hash,terminal_receipt_ref,terminal_receipt_sha256,workflow_receipt_ref,workflow_receipt_sha256,result)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",vec![c.completion_id.into(),c.task_id.into(),c.fence.execution_id.into(),
            c.fence.assignment_id.into(),c.fence.assignment_version.into(),c.lease_id.into(),c.lease_version.into(),c.requirement_revision.into(),
            c.content_hash.clone().into(),c.terminal_receipt_ref.clone().into(),c.terminal_receipt_sha256.clone().into(),c.workflow_receipt_ref.clone().into(),
            c.workflow_receipt_sha256.clone().into(),json_value(c).unwrap()]).await.unwrap();
}

async fn insert_generation(
    tx: &DatabaseTransaction,
    p: &Packet,
    previous: Uuid,
) -> Result<ExecResult, DbErr> {
    let g = &p.plan.routing;
    let s = &g.snapshot;
    tx.execute(statement("INSERT INTO sdlc_task_routing_generations(task_id,generation,snapshot_id,previous_snapshot_id,blocker_id,execution_id,
        completion_id,operation_id,confirmation_id,requirement_revision,content_hash,project_id,policy_version,payload)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",vec![s.task_id.into(),g.generation.into(),s.snapshot_id.into(),previous.into(),
            p.plan.before.blocker.blocker_id.into(),p.plan.completion.fence.execution_id.into(),p.plan.completion.completion_id.into(),p.operation_id.into(),
            s.confirmation_id.into(),s.requirement_revision.into(),s.content_hash.clone().into(),s.project_id.into(),s.policy.version.into(),json_value(s).unwrap()])).await
}

async fn packet_without_outbox(
    tx: &DatabaseTransaction,
    r: &ReviewFixture,
    p: &Packet,
    omitted: &str,
) {
    insert_journal(tx, r, p).await;
    insert_completion(tx, p).await;
    let c = &p.plan.completion;
    let intent = &p.plan.analysis_intent;
    let a = &p.plan.after;
    exec(tx,"INSERT INTO sdlc_pm_completion_operations(task_id,idempotency_key,actor_subject,payload_hash,completion_id,result)
        VALUES($1,$2,$3,$4,$5,$6)",vec![r.task.into(),p.command.idempotency_key.clone().into(),r.f.writer.subject.clone().into(),
            p.hash.clone().into(),c.completion_id.into(),json_value(c).unwrap()]).await.unwrap();
    assert_eq!(
        insert_generation(tx, p, p.plan.routing.previous_snapshot_id)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    exec(tx,"INSERT INTO sdlc_analysis_intents(intent_id,task_id,requirement_revision,content_hash,confirmation_id,operation_key,payload,
        routing_generation,review_routing_snapshot_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",vec![intent.intent_id.into(),r.task.into(),
            intent.requirement_revision.into(),intent.content_hash.clone().into(),intent.confirmation_id.into(),intent.operation_key.clone().into(),
            json_value(intent).unwrap(),p.plan.routing.generation.into(),p.plan.routing.snapshot.snapshot_id.into()]).await.unwrap();
    exec(tx,"INSERT INTO sdlc_delivery_projection_permits(task_id,transaction_id,next_state,next_execution) VALUES($1,pg_current_xact_id(),$2,$3)",
        vec![r.task.into(),json_value(&a.state).unwrap(),head_json(&a.lifecycle).into()]).await.unwrap();
    exec(tx,"INSERT INTO sdlc_pm_state_write_permits(task_id,transaction_id,next_state) VALUES($1,pg_current_xact_id(),$2)",
        vec![r.task.into(),json_value(&a.state).unwrap()]).await.unwrap();
    assert_eq!(
        tx.execute(statement(
            "UPDATE sdlc_tasks SET state=$2,pm_admission_state='completed' WHERE task_id=$1
        AND pm_execution_id=$3 AND pm_owner_version=$4 AND pm_admission_state='reserved'",
            vec![
                r.task.into(),
                json_value(&a.state).unwrap(),
                c.fence.execution_id.into(),
                c.owner_version.into()
            ]
        ))
        .await
        .unwrap()
        .rows_affected(),
        1
    );
    assert_eq!(
        tx.execute(statement(
            "UPDATE sdlc_delivery_heads SET execution=$2 WHERE task_id=$1",
            vec![r.task.into(), head_json(&a.lifecycle).into()]
        ))
        .await
        .unwrap()
        .rows_affected(),
        1
    );
    let status =
        crate::sdlc::lifecycle::mapped_issue_status(tx, a.state.stage, a.lifecycle.task.status)
            .await
            .unwrap();
    crate::sdlc::lifecycle::project_issue_status(tx, r.task, &r.f.writer, status)
        .await
        .unwrap();
    for (id, event, result) in [
        (c.completion_id, "pm.completed", json!(c)),
        (intent.intent_id, "analysis.intent_created", json!(intent)),
    ] {
        if event != omitted {
            exec(tx,"INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,$3,$4)",vec![id.into(),r.task.into(),event.into(),
                json!({"contract_version":1,"tracker_instance_id":a.state.tracker_instance_id,"project_id":a.state.project_id,
                    "task_id":r.task,"root_task_id":r.task,"owner_subject":a.state.owner_subject,"stage":"Analysis",
                    "requirement_revision":c.requirement_revision,"result":result}).into()]).await.unwrap();
        }
    }
}

fn denied(error: DbErr, expected: &str) {
    assert!(
        error.to_string().contains(expected),
        "expected {expected}, got {error}"
    );
}

// Parent completion fixture can reuse this instead of deleting append-only outbox history.
pub(super) async fn incomplete_packets(r: &ReviewFixture, c: &CompletePmDraft) {
    let before = actual_snapshot(r).await;
    {
        let tx = r.f.db.begin().await.unwrap();
        let p = Box::pin(prepared(r, &tx, c, Uuid::new_v4())).await;
        insert_completion(&tx, &p).await;
        denied(
            tx.execute_unprepared(
                "SET CONSTRAINTS sdlc_critical_pm_completion_commit_gate IMMEDIATE",
            )
            .await
            .unwrap_err(),
            "SDLC review completion requires same-transaction resolution/generation journal",
        );
        tx.rollback().await.unwrap();
    }
    assert_eq!(actual_snapshot(r).await, before);
    {
        let tx = r.f.db.begin().await.unwrap();
        let p = Box::pin(prepared(r, &tx, c, Uuid::new_v4())).await;
        insert_journal(&tx, r, &p).await;
        denied(tx.execute_unprepared("SET CONSTRAINTS sdlc_critical_pm_lifecycle_operations_completion_id_fkey IMMEDIATE").await.unwrap_err(),
            "sdlc_critical_pm_lifecycle_operations_completion_id_fkey");
        tx.rollback().await.unwrap();
    }
    assert_eq!(actual_snapshot(r).await, before);
    for omitted in ["pm.completed", "analysis.intent_created"] {
        let tx = r.f.db.begin().await.unwrap();
        let p = Box::pin(prepared(r, &tx, c, Uuid::new_v4())).await;
        Box::pin(packet_without_outbox(&tx, r, &p, omitted)).await;
        let expected = if omitted == "pm.completed" {
            "SDLC review operation must commit complete exact projection/outbox"
        } else {
            "SDLC review actual original ledger/effects missing"
        };
        denied(
            tx.execute_unprepared(
                "SET CONSTRAINTS sdlc_critical_pm_lifecycle_commit_gate IMMEDIATE",
            )
            .await
            .unwrap_err(),
            expected,
        );
        tx.rollback().await.unwrap();
        assert_eq!(actual_snapshot(r).await, before);
    }
}

async fn generation_insert_negatives(r: &ReviewFixture, c: &CompletePmDraft, donor: Uuid) {
    let before = actual_snapshot(r).await;
    {
        let tx = r.f.db.begin().await.unwrap();
        let p = Box::pin(prepared(r, &tx, c, Uuid::new_v4())).await;
        insert_journal(&tx, r, &p).await;
        insert_completion(&tx, &p).await;
        tx.execute_unprepared("SAVEPOINT wrong_previous")
            .await
            .unwrap();
        denied(
            insert_generation(&tx, &p, donor).await.unwrap_err(),
            "SDLC routing generation requires atomic completion and copied frozen policy",
        );
        tx.execute_unprepared("ROLLBACK TO SAVEPOINT wrong_previous")
            .await
            .unwrap();
        // Positive INSERT control with the SAME real source/journal/receipt proves the builders work.
        assert_eq!(
            insert_generation(&tx, &p, p.plan.routing.previous_snapshot_id)
                .await
                .unwrap()
                .rows_affected(),
            1
        );
        tx.rollback().await.unwrap();
    }
    assert_eq!(actual_snapshot(r).await, before);
    {
        let tx = r.f.db.begin().await.unwrap();
        // Allocation is otherwise valid to the pure planner; this UUID belongs to another actual initial snapshot.
        let p = Box::pin(prepared(r, &tx, c, donor)).await;
        insert_journal(&tx, r, &p).await;
        insert_completion(&tx, &p).await;
        // This earlier exact generation gate ALSO forbids original-table UUID collisions.
        denied(
            insert_generation(&tx, &p, p.plan.routing.previous_snapshot_id)
                .await
                .unwrap_err(),
            "SDLC routing generation requires atomic completion and copied frozen policy",
        );
        tx.rollback().await.unwrap();
    }
    assert_eq!(actual_snapshot(r).await, before);
}

fn completion_command(r: &ReviewFixture, revision: i64, hash: String) -> CompletePmDraft {
    CompletePmDraft {
        contract_version: 1,
        fence: r.lease.fence.clone(),
        lease_id: r.lease.lease.lease_id,
        lease_version: r.lease.lease.version,
        requirement_revision: revision,
        content_hash: hash,
        terminal_receipt_ref: "fixture://synthetic-current-review-terminal".into(),
        terminal_receipt_sha256: "a".repeat(64),
        workflow_receipt_ref: "fixture://synthetic-current-review-workflow".into(),
        workflow_receipt_sha256: "b".repeat(64),
        idempotency_key: format!("cycles:complete:{revision}"),
    }
}

async fn complete_once(r: &ReviewFixture, c: &CompletePmDraft) -> PmDraftCompletionReceipt {
    let (a, b) = tokio::join!(
        Box::pin(r.f.repo.complete_pm_draft(r.task, &r.f.writer, c.clone())),
        Box::pin(r.f.repo.complete_pm_draft(r.task, &r.f.writer, c.clone()))
    );
    let (a, ar) = a.unwrap();
    let (b, br) = b.unwrap();
    assert_ne!(ar, br);
    assert_eq!(json!(a), json!(b));
    let op =
        r.f.repo
            .pm_draft_completion_operation(r.task, &r.f.reader, &c.idempotency_key)
            .await
            .unwrap();
    assert_eq!(json!(op.result), json!(a));
    let head =
        r.f.repo
            .lifecycle_read(r.task, &r.f.reader)
            .await
            .unwrap()
            .receipt;
    assert_eq!(head.task.task_id, r.task);
    assert_eq!(head.task.stage, LifecycleStage::Analysis);
    assert_eq!(head.task.status, LifecycleStatus::Ready);
    assert_eq!(head.task.runtime, RuntimeState::Queued);
    assert!(
        head.assignment.is_none()
            && head.lease.is_none()
            && head.binding.is_none()
            && head.result.is_none()
    );
    assert_eq!(
        head.task.routing.requirement_revision,
        c.requirement_revision
    );
    let routing =
        r.f.repo
            .task_routing_snapshot(r.task, &r.f.reader)
            .await
            .unwrap();
    assert_eq!(routing, head.task.routing);
    let intent = r.f.repo.analysis_intent(r.task, &r.f.reader).await.unwrap();
    assert_eq!(intent.requirement_revision, c.requirement_revision);
    assert_eq!(intent.content_hash, c.content_hash);
    assert_eq!(intent.confirmation_id, routing.confirmation_id);
    a
}

async fn committed_generation_negatives(r: &ReviewFixture, donor_task: Uuid) {
    let before = actual_snapshot(r).await;
    let foreign_snapshot: Uuid =
        r.f.db
            .query_one(statement(
                "SELECT snapshot_id FROM sdlc_task_routing_snapshots WHERE task_id=$1",
                vec![donor_task.into()],
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "snapshot_id")
            .unwrap();
    for invalid_snapshot in [Uuid::new_v4(), foreign_snapshot] {
        let tx = r.f.db.begin().await.unwrap();
        let error = tx.execute(statement(
            "INSERT INTO sdlc_task_source_bindings(task_id,project_id,confirmation_id,requirement_revision,
                content_hash,routing_snapshot_id,forge_instance_id,repository_id,candidate_ref,
                source_revision,policy_hash,source_hash,payload)
            SELECT task_id,project_id,confirmation_id,requirement_revision,content_hash,$2,
                forge_instance_id,repository_id,candidate_ref,source_revision,policy_hash,source_hash,
                jsonb_set(payload,'{routing_snapshot_id}',to_jsonb($2::uuid))
            FROM sdlc_task_source_bindings WHERE task_id=$1 AND requirement_revision=2",
            vec![r.task.into(), invalid_snapshot.into()],
        )).await.unwrap_err();
        denied(error, "SDLC exact confirmed source binding required");
        tx.rollback().await.unwrap();
        assert_eq!(actual_snapshot(r).await, before);
    }
    let snapshot:Uuid=r.f.db.query_one(statement("SELECT snapshot_id FROM sdlc_task_routing_generations WHERE task_id=$1 AND generation=2",
        vec![r.task.into()])).await.unwrap().unwrap().try_get("","snapshot_id").unwrap();
    {
        let tx = r.f.db.begin().await.unwrap();
        // The reverse collision direction must fail in the collision guard, NOT an unrelated duplicate task PK.
        let error=tx.execute(statement("INSERT INTO sdlc_task_routing_snapshots(snapshot_id,task_id,project_id,policy_version,
            confirmation_id,requirement_revision,content_hash,payload)
            SELECT $2,task_id,project_id,policy_version,confirmation_id,requirement_revision,content_hash,
                jsonb_set(payload,'{snapshot_id}',to_jsonb($2::uuid)) FROM sdlc_task_routing_snapshots WHERE task_id=$1",
            vec![donor_task.into(),snapshot.into()])).await.unwrap_err();
        denied(error, "SDLC routing snapshot UUID collision");
        tx.rollback().await.unwrap();
    }
    assert_eq!(actual_snapshot(r).await, before);
    let initial: Uuid =
        r.f.db
            .query_one(statement(
                "SELECT snapshot_id FROM sdlc_task_routing_snapshots WHERE task_id=$1",
                vec![r.task.into()],
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "snapshot_id")
            .unwrap();
    for (old, review, generation, message) in [
        (
            Some(initial),
            Some(snapshot),
            Some(2_i64),
            "SDLC exactly one routed intent FK required",
        ),
        (None, None, None, "SDLC initial intent routing FK mismatch"),
        (
            None,
            Some(snapshot),
            None,
            "SDLC exactly one routed intent FK required",
        ),
        (
            None,
            None,
            Some(2),
            "SDLC initial intent routing FK mismatch",
        ),
        (
            None,
            Some(snapshot),
            Some(3),
            "SDLC exact intent generation missing",
        ),
        (
            None,
            Some(Uuid::new_v4()),
            Some(2),
            "SDLC exact intent generation missing",
        ),
    ] {
        let tx = r.f.db.begin().await.unwrap();
        let error=tx.execute(statement("INSERT INTO sdlc_analysis_intents(intent_id,task_id,requirement_revision,content_hash,
            confirmation_id,operation_key,payload,routing_snapshot_id,review_routing_snapshot_id,routing_generation)
            SELECT intent_id,task_id,requirement_revision,content_hash,confirmation_id,operation_key,payload,$2,$3,$4
            FROM sdlc_analysis_intents WHERE task_id=$1 AND review_routing_snapshot_id=$5",
            vec![r.task.into(),old.into(),review.into(),generation.into(),snapshot.into()])).await.unwrap_err();
        denied(error, message);
        tx.rollback().await.unwrap();
        assert_eq!(actual_snapshot(r).await, before);
    }
    for sql in [
        "UPDATE sdlc_task_routing_generations SET payload=jsonb_set(payload,'{policy,version}','2'::jsonb) WHERE task_id=$1",
        "UPDATE sdlc_task_routing_generations SET generation=3 WHERE task_id=$1",
        "DELETE FROM sdlc_task_routing_generations WHERE task_id=$1",
        "UPDATE sdlc_task_routing_snapshots SET payload=payload WHERE task_id=$1",
    ] {
        let tx = r.f.db.begin().await.unwrap();
        denied(
            tx.execute(statement(sql, vec![r.task.into()]))
                .await
                .unwrap_err(),
            "SDLC history is append-only",
        );
        tx.rollback().await.unwrap();
        assert_eq!(actual_snapshot(r).await, before);
    }
}

async fn first_archive(r: &ReviewFixture, blocker_id: Uuid) -> Json {
    r.f.db.query_one(statement("SELECT jsonb_build_object(
        'initial',(SELECT to_jsonb(s) FROM sdlc_task_routing_snapshots s WHERE task_id=$1),
        'blocker',(SELECT to_jsonb(b) FROM sdlc_critical_blockers b WHERE task_id=$1 AND blocker_id=$2),
        'return',(SELECT to_jsonb(j) FROM sdlc_critical_pm_returns j WHERE task_id=$1 AND blocker_id=$2),
        'review',(SELECT to_jsonb(j) FROM sdlc_critical_pm_reviews j WHERE task_id=$1 AND blocker_id=$2),
        'journal',(SELECT jsonb_agg(to_jsonb(j) ORDER BY operation_id) FROM sdlc_critical_pm_lifecycle_operations j WHERE task_id=$1 AND blocker_id=$2),
        'generation',(SELECT to_jsonb(g) FROM sdlc_task_routing_generations g WHERE task_id=$1 AND blocker_id=$2)) AS value",
        vec![r.task.into(),blocker_id.into()])).await.unwrap().unwrap().try_get("","value").unwrap()
}

async fn second_review(r: &mut ReviewFixture) {
    let old_pm = r.pm.clone();
    let old_review = r.review.clone();
    let old_lease = r.lease.clone();
    let archive=r.f.db.query_one(statement("SELECT b.payload,j.before_receipt FROM sdlc_critical_blockers b
        JOIN sdlc_critical_pm_returns j USING(task_id,blocker_id) WHERE b.task_id=$1 AND b.blocker_id=$2",
        vec![r.task.into(),old_review.context.blocker_id.into()])).await.unwrap().unwrap();
    let old_blocker: CriticalBlockerReceipt = parse(archive.try_get("", "payload").unwrap());
    let old_head: LifecycleReceipt = parse(archive.try_get("", "before_receipt").unwrap());
    let old_producer = actor(
        &old_blocker.producer_subject,
        false,
        &[
            "task-tracker:read",
            "task-tracker:write",
            &old_head.assignment.as_ref().unwrap().scope(),
        ],
    );
    let candidates =
        r.f.repo
            .delivery_candidates(&r.f.reader, r.f.query(None, 100))
            .await
            .unwrap();
    assert_eq!(
        candidates
            .items
            .iter()
            .filter(|item| item.task_id == r.task)
            .count(),
        1
    );
    assert!(
        candidates
            .items
            .iter()
            .find(|item| item.task_id == r.task)
            .unwrap()
            .task_version
            .is_some()
    );
    let storage = domain::InMemoryStorage::default();
    // Real Reserve/Bind and actual sealed bytes under the generation2 assignment, not a seeded head.
    let head = Box::pin(bind(&r.f, r.task)).await;
    let running = Box::pin(output(&r.f, r.task, head, &storage)).await;
    assert_eq!(running.head.task.task_id, r.task);
    assert_eq!(running.head.task.routing.requirement_revision, 2);
    assert_ne!(
        running.head.assignment.as_ref().unwrap().execution_id,
        old_head.assignment.as_ref().unwrap().execution_id
    );
    let mut body = blocker(&running);
    body.idempotency_key = "cycles:second-blocker".into();
    let (a, b) = tokio::join!(
        Box::pin(r.f.repo.submit_critical_blocker_with_storage(
            r.task,
            &running.producer,
            body.clone(),
            &storage
        )),
        Box::pin(r.f.repo.submit_critical_blocker_with_storage(
            r.task,
            &running.producer,
            body.clone(),
            &storage
        ))
    );
    let (held, ar) = a.unwrap();
    let (same, br) = b.unwrap();
    assert_ne!(ar, br);
    assert_eq!(held, same);
    assert_eq!(held.routing_generation, 2);
    assert_ne!(held.blocker_id, old_blocker.blocker_id);
    let before = actual_snapshot(r).await;
    {
        let tx = r.f.db.begin().await.unwrap();
        denied(
            tx.execute(statement(
                "UPDATE sdlc_critical_blocker_heads SET blocker_id=$2 WHERE task_id=$1",
                vec![r.task.into(), old_blocker.blocker_id.into()],
            ))
            .await
            .unwrap_err(),
            "SDLC only resolved blocker head may CAS to actual current technical proposal",
        );
        tx.rollback().await.unwrap();
    }
    assert_eq!(actual_snapshot(r).await, before);
    let old_read = r
        .f
        .repo
        .critical_blocker_operation(r.task, &old_producer, &old_blocker.proposal.idempotency_key)
        .await
        .unwrap();
    assert_eq!(old_read, old_blocker);
    let (old_replay, replayed) = Box::pin(r.f.repo.submit_critical_blocker_with_storage(
        r.task,
        &old_producer,
        old_blocker.proposal.clone(),
        &storage,
    ))
    .await
    .unwrap();
    assert!(replayed);
    assert_eq!(old_replay, old_blocker);
    assert_eq!(actual_snapshot(r).await, before);
    let current =
        r.f.repo
            .lifecycle_read(r.task, &r.f.reader)
            .await
            .unwrap()
            .receipt;
    let (returned, _) = Box::pin(r.f.repo.return_critical_to_pm_with_terminal(
        r.task,
        &r.f.writer,
        ReturnCriticalToPm {
            review: CriticalPmReviewGuard {
                blocker_id: held.blocker_id,
                blocker_sha256: held.blocker_sha256.clone(),
                expected_blocker_version: held.version,
                expected_task_version: current.task.version,
                fence: fence(&current),
                idempotency_key: "cycles:second-return".into(),
            },
            terminal: RuntimeTerminalObservation {
                assignment_hash: held.assignment_hash.clone(),
                chat_id: held.chat_id,
                run_id: held.run_id,
                state: RuntimeTerminalState::Cancelled,
                receipt: ArtifactReceipt {
                    reference: "fixture://synthetic-second-technical-terminal".into(),
                    sha256: "d".repeat(64),
                },
            },
        },
    ))
    .await
    .unwrap();
    let previous =
        r.f.repo
            .pm_draft_assignment(r.task, &r.f.owner, None)
            .await
            .unwrap()
            .current
            .unwrap();
    let previous_completion =
        r.f.repo
            .pm_draft_completion(r.task, &r.f.reader)
            .await
            .unwrap()
            .current
            .unwrap();
    assert_eq!(previous.assignment, old_review.reservation.assignment);
    assert_eq!(
        previous_completion.fence.execution_id,
        old_review.reservation.assignment.execution_id
    );
    let (review, replay) = Box::pin(r.f.repo.reserve_critical_pm_review(
        r.task,
        &r.f.writer,
        ReserveCriticalPmReview {
            blocker_id: held.blocker_id,
            blocker_sha256: held.blocker_sha256,
            expected_blocker_version: returned.blocker.version,
            expected_task_version: returned.lifecycle.task.version,
            expected_pm_owner_version: previous.owner_cas.version,
            expected_pm_assignment_version: previous.assignment.version,
            idempotency_key: "cycles:second-review".into(),
        },
    ))
    .await
    .unwrap();
    assert!(!replay);
    assert_eq!(review.context.requirements.revision, 2);
    assert_ne!(
        review.reservation.assignment.execution_id,
        previous.assignment.execution_id
    );
    assert_ne!(
        review.reservation.assignment.assignment_id,
        previous.assignment.assignment_id
    );
    assert_eq!(
        review.reservation.assignment.version,
        previous.assignment.version + 1
    );
    assert_eq!(
        review.reservation.owner_cas.version,
        previous.owner_cas.version + 1
    );
    assert!(
        r.f.repo
            .pm_draft_completion(r.task, &r.f.reader)
            .await
            .unwrap()
            .current
            .is_none()
    );
    assert!(matches!(
        r.f.repo
            .claim_execution_lease(
                r.task,
                &old_pm,
                ClaimExecutionLease {
                    expected_owner_version: old_review.reservation.owner_cas.version,
                    fence: old_lease.fence,
                    idempotency_key: "cycles:old-pm-claim".into()
                }
            )
            .await,
        Err(AppError::Forbidden)
    ));
    let pm = actor(
        "pm",
        false,
        &[
            "task-tracker:read",
            "task-tracker:write",
            &review.reservation.assignment.scope(r.task),
        ],
    );
    let assignment = &review.reservation.assignment;
    let (lease, replayed) =
        r.f.repo
            .claim_execution_lease(
                r.task,
                &pm,
                ClaimExecutionLease {
                    expected_owner_version: review.reservation.owner_cas.version,
                    fence: MachineFence {
                        assignment_id: assignment.assignment_id,
                        execution_id: assignment.execution_id,
                        agent_id: assignment.agent_id,
                        assignment_version: assignment.version,
                    },
                    idempotency_key: "cycles:second-claim".into(),
                },
            )
            .await
            .unwrap();
    assert!(!replayed);
    assert_ne!(lease.lease.lease_id, r.lease.lease.lease_id);
    r.review = review;
    r.pm = pm;
    r.lease = lease;
}

async fn revise_second(r: &mut ReviewFixture) -> (String, Confirmation) {
    let q: Question = parse(
        command(
            r,
            &r.pm,
            SdlcCommand::PublishQuestion(PublishQuestion {
                fence: r.lease.fence.clone(),
                request_id: Uuid::new_v4(),
                question_id: Uuid::new_v4(),
                expected_question_version: None,
                requirement_revision: 2,

                requirement_reference: None,
                text: "Second synthetic business decision?".into(),
                rationale: "Second actual review cycle fixture".into(),
                required: true,
                mode: QuestionMode::Text,
                options: vec![],
                recommended_option_id: None,
                idempotency_key: "cycles:second-question".into(),
            }),
        )
        .await,
    );
    let answer: Answer = parse(
        command(
            r,
            &r.f.owner,
            SdlcCommand::Answer {
                question_id: q.id,
                command: AnswerCommand {
                    expected_question_version: q.version,
                    requirement_revision: 2,
                    selected_option_ids: vec![],
                    text: Some("Second synthetic owner decision".into()),
                    comment: None,
                    idempotency_key: "cycles:second-answer".into(),
                },
            },
        )
        .await,
    );
    let (lease, replayed) =
        r.f.repo
            .resume_pm_execution_lease(
                r.task,
                &r.f.writer,
                ResumePmExecutionLease {
                    recovery: None,
                    expected_owner_version: r.review.reservation.owner_cas.version,
                    lease_id: r.lease.lease.lease_id,
                    expected_lease_version: r.lease.lease.version,
                    input: PmResumeInput::Answer {
                        input_id: answer.id,
                    },
                    checkpoint: PmResumeCheckpoint {
                        checkpoint_id: q.checkpoint_id.unwrap(),
                        checkpoint_revision: 1,
                        checkpoint_sha256: "c".repeat(64),
                        fence: r.lease.fence.clone(),
                        requirement_revision: Some(2),
                        content_hash: Some(r.review.context.requirements.content_hash.clone()),
                    },
                    previous_run_id: Uuid::new_v4(),
                    previous_run_terminal: ArtifactReceipt {
                        reference: "fixture://synthetic-second-old-pm-run".into(),
                        sha256: "d".repeat(64),
                    },
                    idempotency_key: "cycles:second-answer-resume".into(),
                },
            )
            .await
            .unwrap();
    assert!(!replayed);
    r.lease = lease;
    let tx = r.f.db.begin().await.unwrap();
    let state = r.f.repo.load(&tx, r.task, &r.f.owner).await.unwrap();
    let mut document = state.revisions.last().unwrap().document.clone();
    document.goal = "Second revised synthetic goal".into();
    tx.rollback().await.unwrap();
    let revision: RequirementsRevision = parse(
        command(
            r,
            &r.pm,
            SdlcCommand::PublishRevision(PublishRevision {
                fence: r.lease.fence.clone(),
                expected_requirement_revision: Some(2),
                document: document.clone(),
                idempotency_key: "cycles:second-revision".into(),
            }),
        )
        .await,
    );
    assert_eq!(revision.revision, 3);
    let verifier = actor(
        "verifier",
        false,
        &[
            "task-tracker:write",
            &format!(
                "task-tracker:sdlc:evidence:{}:{}",
                r.task, r.lease.fence.assignment_version
            ),
        ],
    );
    for check in document.checklist.iter().chain(&document.prerequisites) {
        command(
            r,
            &verifier,
            SdlcCommand::Evidence(EvidenceCommand {
                fence: r.lease.fence.clone(),
                requirement_revision: 3,
                content_hash: revision.content_hash.clone(),
                check_id: check.clone(),
                evidence_reference: "fixture://synthetic-second-readiness".into(),
                idempotency_key: format!("cycles:second-evidence:{check}"),
            }),
        )
        .await;
    }
    let consent: Confirmation = parse(
        command(
            r,
            &r.f.owner,
            SdlcCommand::Confirm {
                revision: 3,
                command: ConfirmCommand {
                    content_hash: revision.content_hash.clone(),
                    idempotency_key: "cycles:second-confirmation".into(),
                    expected_routing_policy_version: Some(1),
                },
            },
        )
        .await,
    );
    let (lease, replayed) =
        r.f.repo
            .resume_pm_execution_lease(
                r.task,
                &r.f.writer,
                ResumePmExecutionLease {
                    recovery: None,
                    expected_owner_version: r.review.reservation.owner_cas.version,
                    lease_id: r.lease.lease.lease_id,
                    expected_lease_version: r.lease.lease.version,
                    input: PmResumeInput::Confirmation {
                        input_id: consent.id,
                    },
                    checkpoint: PmResumeCheckpoint {
                        checkpoint_id: Uuid::new_v4(),
                        checkpoint_revision: 1,
                        checkpoint_sha256: "e".repeat(64),
                        fence: r.lease.fence.clone(),
                        requirement_revision: Some(3),
                        content_hash: Some(revision.content_hash.clone()),
                    },
                    previous_run_id: Uuid::new_v4(),
                    previous_run_terminal: ArtifactReceipt {
                        reference: "fixture://synthetic-second-confirmed-pm-run".into(),
                        sha256: "f".repeat(64),
                    },
                    idempotency_key: "cycles:second-confirmation-resume".into(),
                },
            )
            .await
            .unwrap();
    assert!(!replayed);
    r.lease = lease;
    let current =
        r.f.repo
            .lifecycle_read(r.task, &r.f.reader)
            .await
            .unwrap()
            .receipt;
    assert_eq!(current.task.stage, LifecycleStage::Draft);
    assert_eq!(
        current.task.waiting_reason.as_deref(),
        Some("critical_pm_awaiting_finalization")
    );
    assert_eq!(current.task.routing.requirement_revision, 2);
    let row=r.f.db.query_one(statement("SELECT NOT EXISTS(SELECT 1 FROM sdlc_analysis_intents WHERE task_id=$1 AND requirement_revision=3) AS no_intent,
        NOT EXISTS(SELECT 1 FROM sdlc_pm_completions WHERE execution_id=$2) AS no_completion",
        vec![r.task.into(),r.review.reservation.assignment.execution_id.into()])).await.unwrap().unwrap();
    assert!(row.try_get::<bool>("", "no_intent").unwrap());
    assert!(row.try_get::<bool>("", "no_completion").unwrap());
    (revision.content_hash, consent)
}

async fn final_history(r: &ReviewFixture, first: Uuid, second: Uuid) {
    let rows=r.f.db.query_all(statement("SELECT snapshot_id,task_id,project_id,generation,requirement_revision,payload FROM sdlc_routing_snapshot_inventory
        WHERE task_id=$1 ORDER BY generation",vec![r.task.into()])).await.unwrap();
    assert_eq!(rows.len(), 3);
    let mut ids = std::collections::HashSet::new();
    let policy = rows[0].try_get::<Json>("", "payload").unwrap()["policy"].clone();
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.try_get::<Uuid>("", "task_id").unwrap(), r.task);
        assert_eq!(row.try_get::<Uuid>("", "project_id").unwrap(), r.f.project);
        assert_eq!(
            row.try_get::<i64>("", "generation").unwrap(),
            index as i64 + 1
        );
        assert_eq!(
            row.try_get::<i64>("", "requirement_revision").unwrap(),
            index as i64 + 1
        );
        assert_eq!(
            row.try_get::<Json>("", "payload").unwrap()["policy"],
            policy
        );
        assert!(ids.insert(row.try_get::<Uuid>("", "snapshot_id").unwrap()));
    }
    let links=r.f.db.query_all(statement("SELECT previous_snapshot_id FROM sdlc_task_routing_generations WHERE task_id=$1 ORDER BY generation",
        vec![r.task.into()])).await.unwrap();
    assert_eq!(links.len(), 2);
    assert_eq!(
        links[0]
            .try_get::<Uuid>("", "previous_snapshot_id")
            .unwrap(),
        rows[0].try_get::<Uuid>("", "snapshot_id").unwrap()
    );
    assert_eq!(
        links[1]
            .try_get::<Uuid>("", "previous_snapshot_id")
            .unwrap(),
        rows[1].try_get::<Uuid>("", "snapshot_id").unwrap()
    );
    let row=r.f.db.query_one(statement("SELECT (SELECT count(*) FROM sdlc_critical_blockers WHERE task_id=$1) AS blockers,
        (SELECT count(*) FROM sdlc_critical_pm_reviews WHERE task_id=$1) AS reviews,
        (SELECT count(*) FROM sdlc_pm_completions WHERE task_id=$1) AS completions,
        (SELECT count(*) FROM sdlc_delivery_holds WHERE task_id=$1) AS holds,
        (SELECT count(*) FROM sdlc_delivery_projection_permits WHERE task_id=$1) AS delivery_permits,
        (SELECT count(*) FROM sdlc_pm_state_write_permits WHERE task_id=$1) AS pm_permits,
        (SELECT blocker_id FROM sdlc_critical_blocker_heads WHERE task_id=$1) AS head,
        sdlc_critical_pm_detail($2)->>'state' AS first,sdlc_critical_pm_detail($3)->>'state' AS second",
        vec![r.task.into(),first.into(),second.into()])).await.unwrap().unwrap();
    assert_eq!(row.try_get::<i64>("", "blockers").unwrap(), 2);
    assert_eq!(row.try_get::<i64>("", "reviews").unwrap(), 2);
    assert_eq!(row.try_get::<i64>("", "completions").unwrap(), 3);
    for column in ["holds", "delivery_permits", "pm_permits"] {
        assert_eq!(row.try_get::<i64>("", column).unwrap(), 0, "{column}");
    }
    assert_eq!(row.try_get::<Uuid>("", "head").unwrap(), second);
    assert_eq!(row.try_get::<String>("", "first").unwrap(), "resolved");
    assert_eq!(row.try_get::<String>("", "second").unwrap(), "resolved");
    let candidates =
        r.f.repo
            .delivery_candidates(&r.f.reader, r.f.query(None, 100))
            .await
            .unwrap();
    assert_eq!(
        candidates
            .items
            .iter()
            .filter(|item| item.task_id == r.task)
            .count(),
        1
    );
}

async fn current_source(
    r: &ReviewFixture,
    revision: i64,
    original: &domain::sdlc_source::TaskSourceBinding,
) {
    use domain::sdlc_source::TaskSourceReadback;
    let binding = match r.f.repo.task_source(r.task, &r.f.owner).await.unwrap() {
        TaskSourceReadback::Bound { binding, .. }
        | TaskSourceReadback::Blocked {
            binding: Some(binding),
            ..
        } => binding,
        _ => panic!("trusted PM completion must publish the exact confirmed source binding"),
    };
    assert_eq!(binding.requirement_revision, revision);
    let confirmation =
        r.f.repo
            .confirmation_read(r.task, &r.f.owner, revision)
            .await
            .unwrap();
    assert_eq!(binding.confirmation_id, confirmation.id);
    let snapshot = r.f.db.query_one(statement(
        "SELECT snapshot_id FROM sdlc_routing_snapshot_inventory WHERE task_id=$1 ORDER BY generation DESC LIMIT 1",
        vec![r.task.into()],
    )).await.unwrap().unwrap().try_get::<Uuid>("", "snapshot_id").unwrap();
    assert_eq!(binding.routing_snapshot_id, snapshot);
    assert_eq!(binding.source_revision, original.source_revision);
    assert_eq!(binding.policy_hash, original.policy_hash);
    assert_eq!(binding.namespace, original.namespace);
    assert_eq!(binding.forge_instance_id, original.forge_instance_id);
    assert_eq!(binding.repository_id, original.repository_id);
    assert_eq!(binding.base_ref, original.base_ref);
    assert_eq!(binding.base_commit, original.base_commit);
    assert_eq!(binding.candidate_ref, original.candidate_ref);
    let retained: domain::sdlc_source::TaskSourceBinding = parse(r.f.db.query_one(statement(
        "SELECT payload FROM sdlc_task_source_bindings WHERE task_id=$1 AND requirement_revision=1",
        vec![r.task.into()],
    )).await.unwrap().unwrap().try_get("", "payload").unwrap());
    assert_eq!(&retained, original);
}

#[tokio::test]
#[ignore = "fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test; main child registration/hooks/096 required; synthetic runtime only"]
async fn actual_two_critical_pm_cycles_and_atomic_sql_negatives() {
    let mut r = Box::pin(setup()).await;
    let task = r.task;
    let first = r.review.context.blocker_id;
    eprintln!("critical_cycles.phase.setup.done");
    let original_source: domain::sdlc_source::TaskSourceBinding = parse(r.f.db.query_one(statement(
        "SELECT payload FROM sdlc_task_source_bindings WHERE task_id=$1 AND requirement_revision=1",
        vec![r.task.into()],
    )).await.unwrap().unwrap().try_get("", "payload").unwrap());
    // A real second root supplies collision-only immutable source data; the two review cycles retain task above.
    let donor_task = Box::pin(r.f.queued_analysis("cycles-collision-donor", true)).await;
    let donor =
        r.f.db
            .query_one(statement(
                "SELECT snapshot_id FROM sdlc_task_routing_snapshots WHERE task_id=$1",
                vec![donor_task.into()],
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get::<Uuid>("", "snapshot_id")
            .unwrap();
    let (hash, consent) = Box::pin(questions_and_revision(&mut r)).await;
    let command = completion_command(&r, 2, hash);
    Box::pin(incomplete_packets(&r, &command)).await;
    Box::pin(generation_insert_negatives(&r, &command, donor)).await;
    eprintln!("critical_cycles.phase.partial_packets.denied");
    let first_completion = Box::pin(complete_once(&r, &command)).await;
    current_source(&r, 2, &original_source).await;
    assert_eq!(frozen_history(&r.f, r.task).await, r.original);
    assert_eq!(
        r.f.repo
            .confirmation_read(r.task, &r.f.owner, 2)
            .await
            .unwrap(),
        consent
    );
    Box::pin(committed_generation_negatives(&r, donor_task)).await;
    let archive = first_archive(&r, first).await;
    eprintln!("critical_cycles.phase.first.done");
    let old_policy =
        r.f.repo
            .source_policy(
                r.f.project,
                &r.f.owner,
                Some(original_source.source_revision),
            )
            .await
            .unwrap();
    let mut next_source = old_policy.source;
    next_source.base_commit = "b".repeat(40);
    let mut observation = old_policy.verification;
    observation.base_commit = next_source.base_commit.clone();
    observation.observed_at = chrono::Utc::now();
    r.f.repo
        .set_source_policy(
            r.f.project,
            &r.f.owner,
            SetSourcePolicy {
                expected_source_revision: Some(original_source.source_revision),
                source: next_source,
                idempotency_key: "cycles:new-source-policy".into(),
            },
            observation,
        )
        .await
        .unwrap();
    // Legitimately change current project policy; the existing task must KEEP its original frozen policy.
    let mut routes = r.f.roles.clone();
    routes.analyst.fleet_config_revision += 1;
    let (policy, replayed) =
        r.f.repo
            .set_routing_policy(
                r.f.project,
                &r.f.owner,
                domain::sdlc_routing::SetRoutingPolicy {
                    expected_version: Some(1),
                    routes,
                    idempotency_key: "cycles:new-project-policy".into(),
                },
            )
            .await
            .unwrap();
    assert!(!replayed);
    assert_eq!(policy.version, 2);
    Box::pin(second_review(&mut r)).await;
    assert_eq!(r.task, task);
    assert_eq!(first_archive(&r, first).await, archive);
    eprintln!("critical_cycles.phase.second.reserved");
    let second = r.review.context.blocker_id;
    let (hash, second_consent) = Box::pin(revise_second(&mut r)).await;
    let second_command = completion_command(&r, 3, hash);
    let second_completion = Box::pin(complete_once(&r, &second_command)).await;
    current_source(&r, 3, &original_source).await;
    assert_ne!(
        second_completion.fence.execution_id,
        first_completion.fence.execution_id
    );
    assert_eq!(
        r.f.repo
            .confirmation_read(r.task, &r.f.owner, 2)
            .await
            .unwrap(),
        consent
    );
    assert_eq!(
        r.f.repo
            .confirmation_read(r.task, &r.f.owner, 3)
            .await
            .unwrap(),
        second_consent
    );
    assert_eq!(
        r.f.repo
            .pm_draft_completion_operation(r.task, &r.f.reader, &command.idempotency_key)
            .await
            .unwrap()
            .result,
        first_completion
    );
    assert_eq!(r.task, task);
    assert_eq!(first_archive(&r, first).await, archive);
    Box::pin(final_history(&r, first, second)).await;
    eprintln!("critical_cycles.phase.second.done");
}
