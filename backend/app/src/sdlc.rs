use domain::sdlc::*;
use domain::sdlc_pm_draft::{PmDraftReadback, PmDraftReservation, ReservePmDraft};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use shared::AppError;
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

#[cfg(test)]
#[path = "sdlc_tests.rs"]
mod tests;

#[derive(Clone)]
pub struct SdlcService {
    pub repository: Arc<dyn SdlcRepository>,
}

impl SdlcService {
    pub async fn claim_execution_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        command: domain::sdlc_execution_lease::ClaimExecutionLease,
    ) -> Result<(domain::sdlc_execution_lease::ExecutionLeaseReceipt, bool), AppError> {
        self.repository
            .claim_execution_lease(task, actor, command)
            .await
    }
    pub async fn heartbeat_execution_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        command: domain::sdlc_execution_lease::HeartbeatExecutionLease,
    ) -> Result<(domain::sdlc_execution_lease::ExecutionLeaseReceipt, bool), AppError> {
        self.repository
            .heartbeat_execution_lease(task, actor, command)
            .await
    }
    pub async fn execution_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        key: Option<&str>,
    ) -> Result<domain::sdlc_execution_lease::ExecutionLeaseReadback, AppError> {
        self.repository.execution_lease(task, actor, key).await
    }
    pub async fn project_access(&self, actor: &Principal) -> Result<ProjectAccess, AppError> {
        self.repository.project_access_scope(actor).await
    }
    pub async fn project_directory(
        &self,
        actor: &Principal,
        after: Option<Uuid>,
        limit: u16,
    ) -> Result<ProjectDirectory, AppError> {
        if !(1..=100).contains(&limit) || after.is_some_and(|id| id.is_nil()) {
            return Err(AppError::validation("invalid project directory query"));
        }
        self.repository.project_directory(actor, after, limit).await
    }
    pub async fn create_draft(
        &self,
        project: Uuid,
        actor: &Principal,
        command: CreateDraftCommand,
    ) -> Result<(CreatedDraft, bool), AppError> {
        self.repository.create_draft(project, actor, command).await
    }
    pub async fn context(&self, task: Uuid, actor: &Principal) -> Result<SdlcContext, AppError> {
        Ok(self.repository.read(task, actor).await?.context(actor))
    }
    pub async fn draft_creation_operation(
        &self,
        project: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<CreatedDraft, AppError> {
        self.repository
            .draft_creation_operation(project, actor, key)
            .await
    }
    pub async fn reserve_pm_draft(
        &self,
        task: Uuid,
        actor: &Principal,
        command: ReservePmDraft,
    ) -> Result<(PmDraftReservation, bool), AppError> {
        self.repository.reserve_pm_draft(task, actor, command).await
    }
    pub async fn pm_draft_assignment(
        &self,
        task: Uuid,
        actor: &Principal,
        key: Option<&str>,
    ) -> Result<PmDraftReadback, AppError> {
        self.repository.pm_draft_assignment(task, actor, key).await
    }
    pub async fn pm_draft_input(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<PmDraftInputResponse, AppError> {
        self.repository.pm_draft_input(task, actor).await
    }
    pub async fn execute<T: serde::de::DeserializeOwned>(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SdlcCommand,
    ) -> Result<T, AppError> {
        serde_json::from_value(self.repository.execute(task, actor, command).await?)
            .map_err(AppError::internal)
    }
}

pub const PM_GRANT_PREFIX: &str = "task-tracker:sdlc:pm:";

pub fn has_pm_grant(scopes: &HashSet<String>) -> bool {
    scopes
        .iter()
        .any(|scope| scope.starts_with(PM_GRANT_PREFIX))
}

pub fn canonical_pm_uuid(value: &str) -> Option<Uuid> {
    let id = Uuid::parse_str(value).ok()?;
    (!id.is_nil() && id.to_string() == value).then_some(id)
}

pub fn canonical_pm_version(value: &str) -> bool {
    value.parse::<i64>().is_ok_and(|version| {
        (1..=MAX_SAFE_VERSION).contains(&version) && version.to_string() == value
    })
}

/// A PM credential carries one exact capability, never a union of task grants.
pub fn pm_grant_task(actor: &Principal) -> Result<Option<Uuid>, AppError> {
    let mut grants = actor
        .scopes
        .iter()
        .filter_map(|scope| scope.strip_prefix(PM_GRANT_PREFIX));
    let Some(grant) = grants.next() else {
        return Ok(None);
    };
    if actor.human_session || grants.next().is_some() {
        return Err(AppError::Forbidden);
    }
    let parts: Vec<_> = grant.split(':').collect();
    if parts.len() != 5
        || parts[..4]
            .iter()
            .any(|part| canonical_pm_uuid(part).is_none())
        || !canonical_pm_version(parts[4])
    {
        return Err(AppError::Forbidden);
    }
    Ok(canonical_pm_uuid(parts[0]))
}

pub fn authorize_pm_read(state: &TaskState, actor: &Principal) -> Result<(), AppError> {
    let Some(task) = pm_grant_task(actor)? else {
        return Ok(());
    };
    let assignment = state.assignment.as_ref().ok_or(AppError::Forbidden)?;
    if task != state.task_id
        || actor.subject != assignment.machine_subject
        || !actor.scopes.contains(&assignment.scope(task))
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub fn canonical_hash<T: Serialize>(value: &T) -> Result<String, AppError> {
    fn sort(value: Value) -> Value {
        match value {
            Value::Object(object) => {
                let sorted: BTreeMap<_, _> = object
                    .into_iter()
                    .map(|(key, value)| (key, sort(value)))
                    .collect();
                Value::Object(sorted.into_iter().collect())
            }
            Value::Array(array) => Value::Array(array.into_iter().map(sort).collect()),
            other => other,
        }
    }
    let bytes = serde_json::to_vec(&sort(
        serde_json::to_value(value).map_err(AppError::internal)?,
    ))
    .map_err(AppError::internal)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

pub fn command_hash(command: &SdlcCommand) -> Result<String, AppError> {
    let mut canonical = command.clone();
    // Multiple-choice selection is a set; display order is not part of the command.
    if let SdlcCommand::Answer { command, .. } = &mut canonical {
        command.selected_option_ids.sort();
    }
    canonical_hash(&canonical)
}

pub fn pm_draft_input_hash(title: &str, description: &str) -> Result<String, AppError> {
    canonical_hash(&serde_json::json!({"description": description, "title": title}))
}

pub fn validate_key(key: &str) -> Result<(), AppError> {
    if key.is_empty() || key.len() > 128 || key.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(AppError::validation(
            "idempotency_key must be 1..128 non-whitespace characters",
        ));
    }
    Ok(())
}

pub fn validate_draft(actor: &Principal, command: &CreateDraftCommand) -> Result<(), AppError> {
    if !actor.human_session || actor.subject.is_empty() {
        return Err(AppError::Forbidden);
    }
    validate_key(&command.idempotency_key)?;
    if command.title.trim().is_empty()
        || command.title.chars().count() > 500
        || command.title.chars().any(char::is_control)
        || command.description.chars().count() > 100_000
        || command.description.contains('\0')
    {
        return Err(AppError::validation("invalid draft title or description"));
    }
    Ok(())
}

fn nonblank(value: &str, field: &str) -> Result<(), AppError> {
    if value.trim().is_empty() || value.len() > 100_000 {
        return Err(AppError::validation(format!("invalid {field}")));
    }
    Ok(())
}

fn next_version(current: i64) -> Result<i64, AppError> {
    if !(0..MAX_SAFE_VERSION).contains(&current) {
        return Err(AppError::conflict(
            "version/revision exceeds Number.MAX_SAFE_INTEGER",
        ));
    }
    Ok(current + 1)
}

fn require_owner(state: &TaskState, actor: &Principal) -> Result<(), AppError> {
    if state.owner(actor) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

fn fence_matches(assignment: &PmAssignment, fence: &MachineFence) -> bool {
    assignment.assignment_id == fence.assignment_id
        && assignment.execution_id == fence.execution_id
        && assignment.agent_id == fence.agent_id
        && assignment.version == fence.assignment_version
}

/// Revalidate authorization before idempotent readback, even after reassignment.
pub fn authorize(
    state: &TaskState,
    actor: &Principal,
    config: &SdlcConfig,
    command: &SdlcCommand,
) -> Result<(), AppError> {
    match command {
        SdlcCommand::Answer { .. } | SdlcCommand::Confirm { .. } => require_owner(state, actor),
        SdlcCommand::Assign(_) => {
            if !actor.human_session
                && !config.orchestrator_subject.is_empty()
                && actor.subject == config.orchestrator_subject
                && actor.scopes.contains("task-tracker:sdlc:assign")
            {
                Ok(())
            } else {
                Err(AppError::Forbidden)
            }
        }
        _ => {
            let assignment = state
                .assignment
                .as_ref()
                .ok_or_else(|| AppError::conflict("PM assignment absent"))?;
            let fence = match command {
                SdlcCommand::PublishQuestion(c) => &c.fence,
                SdlcCommand::PublishRevision(c) => &c.fence,
                SdlcCommand::Cancel { command, .. } => &command.fence,
                SdlcCommand::Evidence(c) => &c.fence,
                _ => unreachable!(),
            };
            if actor.human_session {
                return Err(AppError::Forbidden);
            }
            if !fence_matches(assignment, fence) {
                return Err(AppError::conflict("stale PM assignment fence"));
            }
            if matches!(command, SdlcCommand::Evidence(_)) {
                if config.verifier_subject.is_empty() {
                    return Err(AppError::Unavailable(
                        "trusted readiness verifier is not configured".into(),
                    ));
                }
                if actor.subject != config.verifier_subject
                    || actor.subject == assignment.machine_subject
                    || !actor.scopes.contains(&format!(
                        "task-tracker:sdlc:evidence:{}:{}",
                        state.task_id, assignment.version
                    ))
                {
                    return Err(AppError::Forbidden);
                }
            } else if actor.subject != assignment.machine_subject
                || !actor.scopes.contains(&assignment.scope(state.task_id))
            {
                return Err(AppError::Forbidden);
            }
            Ok(())
        }
    }
}

pub fn validate_document(document: &RequirementsDocument) -> Result<(), AppError> {
    nonblank(&document.goal, "goal")?;
    if document.scope.is_empty()
        || document.scenarios.is_empty()
        || document.acceptance_criteria.is_empty()
        || document.checklist.is_empty()
        || document.prerequisites.is_empty()
    {
        return Err(AppError::validation(
            "scope, scenarios, acceptance_criteria, checklist and prerequisites must be nonempty",
        ));
    }
    for item in document
        .scope
        .iter()
        .chain(&document.exclusions)
        .chain(&document.scenarios)
        .chain(&document.acceptance_criteria)
        .chain(&document.constraints)
        .chain(&document.dependencies)
        .chain(&document.assumptions)
        .chain(&document.checklist)
        .chain(&document.prerequisites)
    {
        nonblank(item, "document item")?;
    }
    let checks: Vec<_> = document
        .checklist
        .iter()
        .chain(&document.prerequisites)
        .collect();
    if checks.iter().collect::<HashSet<_>>().len() != checks.len() {
        return Err(AppError::validation(
            "check IDs must be unique across checklist and prerequisites",
        ));
    }
    Ok(())
}

pub fn validate_answer(question: &Question, command: &AnswerCommand) -> Result<(), AppError> {
    let selected: HashSet<_> = command.selected_option_ids.iter().collect();
    if selected.len() != command.selected_option_ids.len() {
        return Err(AppError::validation("duplicate option IDs"));
    }
    if selected
        .iter()
        .any(|id| !question.options.iter().any(|option| &option.id == *id))
    {
        return Err(AppError::validation("unknown option ID"));
    }
    let needs_text = question.mode == QuestionMode::Text
        || question
            .options
            .iter()
            .any(|option| option.is_custom && selected.contains(&option.id));
    if needs_text
        && command
            .text
            .as_deref()
            .is_none_or(|text| text.trim().is_empty())
    {
        return Err(AppError::validation("text is required"));
    }
    match question.mode {
        QuestionMode::Single if selected.len() != 1 => {
            return Err(AppError::validation("single requires exactly one option"));
        }
        QuestionMode::Multiple if selected.is_empty() => {
            return Err(AppError::validation(
                "multiple requires at least one option",
            ));
        }
        QuestionMode::Text if !selected.is_empty() => {
            return Err(AppError::validation("text mode does not accept options"));
        }
        _ => {}
    }
    if command.text.as_ref().is_some_and(|s| s.len() > 100_000)
        || command.comment.as_ref().is_some_and(|s| s.len() > 100_000)
    {
        return Err(AppError::validation("answer too large"));
    }
    Ok(())
}

/// Called only under the repository's task lock; all effects commit with the outbox.
pub fn apply(
    state: &mut TaskState,
    actor: &Principal,
    config: &SdlcConfig,
    command: &SdlcCommand,
) -> Result<Value, AppError> {
    authorize(state, actor, config, command)?;
    validate_key(command.key())?;
    let now = chrono::Utc::now();
    let result = match command {
        SdlcCommand::Assign(c) => {
            if state.assignment.as_ref().map(|a| a.version) != c.expected_assignment_version {
                return Err(AppError::conflict("stale assignment version"));
            }
            if c.assignment.version != next_version(c.expected_assignment_version.unwrap_or(0))?
                || c.assignment.machine_subject.trim().is_empty()
                || c.assignment.machine_subject == state.owner_subject
                || c.assignment.machine_subject == config.verifier_subject
            {
                return Err(AppError::validation("invalid assignment"));
            }
            state.assignment = Some(c.assignment.clone());
            state.confirmation_revision = None;
            // Questions from a replaced execution cannot resume the new assignment.
            for q in &mut state.questions {
                if q.state == QuestionState::Open {
                    q.state = QuestionState::Superseded;
                }
            }
            serde_json::to_value(&c.assignment)
        }
        SdlcCommand::PublishRevision(c) => {
            if state.current_revision() != c.expected_requirement_revision {
                return Err(AppError::conflict("stale requirements revision"));
            }
            if state
                .questions
                .iter()
                .any(|q| q.required && q.state == QuestionState::Open)
            {
                return Err(AppError::conflict("mandatory questions are still open"));
            }
            validate_document(&c.document)?;
            let revision = RequirementsRevision {
                revision: next_version(state.current_revision().unwrap_or(0))?,
                content_hash: canonical_hash(&c.document)?,
                document: c.document.clone(),
                author_subject: actor.subject.clone(),
                created_at: now,
            };
            for q in &mut state.questions {
                if q.state == QuestionState::Open {
                    q.state = QuestionState::Superseded;
                }
            }
            state.revisions.push(revision.clone());
            state.confirmation_revision = Some(revision.revision);
            state.stage = Stage::Clarification;
            serde_json::to_value(revision)
        }
        SdlcCommand::PublishQuestion(c) => {
            if matches!(state.stage, Stage::Backlog)
                || state.current_revision() != Some(c.requirement_revision)
            {
                return Err(AppError::conflict(
                    "question requires current unconfirmed revision",
                ));
            }
            nonblank(&c.text, "question text")?;
            nonblank(&c.rationale, "rationale")?;
            let ids: HashSet<_> = c.options.iter().map(|o| o.id).collect();
            if ids.len() != c.options.len()
                || (c.mode == QuestionMode::Text && !c.options.is_empty())
                || (c.mode != QuestionMode::Text && c.options.is_empty())
                || c.recommended_option_id.is_some_and(|id| !ids.contains(&id))
            {
                return Err(AppError::validation(
                    "invalid question options/recommendation",
                ));
            }
            for option in &c.options {
                nonblank(&option.label, "option label")?;
                nonblank(&option.consequences, "option consequences")?;
            }
            let previous = state.questions.iter().position(|q| q.id == c.question_id);
            let version = if let Some(index) = previous {
                let old = &state.questions[index];
                if Some(old.version) != c.expected_question_version {
                    return Err(AppError::conflict("stale question version"));
                }
                if old.request_id != c.request_id {
                    return Err(AppError::conflict("request binding is immutable"));
                }
                for option in &c.options {
                    if old.options.iter().any(|o| {
                        o.id == option.id
                            && (o.label != option.label
                                || o.consequences != option.consequences
                                || o.is_custom != option.is_custom)
                    }) {
                        return Err(AppError::validation(
                            "changed options require new stable IDs",
                        ));
                    }
                }
                next_version(old.version)?
            } else {
                if c.expected_question_version.is_some() {
                    return Err(AppError::conflict("question absent"));
                }
                1
            };
            let q = Question {
                id: c.question_id,
                request_id: c.request_id,
                task_id: state.task_id,
                root_task_id: state.root_task_id,
                version,
                requirement_revision: c.requirement_revision,
                requirement_reference: c.requirement_reference.clone(),
                assignment_id: c.fence.assignment_id,
                execution_id: c.fence.execution_id,
                agent_id: c.fence.agent_id,
                assignment_version: c.fence.assignment_version,
                checkpoint_id: c.checkpoint_id,
                text: c.text.clone(),
                rationale: c.rationale.clone(),
                required: c.required,
                mode: c.mode.clone(),
                options: c.options.clone(),
                recommended_option_id: c.recommended_option_id,
                state: QuestionState::Open,
                answer: None,
                author_subject: actor.subject.clone(),
                created_at: now,
            };
            if let Some(index) = previous {
                state.questions[index] = q.clone();
            } else {
                state.questions.push(q.clone());
            }
            state.stage = Stage::Clarification;
            state.confirmation_revision = None;
            serde_json::to_value(q)
        }
        SdlcCommand::Answer {
            question_id,
            command: c,
        } => {
            if state.current_revision() != Some(c.requirement_revision) {
                return Err(AppError::conflict("stale requirements revision"));
            }
            let q = state
                .questions
                .iter_mut()
                .find(|q| q.id == *question_id)
                .ok_or_else(|| AppError::not_found("question", question_id))?;
            if q.version != c.expected_question_version
                || q.requirement_revision != c.requirement_revision
                || q.state != QuestionState::Open
            {
                return Err(AppError::conflict("question is stale or not open"));
            }
            validate_answer(q, c)?;
            let answer = Answer {
                id: Uuid::new_v4(),
                question_id: *question_id,
                question_version: q.version,
                requirement_revision: c.requirement_revision,
                selected_option_ids: c.selected_option_ids.clone(),
                text: c.text.clone(),
                comment: c.comment.clone(),
                author_subject: actor.subject.clone(),
                created_at: now,
            };
            q.answer = Some(answer.clone());
            q.state = QuestionState::Answered;
            serde_json::to_value(answer)
        }
        SdlcCommand::Confirm {
            revision,
            command: c,
        } => {
            if config.verifier_subject.is_empty() {
                return Err(AppError::Unavailable(
                    "trusted readiness verifier is not configured".into(),
                ));
            }
            let current = state
                .revisions
                .last()
                .ok_or_else(|| AppError::conflict("requirements absent"))?;
            if current.revision != *revision || current.content_hash != c.content_hash {
                return Err(AppError::conflict("stale requirements revision/hash"));
            }
            if !state.ready() {
                return Err(AppError::conflict(
                    "requirements are not ready for confirmation",
                ));
            }
            let confirmation = Confirmation {
                id: Uuid::new_v4(),
                task_id: state.task_id,
                revision: *revision,
                content_hash: c.content_hash.clone(),
                owner_subject: actor.subject.clone(),
                created_at: now,
                stage: Stage::Backlog,
            };
            state.confirmations.push(confirmation.clone());
            state.stage = Stage::Backlog;
            serde_json::to_value(confirmation)
        }
        SdlcCommand::Evidence(c) => {
            let revision = state
                .revisions
                .last()
                .ok_or_else(|| AppError::conflict("requirements absent"))?;
            if revision.revision != c.requirement_revision
                || revision.content_hash != c.content_hash
            {
                return Err(AppError::conflict("stale evidence revision/hash"));
            }
            if !revision.document.checklist.contains(&c.check_id)
                && !revision.document.prerequisites.contains(&c.check_id)
            {
                return Err(AppError::validation("unknown check ID"));
            }
            nonblank(&c.evidence_reference, "evidence reference")?;
            let evidence = Evidence {
                requirement_revision: c.requirement_revision,
                content_hash: c.content_hash.clone(),
                check_id: c.check_id.clone(),
                evidence_reference: c.evidence_reference.clone(),
                verifier_subject: actor.subject.clone(),
                created_at: now,
            };
            state.evidence.push(evidence.clone());
            serde_json::to_value(evidence)
        }
        SdlcCommand::Cancel {
            question_id,
            command: c,
        } => {
            let q = state
                .questions
                .iter_mut()
                .find(|q| q.id == *question_id)
                .ok_or_else(|| AppError::not_found("question", question_id))?;
            if q.version != c.expected_question_version || q.state != QuestionState::Open {
                return Err(AppError::conflict("question is stale or not open"));
            }
            q.state = QuestionState::Cancelled;
            serde_json::to_value(q)
        }
    };
    result.map_err(AppError::internal)
}
