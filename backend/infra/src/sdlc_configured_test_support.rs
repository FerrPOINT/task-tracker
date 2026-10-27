//! Synthetic Workflow receipt for downstream ledger tests, never live acceptance.
use super::*;
use domain::sdlc_configured_cli::*;
use domain::sdlc_discussion::{DiscussionRepository, PublishStageComment};
use domain::sdlc_pm_draft::ReservePmDraft;

pub(super) async fn service(repo: &PostgresSdlcRepository, project: Uuid) -> Principal {
    let subject = Uuid::new_v4().to_string();
    let user = Uuid::new_v4();
    exec(
        repo.db.raw(),
        "INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)
        VALUES($1,$2,$3,$3,'!',$3,true)",
        vec![
            user.into(),
            format!("{user}@example.test").into(),
            subject.clone().into(),
        ],
    )
    .await
    .unwrap();
    exec(
        repo.db.raw(),
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'developer')",
        vec![project.into(), user.into()],
    )
    .await
    .unwrap();
    Principal {
        subject: subject.clone(),
        human_session: false,
        trusted_human: false,
        scopes: ["task-tracker:read".into(), "task-tracker:write".into()].into(),
    }
}

pub(super) async fn assigned_pm(
    repo: &PostgresSdlcRepository,
    task: Uuid,
    owner: &Principal,
    agent: Uuid,
) -> (PmDraftReservation, Principal) {
    let state = repo.read(task, owner).await.unwrap();
    let pm = service(repo, state.project_id).await;
    let subject = pm.subject.clone();
    let (reservation, replay) = repo
        .reserve_pm_draft(
            task,
            owner,
            ReservePmDraft {
                interaction_protocol: Some(InteractionProtocol::ConfiguredCliV1),
                machine_subject: Some(subject),
                requested_agent_id: agent,
                expected_owner_version: 0,
                expected_assignment_version: None,
                idempotency_key: "fixture:pm-reserve".into(),
            },
        )
        .await
        .unwrap();
    assert!(!replay);
    (reservation, pm)
}

pub(super) async fn confirmed_pm(
    repo: &PostgresSdlcRepository,
    task: Uuid,
    owner: &Principal,
    agent: Uuid,
    document: RequirementsDocument,
    complete: bool,
) -> (PmDraftReservation, Option<ConfiguredPmCompletionReceipt>) {
    let (reservation, pm) = assigned_pm(repo, task, owner, agent).await;
    let fence = MachineFence {
        assignment_id: reservation.assignment.assignment_id,
        execution_id: reservation.assignment.execution_id,
        agent_id: agent,
        assignment_version: reservation.assignment.version,
    };
    let hash = app::sdlc::canonical_hash(&document).unwrap();
    repo.execute(
        task,
        &pm,
        SdlcCommand::PublishRevision(PublishRevision {
            fence: fence.clone(),
            expected_requirement_revision: None,
            document,
            idempotency_key: "fixture:requirements".into(),
        }),
    )
    .await
    .unwrap();
    let consent = SdlcCommand::Confirm {
        revision: 1,
        command: ConfirmCommand {
            content_hash: hash.clone(),
            idempotency_key: "fixture:consent".into(),
            expected_routing_policy_version: None,
        },
    };
    let confirmed = repo.execute(task, owner, consent.clone()).await.unwrap();
    assert_eq!(confirmed["stage"], "Draft");
    assert_eq!(repo.execute(task, owner, consent).await.unwrap(), confirmed);
    if !complete {
        return (reservation, None);
    }
    let saved = complete_pm(repo, task, owner, &pm, &reservation).await;
    (reservation, Some(saved))
}

pub(super) async fn complete_pm(
    repo: &PostgresSdlcRepository,
    task: Uuid,
    owner: &Principal,
    pm: &Principal,
    reservation: &PmDraftReservation,
) -> ConfiguredPmCompletionReceipt {
    let state = repo.read(task, owner).await.unwrap();
    let assignment = &reservation.assignment;
    let fence = MachineFence {
        assignment_id: assignment.assignment_id,
        execution_id: assignment.execution_id,
        agent_id: assignment.agent_id,
        assignment_version: assignment.version,
    };
    let revision = state.revisions.last().unwrap();
    let hash = revision.content_hash.clone();
    let command = CompleteConfiguredPm {
        interaction_protocol: InteractionProtocol::ConfiguredCliV1,
        fence: fence.clone(),
        requirement_revision: revision.revision,
        content_hash: hash.clone(),
        namespace_id: 1,
        workflow_execution_ref: Uuid::new_v4(),
        workflow_operation_key: "fixture:workflow-final".into(),
        workflow_result_version: 4,
        idempotency_key: format!("fixture:complete:{}", assignment.execution_id),
    };
    let workflow: ConfiguredWorkflowReceipt = serde_json::from_value(json!({
        "interaction_protocol":"configured-cli-v1","execution_ref":command.workflow_execution_ref,
        "task":reservation.execution.key,"tracker_task_id":task,"namespace_id":1,
        "concrete_agent_ref":assignment.agent_id,"assignment_ref":fence.assignment_id,
        "tracker_execution_id":fence.execution_id,"assignment_version":fence.assignment_version,
        "machine_subject":pm.subject,"operation_key":command.workflow_operation_key,
        "request_sha256":"a".repeat(64),"result_version":4,"requirements_revision":revision.revision,
        "requirements_sha256":hash,"owner_subject":owner.subject,
        "confirmation_id":state.current_confirmation().unwrap().id,"history_id":17,"status":"done"
    }))
    .unwrap();
    repo.publish_stage_comment(
        task,
        &pm,
        PublishStageComment {
            fence,
            body: "Synthetic final fixture report; exact owner-approved requirements checked"
                .into(),
            idempotency_key: format!("fixture:final-comment:{}", assignment.execution_id),
        },
    )
    .await
    .unwrap();
    let (saved, replay) = repo
        .complete_configured_pm(task, &pm, command.clone(), workflow.clone())
        .await
        .unwrap();
    assert!(!replay);
    let (same, replay) = repo
        .complete_configured_pm(task, &pm, command, workflow)
        .await
        .unwrap();
    assert!(replay);
    assert_eq!(same, saved);
    saved
}

pub(super) async fn delivery_comment(
    repo: &PostgresSdlcRepository,
    task: Uuid,
    actor: &Principal,
    assignment: &domain::sdlc_lifecycle::DeliveryAssignment,
    key: &str,
) {
    repo.publish_stage_comment(
        task,
        actor,
        PublishStageComment {
            fence: MachineFence {
                assignment_id: assignment.assignment_id,
                execution_id: assignment.execution_id,
                agent_id: assignment.agent_id,
                assignment_version: assignment.fencing_token,
            },
            body:
                "Synthetic final role report for current assignment; work and checks are recorded"
                    .into(),
            idempotency_key: key.into(),
        },
    )
    .await
    .unwrap();
}
