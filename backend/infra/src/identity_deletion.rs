//! Product-owned identity reference barrier; no runtime dispatch permission.
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait,
};
use serde_json::Value;
use shared::{AppError, identity_deletion::*};
use uuid::Uuid;

const SOURCES: &[&str] = &[
    "users",
    "tracker_identity_aliases",
    "projects",
    "project_members",
    "issues",
    "comments",
    "worklogs",
    "attachments",
    "issue_status_history",
    "issue_watchers",
    "issue_votes",
    "notifications",
    "notification_user_settings",
    "audit_logs",
    "totp",
    "password_reset_token",
    "oidc_identity",
    "project_repository_links",
    "task_repository_links",
    "issue_creation_receipts",
    "sdlc_tasks",
    "sdlc_analysis_reservation_leases",
    "sdlc_pm_execution_leases",
    "sdlc_analysis_reservation_operations",
    "sdlc_analysis_result_operations",
    "sdlc_critical_blocker_operations",
    "sdlc_critical_pm_lifecycle_operations",
    "sdlc_critical_pm_returns",
    "sdlc_critical_pm_reviews",
    "sdlc_delivery_operations",
    "sdlc_delivery_proposal_operations",
    "sdlc_draft_creations",
    "sdlc_draft_source_enrollments",
    "sdlc_idempotency",
    "sdlc_pm_completion_operations",
    "sdlc_pm_feedback",
    "sdlc_pm_lease_operations",
    "sdlc_pm_resume_inputs",
    "sdlc_project_routing_operations",
    "sdlc_source_policy_operations",
    "tracker_space_owner_operations",
    "sdlc_analysis_intents",
    "sdlc_analysis_results",
    "sdlc_answers",
    "sdlc_assignments",
    "sdlc_confirmations",
    "sdlc_critical_blockers",
    "sdlc_delivery_assignments",
    "sdlc_delivery_proposals",
    "sdlc_delivery_reconciliations",
    "sdlc_delivery_results",
    "sdlc_delivery_run_bindings",
    "sdlc_evidence",
    "sdlc_options",
    "sdlc_outbox",
    "sdlc_pm_completions",
    "sdlc_pm_executions",
    "sdlc_project_routing_revisions",
    "sdlc_question_versions",
    "sdlc_requirements",
    "sdlc_reservation_capacity",
    "sdlc_source_policy_revisions",
    "sdlc_task_routing_generations",
    "sdlc_task_routing_snapshots",
    "sdlc_task_source_bindings",
    "sdlc_delivery_heads",
    "sdlc_delivery_projection_permits",
    "sdlc_analysis_state_write_permits",
    "sdlc_pm_state_write_permits",
    "tracker_execution_space_grants",
    "tracker_namespace_bindings",
    "tracker_project_lifecycle_operations",
    "tracker_project_space_operations",
    "tracker_project_space_outbox",
];
fn storage(_: impl std::fmt::Display) -> AppError {
    AppError::Unavailable("identity_deletion_storage_unavailable".into())
}
// Only the released request-clock diagnostic is an expired authority. Keep
// every unrelated database failure classified as unavailable.
fn commit_error(error: sea_orm::DbErr) -> AppError {
    match &error {
        sea_orm::DbErr::Exec(sea_orm::RuntimeErr::SqlxError(sea_orm::SqlxError::Database(db)))
        | sea_orm::DbErr::Query(sea_orm::RuntimeErr::SqlxError(sea_orm::SqlxError::Database(db)))
            if db.code().as_deref() == Some("42501")
                && db.message() == "pdlc_request_authority_expired" =>
        {
            AppError::Unauthorized
        }
        _ => storage(error),
    }
}
fn conflict() -> AppError {
    AppError::conflict("identity_deletion_operation_conflict")
}
fn unknown() -> AppError {
    AppError::Unavailable("identity_binding_unknown".into())
}

pub struct IdentityDeletionStore<'a> {
    db: &'a DatabaseConnection,
}
impl<'a> IdentityDeletionStore<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }
    /// Explicit deployment assertion; no email or local UUID inference.
    pub async fn bind_identity_legacy_issuer(&self, issuer: &str) -> Result<(), AppError> {
        let url = reqwest::Url::parse(issuer)
            .map_err(|_| AppError::validation("invalid_identity_issuer"))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(AppError::validation("invalid_identity_issuer"));
        }
        let tx = self.db.begin().await.map_err(storage)?;
        lock(&tx).await?;
        let row = tx
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT issuer FROM tracker_identity_legacy_issuer WHERE singleton",
            ))
            .await
            .map_err(storage)?;
        if let Some(row) = row {
            let bound: String = row.try_get("", "issuer").map_err(storage)?;
            if bound != issuer {
                return Err(conflict());
            }
        } else {
            tx.execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO tracker_identity_legacy_issuer(issuer) VALUES($1)",
                [issuer.into()],
            ))
            .await
            .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)
    }
    async fn identity_owner_begin(
        &self,
        request: &DeletionRequest,
        window: &IdentityDeletionWindow,
    ) -> Result<DatabaseTransaction, AppError> {
        if !request.valid() {
            return Err(AppError::validation("invalid_identity_deletion_request"));
        }
        window.require_current()?;
        let tx = self.db.begin().await.map_err(storage)?;
        let anchor: chrono::DateTime<chrono::Utc> = tx
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT clock_timestamp() AS anchor",
            ))
            .await
            .map_err(storage)?
            .ok_or_else(unknown)?
            .try_get("", "anchor")
            .map_err(storage)?;
        let until = window.commit_valid_until(anchor)?;
        super::guarded_db::bind_commit_deadline(&tx, until)
            .await
            .map_err(storage)?;
        lock(&tx).await?;
        window.require_current()?;
        let row = tx
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT issuer FROM tracker_identity_legacy_issuer WHERE singleton",
            ))
            .await
            .map_err(storage)?
            .ok_or_else(unknown)?;
        let bound: String = row.try_get("", "issuer").map_err(storage)?;
        if bound != request.identity.issuer {
            return Err(unknown());
        }
        Ok(tx)
    }
    pub async fn identity_owner_check(
        &self,
        request: &DeletionRequest,
        window: &IdentityDeletionWindow,
    ) -> Result<OwnerReferenceSummary, AppError> {
        let tx = self.identity_owner_begin(request, window).await?;
        window.require_current()?;
        verify_operation(&tx, request).await?;
        let reference_count = count(&tx, request).await?;
        let revision = revision(&tx).await?;
        window.require_current()?;
        tx.commit().await.map_err(commit_error)?;
        Ok(OwnerReferenceSummary {
            request: request.clone(),
            owner: "task-tracker".into(),
            revision,
            reference_count,
            fencing_supported: true,
        })
    }
    pub async fn identity_owner_freeze(
        &self,
        request: &DeletionRequest,
        window: &IdentityDeletionWindow,
    ) -> Result<OwnerFreezeReceipt, AppError> {
        let tx = self.identity_owner_begin(request, window).await?;
        window.require_current()?;
        if let Some(row) = verify_operation(&tx, request).await? {
            let state: String = row.try_get("", "state").map_err(storage)?;
            if state != "frozen" {
                return Err(conflict());
            }
            let receipt = OwnerFreezeReceipt {
                request: request.clone(),
                owner: "task-tracker".into(),
                fence: row.try_get("", "fence").map_err(storage)?,
                revision: row.try_get::<i64>("", "revision").map_err(storage)? as u64,
                reference_count: row.try_get::<i64>("", "reference_count").map_err(storage)? as u64,
            };
            window.require_current()?;
            tx.commit().await.map_err(commit_error)?;
            return Ok(receipt);
        }
        let active=tx.query_one(Statement::from_sql_and_values(DbBackend::Postgres,"SELECT EXISTS(SELECT 1 FROM tracker_identity_deletion_operations WHERE issuer=$1 AND subject=$2 AND state='frozen') AS active",[request.identity.issuer.clone().into(),request.identity.subject.clone().into()])).await.map_err(storage)?.ok_or_else(unknown)?.try_get::<bool>("","active").map_err(storage)?;
        if active {
            return Err(conflict());
        }
        let reference_count = count(&tx, request).await?;
        let revision = revision(&tx).await?;
        let fence = Uuid::new_v4();
        tx.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO tracker_identity_deletion_operations(operation_id,issuer,subject,inventory_revision,request,state,fence,revision,reference_count) VALUES($1,$2,$3,$4,$5,'frozen',$6,$7,$8)",[
            request.operation_id.into(),request.identity.issuer.clone().into(),request.identity.subject.clone().into(),request.inventory_revision.into(),serde_json::to_value(request).map_err(storage)?.into(),fence.into(),(revision as i64).into(),(reference_count as i64).into()
        ])).await.map_err(storage)?;
        window.require_current()?;
        tx.commit().await.map_err(commit_error)?;
        Ok(OwnerFreezeReceipt {
            request: request.clone(),
            owner: "task-tracker".into(),
            fence,
            revision,
            reference_count,
        })
    }
    pub async fn identity_owner_release(
        &self,
        request: &DeletionRequest,
        window: &IdentityDeletionWindow,
    ) -> Result<OwnerReleaseReceipt, AppError> {
        let tx = self.identity_owner_begin(request, window).await?;
        window.require_current()?;
        verify_operation(&tx, request).await?;
        let revision = revision(&tx).await?;
        tx.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO tracker_identity_deletion_operations(operation_id,issuer,subject,inventory_revision,request,state,fence,revision,reference_count) VALUES($1,$2,$3,$4,$5,'released',NULL,$6,0) ON CONFLICT(operation_id) DO UPDATE SET state='released',fence=NULL",[
            request.operation_id.into(),request.identity.issuer.clone().into(),request.identity.subject.clone().into(),request.inventory_revision.into(),serde_json::to_value(request).map_err(storage)?.into(),(revision as i64).into()
        ])).await.map_err(storage)?;
        window.require_current()?;
        tx.commit().await.map_err(commit_error)?;
        Ok(OwnerReleaseReceipt {
            request: request.clone(),
            owner: "task-tracker".into(),
            cancelled: true,
        })
    }
}
async fn lock(tx: &DatabaseTransaction) -> Result<(), AppError> {
    tx.execute_unprepared("SET LOCAL lock_timeout='8s'; SET LOCAL statement_timeout='8s'")
        .await
        .map_err(storage)?;
    tx.execute_unprepared("SELECT pg_advisory_xact_lock(714402216002::bigint)")
        .await
        .map_err(storage)?;
    let update = tx
        .execute_unprepared(
            "UPDATE tracker_identity_fence_epoch SET revision=revision+1 WHERE singleton",
        )
        .await
        .map_err(storage)?;
    if update.rows_affected() != 1 {
        return Err(unknown());
    }
    Ok(())
}
async fn verify_operation(
    tx: &DatabaseTransaction,
    request: &DeletionRequest,
) -> Result<Option<QueryResult>, AppError> {
    let row=tx.query_one(Statement::from_sql_and_values(DbBackend::Postgres,"SELECT request,state,fence,revision,reference_count FROM tracker_identity_deletion_operations WHERE operation_id=$1",[request.operation_id.into()])).await.map_err(storage)?;
    if let Some(row) = &row {
        let value: Value = row.try_get("", "request").map_err(storage)?;
        if value != serde_json::to_value(request).map_err(storage)? {
            return Err(conflict());
        }
    }
    Ok(row)
}
async fn revision(tx: &DatabaseTransaction) -> Result<u64, AppError> {
    Ok(tx
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT nextval('tracker_identity_reference_revision') AS revision",
        ))
        .await
        .map_err(storage)?
        .ok_or_else(unknown)?
        .try_get::<i64>("", "revision")
        .map_err(storage)? as u64)
}
async fn count(tx: &DatabaseTransaction, request: &DeletionRequest) -> Result<u64, AppError> {
    let mut count = 0_u64;
    for source in SOURCES {
        let row=tx.query_one(Statement::from_string(DbBackend::Postgres,format!("SELECT EXISTS(SELECT 1 FROM {source} t CROSS JOIN LATERAL tracker_identity_row_refs('{source}',to_jsonb(t)) r WHERE NOT r.resolved OR COALESCE(length(btrim(r.issuer)),0)=0 OR COALESCE(length(btrim(r.subject)),0)=0) AS unknown"))).await.map_err(storage)?.ok_or_else(unknown)?;
        if row.try_get::<bool>("", "unknown").map_err(storage)? {
            return Err(unknown());
        }
        let row=tx.query_one(Statement::from_sql_and_values(DbBackend::Postgres,format!("SELECT count(*) AS count FROM {source} t WHERE EXISTS(SELECT 1 FROM tracker_identity_row_refs('{source}',to_jsonb(t)) r WHERE r.issuer=$1 AND r.subject=$2)"),[request.identity.issuer.clone().into(),request.identity.subject.clone().into()])).await.map_err(storage)?.ok_or_else(unknown)?;
        count = count
            .checked_add(row.try_get::<i64>("", "count").map_err(storage)? as u64)
            .ok_or_else(unknown)?;
    }
    Ok(count)
}
