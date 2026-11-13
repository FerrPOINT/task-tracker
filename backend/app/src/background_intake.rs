//! Human finding intake uses the ordinary project and issue creation boundary.
use domain::{background_intake::*, sdlc::*};
use shared::AppError;
use uuid::Uuid;

pub fn task_completed(
    state: Option<&TaskState>,
    delivery: Option<&domain::sdlc_lifecycle::LifecycleReceipt>,
    closed: bool,
) -> bool {
    use domain::sdlc_lifecycle::{DeliveryResult, LifecycleStage, LifecycleStatus, RuntimeState};
    let Some(state) = state else {
        return closed;
    };
    state.stage == Stage::Deployment
        && state.status == Some(LifecycleStatus::Completed)
        && delivery.is_some_and(|d| {
            d.task.task_id == state.task_id
                && d.task.stage == LifecycleStage::Deployment
                && d.task.status == LifecycleStatus::Completed
                && d.task.runtime == RuntimeState::Terminal
                && d.assignment.is_none()
                && d.lease.is_none()
                && d.binding.is_none()
                && d.result.as_ref().is_some_and(|r| {
                    r.task_id == state.task_id
                        && r.previous_stage == LifecycleStage::Deployment
                        && r.next_stage == LifecycleStage::Deployment
                        && r.next_status == LifecycleStatus::Completed
                        && matches!(r.output, DeliveryResult::DevOps { .. })
                })
        })
}

pub fn requirements(finding: &FindingIntake) -> Result<RequirementsDocument, AppError> {
    // These are newly recorded check references to the reviewed finding fields,
    // not claims that the checks or product tests have already passed.
    let reference = format!(
        "finding:{}:{}",
        finding.source.finding_id, finding.source.finding_revision
    );
    let document = RequirementsDocument {
        goal: finding.title.clone(),
        scope: vec![finding.description.clone()],
        scenarios: vec![finding.expected.clone()],
        acceptance_criteria: vec![finding.criteria.clone()],
        checklist: vec![format!("{reference}:acceptance")],
        prerequisites: vec![format!("{reference}:evidence")],
        dependencies: vec![finding.evidence.clone()],
        exclusions: vec![],
        constraints: vec![],
        assumptions: vec![],
    };
    crate::sdlc::validate_document(&document)?;
    Ok(document)
}

pub fn analysis_intent(
    state: &TaskState,
    confirmation: &Confirmation,
    intent: Uuid,
) -> Result<AnalysisIntent, AppError> {
    let origin = state
        .retained_fields
        .get("finding_intake_operation")
        .and_then(serde_json::Value::as_str)
        .and_then(|v| v.parse::<Uuid>().ok())
        .filter(|v| !v.is_nil());
    if origin.is_none()
        || !state.configured_cli()
        || !matches!(state.stage, Stage::Draft | Stage::Analysis)
        || state.revisions.len() != 1
        || state.confirmations.len() != 1
        || confirmation.revision != 1
        || state.confirmation_revision != Some(1)
        || state.assignment.is_some()
    {
        return Err(AppError::conflict(
            "exact initial human finding approval required",
        ));
    }
    crate::sdlc::analysis_intent_payload(state, confirmation, intent)
}

pub fn draft_command(
    actor: &Principal,
    operation: Uuid,
    command: &FindingIntake,
) -> Result<CreateDraftCommand, AppError> {
    if !actor.human_session || actor.subject.is_empty() {
        return Err(AppError::Forbidden);
    }
    if operation.is_nil()
        || command.source.finding_id.is_nil()
        || !(1..=9_007_199_254_740_991).contains(&command.source.finding_revision)
    {
        return Err(AppError::validation(
            "invalid finding operation or revision",
        ));
    }
    for value in [
        &command.description,
        &command.expected,
        &command.criteria,
        &command.evidence,
    ] {
        if value.trim().is_empty() || value.contains('\0') {
            return Err(AppError::validation(
                "finding material must be nonempty and contain no NUL",
            ));
        }
    }
    let draft = CreateDraftCommand {
        title: command.title.clone(),
        description: format!(
            "{}\n\nExpected result\n{}\n\nAcceptance criteria\n{}\n\nEvidence\n{}",
            command.description, command.expected, command.criteria, command.evidence
        ),
        idempotency_key: format!("fleet-background:{operation}"),
        flow: Some(DraftFlow::SingleTaskV1),
        interaction_protocol: Some(InteractionProtocol::ConfiguredCliV1),
    };
    crate::sdlc::validate_draft(actor, &draft)?;
    Ok(draft)
}

impl crate::sdlc::SdlcService {
    pub async fn intake_finding(
        &self,
        project: Uuid,
        actor: &Principal,
        operation: Uuid,
        command: FindingIntake,
    ) -> Result<(FindingIntakeReceipt, bool), AppError> {
        draft_command(actor, operation, &command)?;
        self.repository
            .intake_finding(project, actor, operation, command)
            .await
    }
    pub async fn finding_intake_operation(
        &self,
        project: Uuid,
        actor: &Principal,
        operation: Uuid,
    ) -> Result<FindingIntakeReceipt, AppError> {
        if !actor.human_session || actor.subject.is_empty() || operation.is_nil() {
            return Err(AppError::Forbidden);
        }
        self.repository
            .finding_intake_operation(project, actor, operation)
            .await
    }
}
