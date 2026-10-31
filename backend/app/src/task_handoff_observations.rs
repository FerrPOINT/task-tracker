//! Fresh owner facts for a queued human request, never native execution admission.
use crate::sdlc_configuration::{FleetConfigurationReader, MatchedConfiguration};
use domain::{
    sdlc_source::VerifiedSource, task_handoff::PreparedCurrentStageHandoff,
    task_repositories::TaskRepositoriesReader,
};
use shared::AppError;

/// No Deserialize: HTTP callers supply only the human command, not owner evidence.
/// Private expected inputs bind both observations to the exact prepared selection.
pub struct MatchedHandoffInputs {
    expected: PreparedCurrentStageHandoff,
    fleet: MatchedConfiguration,
    source: VerifiedSource,
}

impl MatchedHandoffInputs {
    /// The caller obtains these typed observations through the registered readers.
    /// This matches configuration/source facts, not an assignment or process claim.
    pub fn bind(
        prepared: &PreparedCurrentStageHandoff,
        fleet: MatchedConfiguration,
        source: VerifiedSource,
    ) -> Result<Self, AppError> {
        let matched = Self {
            expected: prepared.clone(),
            fleet,
            source,
        };
        matched.ensure_for(prepared)?;
        Ok(matched)
    }

    /// Safe owner observation metadata for the immutable queue audit. No tokens,
    /// native claims or renewed timestamps are included.
    pub fn audit(&self) -> serde_json::Value {
        serde_json::json!({"schema_version":1,
            "fleet_observation_ref":self.fleet.observation_ref,
            "fleet_selection_sha256":self.fleet.selection_sha256,
            "fleet_observed_at":self.fleet.observed_at,
            "source_observed_at":self.source.observed_at,
            "source_repository_id":self.source.repository_id,
            "source_namespace":self.source.namespace,
            "issue_materials_sha256":self.expected.issue_materials_sha256,
            "valid_until":std::cmp::min(self.fleet.observed_at+chrono::Duration::seconds(5),self.source.observed_at+chrono::Duration::seconds(30))})
    }

    /// Re-run at the locked owner commit after all HTTP has finished. Never renew
    /// either observation timestamp or the original request authorization deadline.
    pub fn ensure_for(&self, prepared: &PreparedCurrentStageHandoff) -> Result<(), AppError> {
        let before = &self.expected;
        if !prepared.blockers.is_empty()
            || prepared.entry.is_none()
            || before.task_id != prepared.task_id
            || before.project_id != prepared.project_id
            || before.owner_subject != prepared.owner_subject
            || before.state != prepared.state
            || before.issue_sha256 != prepared.issue_sha256
            || before.issue_materials_sha256 != prepared.issue_materials_sha256
            || before.entry != prepared.entry
            || before.routing != prepared.routing
            || before.source != prepared.source
        {
            return Err(AppError::conflict(
                "current_stage_handoff_observation_inputs_changed",
            ));
        }
        if self.fleet.observation_ref.is_nil() {
            return Err(AppError::conflict(
                "current_stage_handoff_configuration_observation_missing",
            ));
        }
        let entry = prepared
            .entry
            .as_ref()
            .ok_or_else(|| AppError::conflict("current_stage_handoff_entry_missing"))?;
        let routing = prepared
            .routing
            .as_ref()
            .ok_or_else(|| AppError::Unavailable("routing_not_configured".into()))?;
        self.fleet
            .ensure_selection(entry.route(routing), entry.role.key(), entry.mode.key())?;
        let source_policy = prepared
            .source
            .as_ref()
            .ok_or_else(|| AppError::Unavailable("source_not_configured".into()))?;
        crate::sdlc_source::validate_verification(
            &source_policy.source,
            &source_policy.verification.namespace,
            &self.source,
            chrono::Utc::now(),
        )?;
        Ok(())
    }
}

/// Preserve the failing owner for a meaningful readiness preview. These errors
/// remain failures on a mutation; no fallback or synthetic observation is made.
pub enum ObservationError {
    Fleet(AppError),
    Source(AppError),
    Inputs(AppError),
}
impl ObservationError {
    pub fn into_app(self) -> AppError {
        match self {
            Self::Fleet(e) | Self::Source(e) | Self::Inputs(e) => e,
        }
    }
    pub fn blocker(self) -> Result<domain::task_handoff::TaskExecutionBlocker, AppError> {
        use domain::task_handoff::TaskExecutionBlocker as Blocker;
        match self {
            Self::Source(AppError::Forbidden) => Err(AppError::Forbidden),
            Self::Fleet(AppError::Conflict(_)) | Self::Inputs(AppError::Conflict(_)) => {
                Ok(Blocker::ConfigurationChanged)
            }
            Self::Fleet(_) => Ok(Blocker::FleetUnavailable),
            Self::Source(_) => Ok(Blocker::SourceUnavailable),
            Self::Inputs(e) => Err(e),
        }
    }
}

/// Read peers outside a database transaction, then bind the original observations.
/// The repository must call ensure_for with a freshly locked plan before queueing.
pub async fn observe_detailed(
    prepared: &PreparedCurrentStageHandoff,
    fleet: &FleetConfigurationReader,
    repositories: &dyn TaskRepositoriesReader,
) -> Result<MatchedHandoffInputs, ObservationError> {
    let inputs = |e| ObservationError::Inputs(e);
    if !prepared.blockers.is_empty() {
        return Err(inputs(AppError::conflict(
            "current_stage_handoff_inputs_blocked",
        )));
    }
    let entry = prepared
        .entry
        .as_ref()
        .ok_or_else(|| inputs(AppError::conflict("current_stage_handoff_entry_missing")))?;
    let policy = prepared
        .routing
        .as_ref()
        .ok_or_else(|| inputs(AppError::Unavailable("routing_not_configured".into())))?;
    let source = prepared
        .source
        .as_ref()
        .ok_or_else(|| inputs(AppError::Unavailable("source_not_configured".into())))?;
    let (fleet, source) = tokio::try_join!(
        async {
            fleet
                .read(entry.route(policy), entry.role.key(), entry.mode.key())
                .await
                .map_err(ObservationError::Fleet)
        },
        async {
            repositories
                .verify_source(&source.verification.namespace, &source.source)
                .await
                .map_err(ObservationError::Source)
        },
    )?;
    MatchedHandoffInputs::bind(prepared, fleet, source).map_err(inputs)
}

pub async fn observe(
    prepared: &PreparedCurrentStageHandoff,
    fleet: &FleetConfigurationReader,
    repositories: &dyn TaskRepositoriesReader,
) -> Result<MatchedHandoffInputs, AppError> {
    observe_detailed(prepared, fleet, repositories)
        .await
        .map_err(ObservationError::into_app)
}
