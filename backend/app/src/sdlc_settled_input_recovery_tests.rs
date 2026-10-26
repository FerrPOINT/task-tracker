use super::*;
use serde_json::{Value, json};

fn command() -> (PmDraftReservation, ResumePmExecutionLease) {
    let (_, reservation, mut command) = super::tests::fixture();
    let hash = request_hash(&command).unwrap();
    command.recovery = Some(PmSettledInputRecovery {
        kind: PmSettledInputRecoveryKind::SettledInputExpiry,
        original_key: command.idempotency_key.clone(),
        original_request_sha256: hash.clone(),
        predecessor_key: command.idempotency_key.clone(),
        predecessor_request_sha256: hash,
        barrier_id: Uuid::from_u128(91),
    });
    command.idempotency_key = "settled-recovery-1".into();
    command.expected_lease_version = 103;
    (reservation, command)
}

fn budget_witness() -> (
    PmDraftReservation,
    ResumePmExecutionLease,
    PmSettledInputRecoveryWitness,
) {
    let (reservation, command) = command();
    let witness = PmSettledInputRecoveryWitness {
        contract_version: 1,
        session_run_id: command.previous_run_id,
        barrier_id: command.recovery.as_ref().unwrap().barrier_id,
        identity: crate::sdlc_pm_bootstrap_retry::identity(&reservation),
        request_sha256: request_hash(&command).unwrap(),
        command: command.clone(),
        manual_budget: Some(PmSettledInputManualBudget {
            contract_version: 1,
            root_operation_id: Uuid::from_u128(100),
            control_operation_id: Uuid::from_u128(101),
            retry_operation_key: format!("fleet-pm-resume-retry:{}", "a".repeat(64)),
            retry_request_sha256: "b".repeat(64),
            original_resume_request_sha256: command
                .recovery
                .as_ref()
                .unwrap()
                .original_request_sha256
                .clone(),
            prefix_count: 3,
            prefix_sha256: "c".repeat(64),
            allowance: 3,
        }),
    };
    (reservation, command, witness)
}

#[test]
fn settled_manual_budget_preserves_default_cap_and_bounds_the_extra_positions() {
    let (reservation, command, mut witness) = budget_witness();
    let command_hash = request_hash(&command).unwrap();
    validate_settled_witness(&witness, &reservation, &command).unwrap();
    for position in 0..8 {
        assert_eq!(
            validate_settled_budget_position(&witness, position).is_ok(),
            (3..6).contains(&position)
        );
    }
    // Root/control may coincide, but neither is inferred from Tracker execution/run IDs.
    let budget = witness.manual_budget.as_mut().unwrap();
    budget.control_operation_id = budget.root_operation_id;
    validate_settled_witness(&witness, &reservation, &command).unwrap();
    assert_eq!(request_hash(&command).unwrap(), command_hash);
    witness.manual_budget = None;
    for position in 0..8 {
        assert_eq!(
            validate_settled_budget_position(&witness, position).is_ok(),
            position < 3
        );
    }
    assert!(
        serde_json::to_value(witness)
            .unwrap()
            .get("manual_budget")
            .is_none()
    );
}

#[test]
fn settled_manual_budget_rejects_malformed_foreign_and_overflowing_permission() {
    let (reservation, command, witness) = budget_witness();
    let raw = serde_json::to_value(witness).unwrap();
    for (field, value) in [
        ("contract_version", json!(2)),
        ("root_operation_id", json!(Uuid::nil())),
        ("control_operation_id", json!(Uuid::nil())),
        ("retry_operation_key", json!("another-retry")),
        ("retry_request_sha256", json!("B".repeat(64))),
        ("original_resume_request_sha256", json!("f".repeat(64))),
        ("prefix_sha256", json!("not-a-hash")),
        ("prefix_count", json!(2)),
        ("prefix_count", json!(9_007_199_254_740_990u64)),
        ("allowance", json!(4)),
    ] {
        let mut changed = raw.clone();
        changed["manual_budget"][field] = value;
        let Ok(changed) = serde_json::from_value::<PmSettledInputRecoveryWitness>(changed) else {
            // Canonical DTO identity checks may refuse before semantic validation.
            continue;
        };
        assert!(
            validate_settled_witness(&changed, &reservation, &command).is_err(),
            "{field}"
        );
        assert!(
            validate_settled_budget_position(&changed, 3).is_err(),
            "{field}"
        );
    }
    for (field, value) in [
        ("unknown", json!(true)),
        ("prefix_count", json!(3.5)),
        (
            "root_operation_id",
            json!("00000000-0000-0000-0000-00000000000A"),
        ),
    ] {
        let mut changed = raw.clone();
        changed["manual_budget"][field] = value;
        assert!(serde_json::from_value::<PmSettledInputRecoveryWitness>(changed).is_err());
    }
    // Clients cannot supply the callback permission through the existing POST DTO.
    let mut supplied = serde_json::to_value(&command).unwrap();
    supplied["manual_budget"] = raw["manual_budget"].clone();
    assert!(serde_json::from_value::<ResumePmExecutionLease>(supplied).is_err());
}

#[test]
fn settled_manual_budget_does_not_substitute_for_exact_callback_custody() {
    let (reservation, command, witness) = budget_witness();
    let raw = serde_json::to_value(&witness).unwrap();
    for (pointer, value) in [
        ("/request_sha256", json!("e".repeat(64))),
        ("/barrier_id", json!(Uuid::new_v4())),
        ("/identity/execution_ref", json!(Uuid::new_v4().to_string())),
        ("/command/expected_lease_version", json!(104)),
    ] {
        let mut changed = raw.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(
            validate_settled_witness(
                &serde_json::from_value(changed).unwrap(),
                &reservation,
                &command
            )
            .is_err()
        );
    }
    // Historical proof validation needs no fresh callback or mutable Fleet parent state.
    let retained: PmSettledInputRecoveryWitness = serde_json::from_value(raw.clone()).unwrap();
    validate_settled_witness(&retained, &reservation, &command).unwrap();
    validate_settled_budget_position(&retained, 3).unwrap();
    assert_eq!(serde_json::to_value(retained).unwrap(), raw);
}

#[test]
fn settled_input_absent_extension_preserves_original_wire_and_hash() {
    let (_, _, command) = super::tests::fixture();
    let value = serde_json::to_value(&command).unwrap();
    assert!(value.get("recovery").is_none());
    let mut keys = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "checkpoint",
            "expected_lease_version",
            "expected_owner_version",
            "idempotency_key",
            "input",
            "lease_id",
            "previous_run_id",
            "previous_run_terminal"
        ]
    );
    assert_eq!(
        request_hash(&command).unwrap(),
        crate::sdlc::canonical_hash(
            &json!({"operation":"resume_pm_execution_lease","payload":value})
        )
        .unwrap()
    );
    assert_eq!(
        serde_json::to_value(
            serde_json::from_value::<ResumePmExecutionLease>(value.clone()).unwrap()
        )
        .unwrap(),
        value
    );
}

#[test]
fn settled_input_witness_is_closed_and_exactly_bound_to_request_and_assignment() {
    let (reservation, command) = command();
    validate_command(&command).unwrap();
    let witness = PmSettledInputRecoveryWitness {
        contract_version: 1,
        session_run_id: command.previous_run_id,
        barrier_id: command.recovery.as_ref().unwrap().barrier_id,
        identity: crate::sdlc_pm_bootstrap_retry::identity(&reservation),
        request_sha256: request_hash(&command).unwrap(),
        command: command.clone(),
        manual_budget: None,
    };
    validate_settled_witness(&witness, &reservation, &command).unwrap();
    let raw = serde_json::to_value(witness).unwrap();
    assert!(raw.get("manual_budget").is_none());
    for (pointer, bad) in [
        ("/contract_version", json!(2)),
        ("/session_run_id", json!(Uuid::new_v4())),
        ("/barrier_id", json!(Uuid::new_v4())),
        ("/request_sha256", json!("f".repeat(64))),
        ("/identity/agent_ref", json!(Uuid::new_v4().to_string())),
        ("/command/expected_lease_version", json!(102)),
        ("/command/input/input_id", json!(Uuid::new_v4())),
    ] {
        let mut changed = raw.clone();
        *changed.pointer_mut(pointer).unwrap() = bad;
        assert!(
            validate_settled_witness(
                &serde_json::from_value(changed).unwrap(),
                &reservation,
                &command
            )
            .is_err(),
            "{pointer}"
        );
    }
    let mut extra = raw;
    extra["no_successor"] = json!(true);
    assert!(serde_json::from_value::<PmSettledInputRecoveryWitness>(extra).is_err());
}

#[test]
fn settled_input_rejects_unknown_kind_fields_and_invalid_or_reused_anchor_keys() {
    let (_, command) = command();
    let raw = serde_json::to_value(&command).unwrap();
    for (pointer, bad) in [
        ("/recovery/kind", json!("initial_unsubmitted")),
        ("/recovery/foreign", json!(true)),
        ("/recovery/barrier_id", json!(Uuid::nil())),
    ] {
        let mut changed = raw.clone();
        if let Some(target) = changed.pointer_mut(pointer) {
            *target = bad;
        } else {
            changed["recovery"]["foreign"] = bad;
        }
        assert!(
            serde_json::from_value::<ResumePmExecutionLease>(changed).is_err(),
            "{pointer}"
        );
    }
    for (field, bad) in [
        ("original_request_sha256", json!("A".repeat(64))),
        ("predecessor_request_sha256", json!("b".repeat(64))),
        ("original_key", json!(command.idempotency_key)),
        ("predecessor_key", json!(command.idempotency_key)),
    ] {
        let mut changed: Value = raw.clone();
        changed["recovery"][field] = bad;
        assert!(
            validate_command(&serde_json::from_value(changed).unwrap()).is_err(),
            "{field}"
        );
    }
}
