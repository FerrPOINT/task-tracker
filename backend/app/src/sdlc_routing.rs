use domain::sdlc::MAX_SAFE_VERSION;
use domain::sdlc_routing::{PACKAGE_COMMIT, RoleRoutes};
use shared::AppError;
use std::collections::HashSet;

pub fn validate_project_pm_default(
    policy: &domain::sdlc_routing::RoutingPolicy,
    project: uuid::Uuid,
    instance: &str,
    requested_agent: uuid::Uuid,
) -> Result<(), AppError> {
    if policy.project_id != project
        || policy.contract_version != policy.routes.contract_version()
        || policy.tracker_instance_id != instance
        || policy.routes.project_manager.agent_id.is_nil()
        || policy.routing_hash != crate::sdlc::canonical_hash(&policy.routes)?
    {
        return Err(AppError::conflict("project PM default is inconsistent"));
    }
    if requested_agent != policy.routes.project_manager.agent_id {
        return Err(AppError::conflict(
            "requested agent differs from authoritative project PM default",
        ));
    }
    Ok(())
}

pub fn validate_routes(routes: &RoleRoutes) -> Result<(), AppError> {
    fn hash(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
    fn id(value: &str) -> bool {
        value
            .parse::<i64>()
            .is_ok_and(|id| id > 0 && id.to_string() == value)
    }
    let mut agents = HashSet::new();
    let mut namespaces = HashSet::new();
    let mut workflows = HashSet::new();
    let single_task = routes.contract_version() == 2;
    for (role, route) in routes.entries() {
        let suffix = role.replace('_', "-");
        let (namespace, workflow, profile, catalog_version, source_matches) = if single_task {
            (
                format!("single-task-{suffix}"),
                format!("single-task-v1:{role}"),
                format!("single-task-v1-{suffix}"),
                1,
                route.package_commit.is_none()
                    && route.package_manifest_sha256.is_none()
                    && route.instruction_profile.as_deref() == Some("single-task-v1")
                    && route
                        .instruction_manifest_sha256
                        .as_deref()
                        .is_some_and(hash)
                    && route.instruction_manifest_sha256
                        == routes.project_manager.instruction_manifest_sha256,
            )
        } else {
            let profile = match role {
                "tester" => "hermes-sdlc-quality".into(),
                "devops" => "hermes-sdlc-operations".into(),
                _ => format!("hermes-sdlc-{suffix}"),
            };
            (
                format!("hermes-{suffix}"),
                format!("hermes-sdlc:{role}"),
                profile,
                3,
                route.instruction_profile.is_none()
                    && route.instruction_manifest_sha256.is_none()
                    && route.package_commit.as_deref() == Some(PACKAGE_COMMIT)
                    && route.package_manifest_sha256.as_deref().is_some_and(hash)
                    && route.package_manifest_sha256
                        == routes.project_manager.package_manifest_sha256,
            )
        };
        if route.agent_id.is_nil()
            || !agents.insert(route.agent_id)
            || !namespaces.insert(&route.namespace_id)
            || !workflows.insert(&route.workflow_id)
            || !(1..=MAX_SAFE_VERSION).contains(&route.fleet_config_revision)
            || !source_matches
            || !id(&route.namespace_id)
            || !id(&route.workflow_id)
            || route.namespace_name != namespace
            || route.workflow_key != workflow
            || route.profile != profile
            || route.workflow_catalog_version != catalog_version
            || !hash(&route.workflow_catalog_sha256)
            || route.workflow_catalog_sha256 != routes.project_manager.workflow_catalog_sha256
        {
            return Err(AppError::validation(
                "invalid seven-role SDLC routing references",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    fn routes() -> RoleRoutes {
        let mut value = json!({});
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
            value[*role] = json!({"agent_id":Uuid::from_u128(i as u128+1), "fleet_config_revision":1,
                "package_commit":PACKAGE_COMMIT, "package_manifest_sha256":"a".repeat(64),
                "namespace_id":(i+1).to_string(), "namespace_name":format!("hermes-{suffix}"),
                "workflow_id":(i+11).to_string(), "workflow_key":format!("hermes-sdlc:{role}"),
                "profile":profile, "workflow_catalog_version":3, "workflow_catalog_sha256":"b".repeat(64)});
        }
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn routing_references_require_seven_distinct_agents_and_exact_public_pins() {
        let valid = routes();
        validate_routes(&valid).unwrap();
        for (field, bad) in [
            ("agent_id", json!(Uuid::nil())),
            ("agent_id", json!(valid.project_manager.agent_id)),
            ("fleet_config_revision", json!(0)),
            ("fleet_config_revision", json!(MAX_SAFE_VERSION + 1)),
            ("package_commit", json!("HEAD")),
            ("package_manifest_sha256", json!("f".repeat(64))),
            ("namespace_id", json!("hermes-analyst")),
            ("namespace_id", json!("02")),
            ("namespace_id", json!("9223372036854775808")),
            ("namespace_id", json!(valid.project_manager.namespace_id)),
            ("namespace_name", json!("hermes-developer")),
            ("workflow_id", json!(valid.project_manager.workflow_id)),
            ("workflow_key", json!("hermes-sdlc:developer")),
            ("profile", json!("foreign")),
            ("workflow_catalog_version", json!(2)),
            ("workflow_catalog_sha256", json!("f".repeat(64))),
        ] {
            let mut value = serde_json::to_value(&valid).unwrap();
            value["analyst"][field] = bad;
            if let Ok(routes) = serde_json::from_value(value) {
                assert!(validate_routes(&routes).is_err(), "{field}");
            }
        }
        for field in ["analyst", "unregistered_role"] {
            let mut value = serde_json::to_value(&valid).unwrap();
            if field == "analyst" {
                value.as_object_mut().unwrap().remove(field);
            } else {
                value[field] = value["analyst"].clone();
            }
            assert!(serde_json::from_value::<RoleRoutes>(value).is_err());
        }
        let mut value = serde_json::to_value(valid).unwrap();
        value["analyst"]["runtime_ready"] = json!(true);
        assert!(serde_json::from_value::<RoleRoutes>(value).is_err());
    }

    #[test]
    fn routing_opt_in_preserves_legacy_confirmation_command_hash() {
        let legacy = json!({"operation":"confirm","payload":{"revision":2,"command":{"content_hash":"a".repeat(64),"idempotency_key":"original"}}});
        let command: domain::sdlc::SdlcCommand = serde_json::from_value(legacy.clone()).unwrap();
        assert_eq!(
            crate::sdlc::command_hash(&command).unwrap(),
            crate::sdlc::canonical_hash(&legacy).unwrap()
        );
        let mut opted = legacy;
        opted["payload"]["command"]["expected_routing_policy_version"] = json!(1);
        let opted: domain::sdlc::SdlcCommand = serde_json::from_value(opted).unwrap();
        assert_ne!(
            crate::sdlc::command_hash(&command).unwrap(),
            crate::sdlc::command_hash(&opted).unwrap()
        );
    }

    #[test]
    fn profile_routing_is_explicit_and_cannot_mix_or_impersonate_base_packages() {
        let legacy = routes();
        let raw = serde_json::to_value(&legacy).unwrap();
        assert!(raw["analyst"].get("instruction_profile").is_none());
        assert_eq!(legacy.contract_version(), 1);
        assert_eq!(
            serde_json::to_value(serde_json::from_value::<RoleRoutes>(raw.clone()).unwrap())
                .unwrap(),
            raw
        );
        let mut value = raw;
        for (role, _) in legacy.entries() {
            let suffix = role.replace('_', "-");
            let route = value[role].as_object_mut().unwrap();
            route.remove("package_commit");
            route.remove("package_manifest_sha256");
            route.insert("instruction_profile".into(), json!("single-task-v1"));
            route.insert("instruction_manifest_sha256".into(), json!("c".repeat(64)));
            route.insert(
                "namespace_name".into(),
                json!(format!("single-task-{suffix}")),
            );
            route.insert(
                "workflow_key".into(),
                json!(format!("single-task-v1:{role}")),
            );
            route.insert("profile".into(), json!(format!("single-task-v1-{suffix}")));
            route.insert("workflow_catalog_version".into(), json!(1));
        }
        let single: RoleRoutes = serde_json::from_value(value.clone()).unwrap();
        validate_routes(&single).unwrap();
        assert_eq!(single.contract_version(), 2);
        assert!(
            serde_json::to_value(&single).unwrap()["analyst"]
                .get("package_commit")
                .is_none()
        );
        for (field, bad) in [
            ("package_commit", json!(PACKAGE_COMMIT)),
            ("package_manifest_sha256", json!("a".repeat(64))),
            ("instruction_profile", json!("single-task-v2")),
            ("instruction_manifest_sha256", json!("A".repeat(64))),
            ("instruction_manifest_sha256", json!("d".repeat(64))),
            ("workflow_catalog_version", json!(3)),
            ("workflow_key", json!("hermes-sdlc:analyst")),
        ] {
            let mut changed = value.clone();
            changed["analyst"][field] = bad;
            let changed = serde_json::from_value(changed).unwrap();
            assert!(validate_routes(&changed).is_err(), "{field}");
        }
        let mut mixed = value;
        mixed["developer"] = serde_json::to_value(&legacy.developer).unwrap();
        assert!(validate_routes(&serde_json::from_value(mixed).unwrap()).is_err());
    }
}
