use super::*;
use domain::sdlc::{PmAssignment, Principal};
use uuid::Uuid;

#[test]
fn current_pm_grant_route_policy_allows_only_exact_task_blocker_detail_get() {
    let task = Uuid::new_v4();
    let assignment = PmAssignment {
        assignment_id: Uuid::new_v4(),
        execution_id: Uuid::new_v4(),
        agent_id: Uuid::new_v4(),
        version: 18,
        machine_subject: "synthetic-pm".into(),
    };
    let actor = Principal {
        subject: assignment.machine_subject.clone(),
        human_session: false,
        trusted_human: false,
        scopes: ["task-tracker:read".into(), assignment.scope(task)].into(),
    };
    let prefix = format!("/api/v1/issues/{task}/sdlc/");
    let id = Uuid::from_u128(0xabcdef);
    pm_request_policy(&actor, "GET", &format!("{prefix}critical-blockers/{id}")).unwrap();
    for method in ["POST", "PATCH", "DELETE"] {
        assert!(
            pm_request_policy(&actor, method, &format!("{prefix}critical-blockers/{id}")).is_err()
        );
    }
    for suffix in [
        "critical-blockers".into(),
        "critical-blockers/returns".into(),
        "critical-blockers/operations/original".into(),
        "critical-blockers/pm-reviews".into(),
        "critical-blockers/pm-reviews/operations/original".into(),
        format!("critical-blockers/{id}/"),
        format!("critical-blockers/{}", id.to_string().to_uppercase()),
        "critical-blockers/00000000-0000-0000-0000-000000000000".into(),
    ] {
        assert!(pm_request_policy(&actor, "GET", &format!("{prefix}{suffix}")).is_err());
    }
    assert!(
        pm_request_policy(
            &actor,
            "GET",
            &format!(
                "/api/v1/issues/{}/sdlc/critical-blockers/{id}",
                Uuid::new_v4()
            )
        )
        .is_err()
    );
    for path in [
        format!("/api/v1/attachments/{id}/download"),
        format!("/api/v1/issues/{task}/attachments"),
    ] {
        assert!(pm_request_policy(&actor, "GET", &path).is_err());
    }
    let mut ambiguous = actor.clone();
    ambiguous.scopes.insert(assignment.scope(Uuid::new_v4()));
    assert!(
        pm_request_policy(
            &ambiguous,
            "GET",
            &format!("{prefix}critical-blockers/{id}")
        )
        .is_err()
    );
    let mut human = actor;
    human.human_session = true;
    assert!(pm_request_policy(&human, "GET", &format!("{prefix}critical-blockers/{id}")).is_err());
}
