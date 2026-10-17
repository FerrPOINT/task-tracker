//! Child of the existing critical candidates fixture. Synthetic custody, not live runtime evidence.
use super::*;
use domain::sdlc_critical_pm_reservation::ReserveCriticalPmReview;
use domain::sdlc_execution_lease::ClaimExecutionLease;

#[path = "attachment_pm_review_pg_tests.rs"]
mod sealed_evidence_tests;

async fn read_snapshot(f: &Fixture, task: Uuid) -> Json {
    f.db.query_one(statement("SELECT jsonb_build_object(
        'state',(SELECT state FROM sdlc_tasks WHERE task_id=$1),
        'reviews',(SELECT count(*) FROM sdlc_critical_pm_reviews WHERE task_id=$1),
        'leases',(SELECT count(*) FROM sdlc_pm_execution_leases l JOIN sdlc_pm_executions e USING(execution_id) WHERE e.task_id=$1),
        'outbox',(SELECT count(*) FROM sdlc_outbox WHERE task_id=$1)) AS value",
        vec![task.into()])).await.unwrap().unwrap().try_get("", "value").unwrap()
}

#[tokio::test]
#[ignore = "fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test; synthetic runtime only"]
async fn actual_current_pm_blocker_read_requires_saved_review_fresh_acl_and_current_lease() {
    let f = Box::pin(Fixture::new()).await;
    exec(&f.db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)
        VALUES($1,'critical-agent@example.test','critical-agent','critical-agent','!','critical-agent',true)",
        vec![Uuid::new_v4().into()]).await.unwrap();
    exec(
        &f.db,
        "INSERT INTO project_members(project_id,user_id,role)
        SELECT $1,id,'developer' FROM users WHERE central_sub='critical-agent'",
        vec![f.project.into()],
    )
    .await
    .unwrap();
    let storage = domain::InMemoryStorage::default();
    let r = Box::pin(start(&f, "pm-blocker-reader", false, &storage)).await;
    let unlisted = Box::pin(sealed_evidence_tests::unlisted(&f, &r, &storage)).await;
    let (held, _) = Box::pin(f.repo.submit_critical_blocker_with_storage(
        r.task,
        &r.producer,
        blocker(&r),
        &storage,
    ))
    .await
    .unwrap();
    let previous = f
        .repo
        .pm_draft_assignment(r.task, &f.owner, None)
        .await
        .unwrap()
        .current
        .unwrap();
    let mut old_pm = actor("pm", false, &["task-tracker:read", "task-tracker:write"]);
    old_pm.scopes.insert(previous.assignment.scope(r.task));
    assert!(
        f.repo
            .critical_blocker(r.task, &old_pm, held.blocker_id)
            .await
            .is_err()
    );
    let current = f
        .repo
        .lifecycle_read(r.task, &f.reader)
        .await
        .unwrap()
        .receipt;
    let (returned, _) = Box::pin(f.repo.return_critical_to_pm_with_terminal(
        r.task,
        &f.writer,
        ReturnCriticalToPm {
            review: CriticalPmReviewGuard {
                blocker_id: held.blocker_id,
                blocker_sha256: held.blocker_sha256.clone(),
                expected_blocker_version: held.version,
                expected_task_version: current.task.version,
                fence: fence(&current),
                idempotency_key: "reader-return-original".into(),
            },
            terminal: RuntimeTerminalObservation {
                assignment_hash: held.assignment_hash.clone(),
                chat_id: held.chat_id,
                run_id: held.run_id,
                state: RuntimeTerminalState::Cancelled,
                receipt: ArtifactReceipt {
                    reference: "fixture://synthetic-terminal-not-live-proof".into(),
                    sha256: "d".repeat(64),
                },
            },
        },
    ))
    .await
    .unwrap();
    assert!(
        f.repo
            .critical_blocker(r.task, &old_pm, held.blocker_id)
            .await
            .is_err()
    );
    let (review, _) = Box::pin(f.repo.reserve_critical_pm_review(
        r.task,
        &f.writer,
        ReserveCriticalPmReview {
            blocker_id: held.blocker_id,
            blocker_sha256: held.blocker_sha256.clone(),
            expected_blocker_version: returned.blocker.version,
            expected_task_version: returned.lifecycle.task.version,
            expected_pm_owner_version: previous.owner_cas.version,
            expected_pm_assignment_version: previous.assignment.version,
            idempotency_key: "reader-review-original".into(),
        },
    ))
    .await
    .unwrap();
    let assignment = &review.reservation.assignment;
    let mut pm = actor(
        &assignment.machine_subject,
        false,
        &["task-tracker:read", "task-tracker:write"],
    );
    pm.scopes.insert(assignment.scope(r.task));
    // A saved review or a syntactically exact grant alone is not an active read capability.
    assert!(
        f.repo
            .critical_blocker(r.task, &pm, held.blocker_id)
            .await
            .is_err()
    );
    let (_, replay) = f
        .repo
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
                idempotency_key: "reader-lease-original".into(),
            },
        )
        .await
        .unwrap();
    assert!(!replay);
    let before = read_snapshot(&f, r.task).await;
    let detail = f
        .repo
        .critical_blocker(r.task, &pm, held.blocker_id)
        .await
        .unwrap();
    assert_eq!(detail.state, CriticalBlockerState::InPmReview);
    assert_eq!(detail.version, review.context.blocker_version);
    assert_eq!(
        detail.pm_review.as_ref().unwrap().execution_id,
        assignment.execution_id
    );
    assert_eq!(detail.proposal, held.proposal);
    // Still reached only AFTER the actual producer claim succeeds; never inject/skip its lease.
    Box::pin(sealed_evidence_tests::check(
        &f, &r, &pm, &old_pm, unlisted, &storage,
    ))
    .await;
    assert_eq!(read_snapshot(&f, r.task).await, before);
    assert!(
        f.repo
            .critical_blocker(r.task, &old_pm, held.blocker_id)
            .await
            .is_err()
    );
    assert!(
        f.repo
            .critical_blocker(r.task, &pm, Uuid::new_v4())
            .await
            .is_err()
    );
    let other = f.draft("foreign-pm-blocker-read", None).await;
    assert!(
        f.repo
            .critical_blocker(other, &pm, held.blocker_id)
            .await
            .is_err()
    );
    // No PM access to original producer or scheduler operation history is introduced.
    assert!(
        f.repo
            .critical_blocker_operation(r.task, &pm, &held.proposal.idempotency_key)
            .await
            .is_err()
    );
    assert!(
        f.repo
            .critical_pm_review_operation(r.task, &pm, &review.reservation.assignment_operation_key)
            .await
            .is_err()
    );

    exec(
        &f.db,
        "UPDATE users SET is_active=false WHERE central_sub='owner'",
        vec![],
    )
    .await
    .unwrap();
    assert!(
        f.repo
            .critical_blocker(r.task, &pm, held.blocker_id)
            .await
            .is_err()
    );
    exec(
        &f.db,
        "UPDATE users SET is_active=true WHERE central_sub='owner'",
        vec![],
    )
    .await
    .unwrap();
    exec(&f.db,"DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub='pm')",
        vec![f.project.into()]).await.unwrap();
    assert!(
        f.repo
            .critical_blocker(r.task, &pm, held.blocker_id)
            .await
            .is_err()
    );
    exec(&f.db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub='pm'",
        vec![f.project.into()]).await.unwrap();
    exec(&f.db,"UPDATE projects SET owner_id=(SELECT id FROM users WHERE central_sub='foreign') WHERE id=$1",
        vec![f.project.into()]).await.unwrap();
    assert!(
        f.repo
            .critical_blocker(r.task, &pm, held.blocker_id)
            .await
            .is_err()
    );
    exec(
        &f.db,
        "UPDATE projects SET owner_id=(SELECT id FROM users WHERE central_sub='owner') WHERE id=$1",
        vec![f.project.into()],
    )
    .await
    .unwrap();
    f.repo
        .critical_blocker(r.task, &pm, held.blocker_id)
        .await
        .unwrap();
    assert_eq!(read_snapshot(&f, r.task).await, before);
}
