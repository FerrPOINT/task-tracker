use domain::sdlc::MAX_SAFE_VERSION;
use domain::sdlc_routing::{PACKAGE_COMMIT, RoleRoutes};
use shared::AppError;
use std::collections::HashSet;

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
    for (role, route) in routes.entries() {
        let suffix = role.replace('_', "-");
        let profile = match role {
            "tester" => "hermes-sdlc-quality".into(),
            "devops" => "hermes-sdlc-operations".into(),
            _ => format!("hermes-sdlc-{suffix}"),
        };
        if route.agent_id.is_nil()
            || !agents.insert(route.agent_id)
            || !namespaces.insert(&route.namespace_id)
            || !workflows.insert(&route.workflow_id)
            || !(1..=MAX_SAFE_VERSION).contains(&route.fleet_config_revision)
            || route.package_commit != PACKAGE_COMMIT
            || !hash(&route.package_manifest_sha256)
            || route.package_manifest_sha256 != routes.project_manager.package_manifest_sha256
            || !id(&route.namespace_id)
            || !id(&route.workflow_id)
            || route.namespace_name != format!("hermes-{suffix}")
            || route.workflow_key != format!("hermes-sdlc:{role}")
            || route.profile != profile
            || route.workflow_catalog_version != 3
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
}
