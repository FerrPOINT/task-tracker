use domain::sdlc_routing::RoutingPolicy;
use shared::AppError;
use uuid::Uuid;

/// Declared routing prerequisites only; actual PM runtime readiness stays in Fleet.
pub fn validate_policy(
    policy: &RoutingPolicy,
    project: Uuid,
    instance: &str,
) -> Result<(), AppError> {
    crate::sdlc_routing::validate_routes(&policy.routes)?;
    crate::sdlc_routing::validate_project_pm_default(
        policy,
        project,
        instance,
        policy.routes.project_manager.agent_id,
    )?;
    if policy.contract_version != 2 || policy.native_ready || policy.dispatch_allowed {
        return Err(AppError::conflict("single_task_v2_routing_policy_required"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn policy() -> RoutingPolicy {
        let mut routes = json!({});
        for (n, role) in [
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
            routes[*role] = json!({"agent_id":Uuid::new_v4(),"fleet_config_revision":1,
                "instruction_profile":"single-task-v1","instruction_manifest_sha256":"a".repeat(64),
                "namespace_id":(n+1).to_string(),"namespace_name":format!("single-task-{suffix}"),
                "workflow_id":(n+11).to_string(),"workflow_key":format!("single-task-v1:{role}"),
                "profile":format!("single-task-v1-{suffix}"),"workflow_catalog_version":1,"workflow_catalog_sha256":"b".repeat(64)});
        }
        let routes = serde_json::from_value(routes).unwrap();
        RoutingPolicy {
            contract_version: 2,
            tracker_instance_id: "test".into(),
            project_id: Uuid::new_v4(),
            version: 1,
            routing_hash: crate::sdlc::canonical_hash(&routes).unwrap(),
            routes,
            verification: domain::sdlc_routing::RoutingVerification::Declared,
            native_ready: false,
            dispatch_allowed: false,
            author_subject: "owner".into(),
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn marked_draft_requires_bound_seven_role_v2_policy_not_runtime_claims() {
        let valid = policy();
        validate_policy(&valid, valid.project_id, "test").unwrap();
        assert!(validate_policy(&valid, Uuid::new_v4(), "test").is_err());
        assert!(validate_policy(&valid, valid.project_id, "foreign").is_err());
        let mut wrong = valid.clone();
        wrong.contract_version = 1;
        assert!(validate_policy(&wrong, valid.project_id, "test").is_err());
        let mut wrong = valid.clone();
        wrong.routing_hash = "f".repeat(64);
        assert!(validate_policy(&wrong, valid.project_id, "test").is_err());
        let mut wrong = valid.clone();
        wrong.routes.developer.instruction_profile = None;
        assert!(validate_policy(&wrong, valid.project_id, "test").is_err());
        let mut wrong = valid.clone();
        wrong.native_ready = true;
        assert!(validate_policy(&wrong, valid.project_id, "test").is_err());
        let mut wrong = valid.clone();
        wrong.dispatch_allowed = true;
        assert!(validate_policy(&wrong, valid.project_id, "test").is_err());
    }
}
