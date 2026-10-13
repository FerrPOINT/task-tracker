//! Synthetic wire fixtures from real producer DTOs; not execution/readiness evidence.
use chrono::{Duration, TimeZone, Utc};
use domain::{sdlc_execution_lease::ExecutionLease, sdlc_lifecycle::*, sdlc_routing::*};
use serde_json::json;
use uuid::Uuid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let now = Utc.with_ymd_and_hms(2026, 10, 13, 10, 0, 0).unwrap();
    let mut route_json = json!({});
    for (index, role) in [
        "project_manager",
        "analyst",
        "architect",
        "developer",
        "reviewer",
        "tester",
        "devops",
    ]
    .into_iter()
    .enumerate()
    {
        let suffix = role.replace('_', "-");
        let profile = match role {
            "tester" => "quality",
            "devops" => "operations",
            _ => &suffix,
        };
        route_json[role] = json!({"agent_id":Uuid::from_u128(index as u128 + 1),"fleet_config_revision":1,
            "package_commit":PACKAGE_COMMIT,"package_manifest_sha256":"a".repeat(64),
            "namespace_id":(index+1).to_string(),"namespace_name":format!("hermes-{suffix}"),
            "workflow_id":(index+1).to_string(),"workflow_key":format!("hermes-sdlc:{role}"),
            "profile":format!("hermes-sdlc-{profile}"),"workflow_catalog_version":3,"workflow_catalog_sha256":"b".repeat(64)});
    }
    let routes: RoleRoutes = serde_json::from_value(route_json)?;
    app::sdlc_routing::validate_routes(&routes)?;
    let scheduler_subject = Uuid::from_u128(100).to_string();
    let owner_subject = Uuid::from_u128(10).to_string();
    let task = Uuid::from_u128(20);
    let project = Uuid::from_u128(30);
    let routing = TaskRoutingSnapshot {
        contract_version: 1,
        snapshot_id: Uuid::from_u128(40),
        tracker_instance_id: "tracker".into(),
        project_id: project,
        task_id: task,
        root_task_id: task,
        confirmation_id: Uuid::from_u128(41),
        requirement_revision: 1,
        content_hash: "c".repeat(64),
        policy: RoutingPolicy {
            contract_version: 1,
            tracker_instance_id: "tracker".into(),
            project_id: project,
            version: 1,
            routing_hash: app::sdlc::canonical_hash(&routes)?,
            routes,
            verification: RoutingVerification::Declared,
            native_ready: false,
            dispatch_allowed: false,
            author_subject: owner_subject.clone(),
            created_at: now,
        },
        created_at: now,
    };
    let mut readbacks = Vec::new();
    for (index, stage) in [
        LifecycleStage::Analysis,
        LifecycleStage::Architecture,
        LifecycleStage::Development,
        LifecycleStage::Review,
        LifecycleStage::Testing,
        LifecycleStage::Rework,
        LifecycleStage::Deployment,
    ]
    .into_iter()
    .enumerate()
    {
        let role = stage.role().unwrap();
        let cycle = u8::from(stage == LifecycleStage::Rework);
        let route = routing
            .policy
            .routes
            .entries()
            .into_iter()
            .find(|(key, _)| *key == role.key())
            .unwrap()
            .1;
        let previous_result_id = if stage == LifecycleStage::Analysis {
            None
        } else {
            Some(Uuid::from_u128(77))
        };
        let mut assignment = DeliveryAssignment {
            assignment_id: Uuid::from_u128(200 + index as u128),
            execution_id: Uuid::from_u128(300 + index as u128),
            task_id: task,
            owner_subject: owner_subject.clone(),
            role,
            stage,
            cycle,
            attempt: 1,
            agent_id: route.agent_id,
            routing: routing.clone(),
            previous_result_id,
            fencing_token: 1,
            lease_id: Uuid::from_u128(400 + index as u128),
            assignment_hash: String::new(),
        };
        assignment.assignment_hash = app::sdlc_lifecycle::assignment_hash(&assignment)?;
        let lease = ExecutionLease {
            lease_id: assignment.lease_id,
            version: 1,
            holder_subject: scheduler_subject.clone(),
            claimed_at: now,
            heartbeat_at: now,
            expires_at: now + Duration::seconds(30),
        };
        readbacks.push(LifecycleReadback {
            observed_at: now + Duration::seconds(1),
            runtime_state: RuntimeState::Reserved,
            receipt: LifecycleReceipt {
                task: LifecycleTask {
                    task_id: task,
                    owner_subject: owner_subject.clone(),
                    version: 2,
                    stage,
                    status: LifecycleStatus::Ready,
                    cycle,
                    transient_retries: 0,
                    waiting_reason: Some("awaiting_run_binding".into()),
                    runtime: RuntimeState::Reserved,
                    routing: routing.clone(),
                    previous_result_id,
                    resume_chat_id: None,
                },
                assignment: Some(assignment),
                lease: Some(lease),
                binding: None,
                result: None,
            },
        });
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"tracker_instance_id":"tracker","scheduler_subject":scheduler_subject,"readbacks":readbacks})
        )?
    );
    Ok(())
}
