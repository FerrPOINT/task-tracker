use super::*;
use serde_json::json;

fn fixture() -> (
    TaskState,
    PmDraftReservation,
    RecoverUnsubmittedPmExecutionLease,
    UnsubmittedPmWitness,
) {
    let task = Uuid::new_v4();
    let assignment = PmAssignment {
        assignment_id: Uuid::new_v4(),
        execution_id: Uuid::new_v4(),
        agent_id: Uuid::new_v4(),
        version: 1,
        machine_subject: "pm".into(),
    };
    let state: TaskState = serde_json::from_value(json!({
        "tracker_instance_id":"tracker", "project_id":Uuid::new_v4(),"task_id":task,"root_task_id":task,
        "owner_subject":"owner","stage":"Draft","flow":"single-task-v1","confirmation_revision":null,
        "assignment":assignment,"questions":[],"revisions":[],"evidence":[],"confirmations":[]
    })).unwrap();
    let reservation: PmDraftReservation = serde_json::from_value(json!({
        "contract_version":1,"variant":"pm_draft_reserved","binding":{"tracker_instance_id":state.tracker_instance_id,
        "project_id":state.project_id,"task_id":task,"root_task_id":task,"owner_subject":"owner"},
        "owner_cas":{"expected_version":0,"version":1},"assignment":assignment,
        "execution":{"ordinal":"1","key":"SDLC-1"},"input":{"snapshot_ref":Uuid::new_v4(),"sha256":"a".repeat(64)},
        "assignment_operation_key":"original-assignment","admission_state":"reserved","dispatch_allowed":false
    })).unwrap();
    let command = RecoverUnsubmittedPmExecutionLease {
        expected_owner_version: 1,
        fence: MachineFence {
            assignment_id: assignment.assignment_id,
            execution_id: assignment.execution_id,
            agent_id: assignment.agent_id,
            assignment_version: 1,
        },
        lease_id: Uuid::new_v4(),
        expected_lease_version: 1,
        session_run_id: Uuid::new_v4(),
        barrier_id: Uuid::new_v4(),
        idempotency_key: "recover-original".into(),
    };
    let witness = UnsubmittedPmWitness {
        contract_version: 1,
        session_run_id: command.session_run_id,
        barrier_id: command.barrier_id,
        lease_id: command.lease_id,
        lease_version: 1,
        owner_version: 1,
        identity: PmExecutionIdentity {
            task: "SDLC-1".into(),
            execution_ref: assignment.execution_id.to_string(),
            tracker_instance_ref: "tracker".into(),
            tracker_project_ref: state.project_id.to_string(),
            task_ref: task.to_string(),
            root_ref: task.to_string(),
            agent_ref: assignment.agent_id.to_string(),
            assignment_operation_key: reservation.assignment_operation_key.clone(),
            assignment_ref: assignment.assignment_id.to_string(),
            assignment_revision: 1,
        },
    };
    (state, reservation, command, witness)
}

#[test]
fn unsubmitted_recovery_rejects_every_foreign_witness_coordinate() {
    let (_, r, c, w) = fixture();
    validate_witness(&w, &r, &c).unwrap();
    let original = serde_json::to_value(w).unwrap();
    for field in [
        "contract_version",
        "session_run_id",
        "barrier_id",
        "lease_id",
        "lease_version",
        "owner_version",
    ] {
        let mut bad = original.clone();
        bad[field] = if field.ends_with("id") {
            json!(Uuid::new_v4())
        } else {
            json!(2)
        };
        assert!(
            validate_witness(&serde_json::from_value(bad).unwrap(), &r, &c).is_err(),
            "{field}"
        );
    }
    for field in original["identity"].as_object().unwrap().keys() {
        let mut bad = original.clone();
        bad["identity"][field] = if field == "assignment_revision" {
            json!(2)
        } else {
            json!("foreign")
        };
        assert!(
            validate_witness(&serde_json::from_value(bad).unwrap(), &r, &c).is_err(),
            "{field}"
        );
    }
}

#[test]
fn unsubmitted_request_and_witness_are_closed_and_reject_fake_runtime_proof() {
    let (_, _, c, w) = fixture();
    let body = serde_json::to_value(&c).unwrap();
    for field in ["never_submitted", "witness", "terminal", "checkpoint"] {
        let mut bad = body.clone();
        bad[field] = json!(true);
        assert!(serde_json::from_value::<RecoverUnsubmittedPmExecutionLease>(bad).is_err());
        let mut bad = serde_json::to_value(&w).unwrap();
        bad[field] = json!(true);
        assert!(serde_json::from_value::<UnsubmittedPmWitness>(bad).is_err());
    }
    for field in ["lease_id", "barrier_id", "session_run_id"] {
        for invalid in [
            Uuid::nil().to_string(),
            "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA".into(),
        ] {
            let mut bad = body.clone();
            bad[field] = json!(invalid);
            assert!(serde_json::from_value::<RecoverUnsubmittedPmExecutionLease>(bad).is_err());
        }
    }
    let mut changed = c.clone();
    changed.barrier_id = Uuid::new_v4();
    assert_ne!(request_hash(&c).unwrap(), request_hash(&changed).unwrap());
    assert_eq!(
        request_hash(&c).unwrap(),
        crate::sdlc::canonical_hash(
            &json!({"operation":"recover_unsubmitted_pm_execution_lease","payload":c})
        )
        .unwrap()
    );
}

#[test]
fn unsubmitted_recovery_requires_initial_current_owner_fence_and_draft() {
    let (state, r, c, _) = fixture();
    validate_context(&state, &r, &c).unwrap();
    for field in [
        "owner_subject",
        "project_id",
        "task_id",
        "root_task_id",
        "tracker_instance_id",
        "stage",
        "flow",
    ] {
        let mut bad = serde_json::to_value(&state).unwrap();
        bad[field] = match field {
            "stage" => json!("Backlog"),
            "flow" => json!(null),
            v if v.ends_with("id") => json!(Uuid::new_v4()),
            _ => json!("foreign"),
        };
        assert!(
            validate_context(&serde_json::from_value(bad).unwrap(), &r, &c).is_err(),
            "{field}"
        );
    }
    let mut successor = r.clone();
    successor.owner_cas.expected_version = 1;
    assert!(validate_context(&state, &successor, &c).is_err());
    let mut stale = c.clone();
    stale.expected_owner_version += 1;
    assert!(validate_context(&state, &r, &stale).is_err());
    let mut stale = c.clone();
    stale.fence.assignment_version += 1;
    assert!(validate_context(&state, &r, &stale).is_err());
    let mut completed = state.clone();
    completed.status = Some(domain::sdlc_lifecycle::LifecycleStatus::Completed);
    assert!(validate_context(&completed, &r, &c).is_err());
}
