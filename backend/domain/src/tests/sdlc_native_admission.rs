use super::*;
use crate::sdlc::PmAssignment;
use crate::sdlc_metadata::MetadataInputRef;
use crate::sdlc_pm_draft::*;

fn fixture() -> (PmDraftReservation, NativeAdmission) {
    let execution = Uuid::new_v4();
    let agent = Uuid::new_v4();
    let task = Uuid::new_v4();
    let assignment = Uuid::new_v4();
    let project = Uuid::new_v4();
    let reservation = PmDraftReservation {
        contract_version: 1,
        variant: PmDraftVariant::Reserved,
        binding: PmDraftBinding {
            tracker_instance_id: "tracker-owner".into(),
            project_id: project,
            task_id: task,
            root_task_id: task,
            owner_subject: Uuid::new_v4().to_string(),
        },
        owner_cas: OwnerCas {
            expected_version: 0,
            version: 1,
        },
        assignment: PmAssignment {
            assignment_id: assignment,
            execution_id: execution,
            agent_id: agent,
            version: 1,
            machine_subject: Uuid::new_v4().to_string(),
        },
        execution: PmDraftExecution {
            ordinal: "1".into(),
            key: "SDLC-1".into(),
        },
        input: MetadataInputRef {
            snapshot_ref: Uuid::new_v4(),
            sha256: "a".repeat(64),
        },
        assignment_operation_key: "owner-assignment".into(),
        admission_state: PmDraftAdmissionState::Reserved,
        dispatch_allowed: false,
    };
    let observed = NativeAdmission {
        contract_version: 1,
        observation_ref: Uuid::new_v4(),
        observed_at: Utc::now(),
        identity: Identity {
            task: "SDLC-1".into(),
            execution_ref: execution.to_string(),
            tracker_instance_ref: "tracker-owner".into(),
            tracker_project_ref: project.to_string(),
            task_ref: task.to_string(),
            root_ref: task.to_string(),
            agent_ref: agent.to_string(),
            assignment_operation_key: "owner-assignment".into(),
            assignment_ref: assignment.to_string(),
            assignment_revision: 1,
        },
        session_run_id: Uuid::new_v4(),
        native_run_ref: "run_native".into(),
        native_session_ref: "native-session".into(),
        binding_ref: "native-bind".into(),
        fence: 1,
        effective_config_revision: 1,
        configuration_sha256: "a".repeat(64),
        native_configuration: NativeConfiguration {
            model: "model".into(),
            provider: "custom".into(),
            api_mode: "chat_completions".into(),
            route_sha256: "a".repeat(64),
            tools: [
                "fleet_pm_checkpoint",
                "fleet_pm_context",
                "fleet_pm_question",
                "fleet_pm_requirements",
                "fleet_pm_skills",
                "fleet_pm_workflow",
            ]
            .map(str::to_owned)
            .to_vec(),
            output_limit: 512,
        },
    };
    (reservation, observed)
}

fn fleet_mcp_fixture() -> (PmDraftReservation, FleetMcpAdmission) {
    let (reservation, native) = fixture();
    let observed = FleetMcpAdmission {
        contract_version: 2,
        profile: "fleet_mcp_custody_v1".into(),
        observation_ref: Uuid::new_v4(),
        observed_at: Utc::now(),
        identity: native.identity,
        session_run_id: Uuid::new_v4(),
        hermes_run_ref: "hermes-run".into(),
        hermes_session_ref: "hermes-session".into(),
        binding_ref: "fleet-binding".into(),
        fence: 1,
        dispatch_operation_key: "fleet-dispatch".into(),
        effective_config_revision: 1,
        configuration_sha256: "b".repeat(64),
        dispatch_request_sha256: "c".repeat(64),
        profile_configuration: FleetMcpProfileConfiguration {
            evidence_basis: "fleet_effective_config_and_accepted_run".into(),
            runtime_source_revision: "bbaf7af5c83546d19f8060f4097d3bb25cd1a3c3".into(),
            mcp_server: "fleet_pm".into(),
            platform_toolsets: ["fleet_pm"].map(str::to_owned).to_vec(),
            advertised_server_tools: [
                "tracker_clarifications",
                "tracker_context",
                "tracker_publish_question",
                "tracker_publish_revision",
                "tracker_requirements",
                "workflow_step",
            ]
            .map(str::to_owned)
            .to_vec(),
            configured_wire_tools: [
                "mcp__fleet_pm__tracker_clarifications",
                "mcp__fleet_pm__tracker_context",
                "mcp__fleet_pm__tracker_publish_question",
                "mcp__fleet_pm__tracker_publish_revision",
                "mcp__fleet_pm__tracker_requirements",
                "mcp__fleet_pm__workflow_step",
            ]
            .map(str::to_owned)
            .to_vec(),
        },
    };
    (reservation, observed)
}

#[test]
fn native_owner_observation_is_exact_fresh_and_closed() {
    let (reservation, observed) = fixture();
    observed.validate(&reservation).unwrap();
    let mut variants = vec![];
    let mut wrong = observed.clone();
    wrong.identity.agent_ref = Uuid::new_v4().to_string();
    variants.push(wrong);
    let mut wrong = observed.clone();
    wrong.identity.assignment_revision += 1;
    variants.push(wrong);
    let mut wrong = observed.clone();
    wrong.observed_at -= chrono::Duration::seconds(6);
    variants.push(wrong);
    let mut wrong = observed.clone();
    wrong.native_configuration.tools.push("terminal".into());
    variants.push(wrong);
    let mut wrong = observed.clone();
    wrong.configuration_sha256 = "bad".into();
    variants.push(wrong);
    for wrong in variants {
        assert!(wrong.validate(&reservation).is_err());
    }
    let mut wire = serde_json::to_value(observed).unwrap();
    wire["allowed"] = serde_json::json!(true);
    assert!(serde_json::from_value::<NativeAdmission>(wire).is_err());
}

#[test]
fn admission_observation_dispatches_by_closed_version_and_profile() {
    let (_, native) = fixture();
    assert!(matches!(
        serde_json::from_value::<AdmissionObservation>(serde_json::to_value(native).unwrap())
            .unwrap(),
        AdmissionObservation::NativeV1(_)
    ));

    let (_, fleet_mcp) = fleet_mcp_fixture();
    assert!(matches!(
        serde_json::from_value::<AdmissionObservation>(
            serde_json::to_value(fleet_mcp.clone()).unwrap()
        )
        .unwrap(),
        AdmissionObservation::FleetMcpCustodyV1(_)
    ));

    let mut without_profile = serde_json::to_value(&fleet_mcp).unwrap();
    without_profile.as_object_mut().unwrap().remove("profile");
    assert!(serde_json::from_value::<AdmissionObservation>(without_profile).is_err());

    let mut unknown_profile = serde_json::to_value(&fleet_mcp).unwrap();
    unknown_profile["profile"] = serde_json::json!("native_tool_custody_v2");
    assert!(serde_json::from_value::<AdmissionObservation>(unknown_profile).is_err());

    let mut unexpected_v1_profile = serde_json::to_value(fixture().1).unwrap();
    unexpected_v1_profile["profile"] = serde_json::json!("fleet_mcp_custody_v1");
    assert!(serde_json::from_value::<AdmissionObservation>(unexpected_v1_profile).is_err());
}

#[test]
fn fleet_mcp_profile_is_exact_fresh_and_closed() {
    let (reservation, observed) = fleet_mcp_fixture();
    observed.validate(&reservation).unwrap();

    let mut variants = vec![];
    let mut wrong = observed.clone();
    wrong.identity.assignment_ref = Uuid::new_v4().to_string();
    variants.push(wrong);
    let mut wrong = observed.clone();
    wrong.observed_at -= chrono::Duration::seconds(6);
    variants.push(wrong);
    let mut wrong = observed.clone();
    wrong
        .profile_configuration
        .configured_wire_tools
        .push("mcp__fleet_pm__terminal".into());
    variants.push(wrong);
    let mut wrong = observed.clone();
    wrong.profile_configuration.runtime_source_revision = "a".repeat(40);
    variants.push(wrong);
    let mut wrong = observed.clone();
    wrong.dispatch_request_sha256 = "bad".into();
    variants.push(wrong);
    let mut wrong = observed.clone();
    wrong.hermes_run_ref = "bad\nrun".into();
    variants.push(wrong);

    for wrong in variants {
        assert!(wrong.validate(&reservation).is_err());
    }

    let mut wire = serde_json::to_value(observed).unwrap();
    wire["profile_configuration"]["live_provider_inventory"] = serde_json::json!(true);
    assert!(serde_json::from_value::<FleetMcpAdmission>(wire).is_err());
}

#[test]
fn immutable_history_comparison_is_profile_specific() {
    let (_, native) = fixture();
    let mut same_native = native.clone();
    same_native.observation_ref = Uuid::new_v4();
    same_native.observed_at = Utc::now();
    assert!(
        AdmissionObservation::NativeV1(same_native)
            .same_immutable_run(&AdmissionObservation::NativeV1(native.clone()))
    );

    let mut changed_native = native.clone();
    changed_native.native_run_ref = "other".into();
    assert!(
        !AdmissionObservation::NativeV1(changed_native)
            .same_immutable_run(&AdmissionObservation::NativeV1(native))
    );

    let (_, fleet_mcp) = fleet_mcp_fixture();
    let mut same_fleet_mcp = fleet_mcp.clone();
    same_fleet_mcp.observation_ref = Uuid::new_v4();
    same_fleet_mcp.observed_at = Utc::now();
    assert!(
        AdmissionObservation::FleetMcpCustodyV1(same_fleet_mcp)
            .same_immutable_run(&AdmissionObservation::FleetMcpCustodyV1(fleet_mcp.clone()))
    );

    let mut changed_fleet_mcp = fleet_mcp.clone();
    changed_fleet_mcp.dispatch_request_sha256 = "d".repeat(64);
    assert!(
        !AdmissionObservation::FleetMcpCustodyV1(changed_fleet_mcp)
            .same_immutable_run(&AdmissionObservation::FleetMcpCustodyV1(fleet_mcp))
    );

    assert!(
        !AdmissionObservation::FleetMcpCustodyV1(fleet_mcp_fixture().1)
            .same_profile(&AdmissionObservation::NativeV1(fixture().1))
    );
}
