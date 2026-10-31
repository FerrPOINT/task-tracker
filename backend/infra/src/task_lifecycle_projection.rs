//! Shared issue projection for human and agent state writers. No execution is implied.
use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, Statement};
use shared::AppError;
use uuid::Uuid;

pub(crate) async fn project_status(
    tx: &DatabaseTransaction,
    task: Uuid,
    actor: Uuid,
    status: Uuid,
) -> Result<(), AppError> {
    // Keep the real local actor active until commit, including ordinary manual work.
    tx.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id FROM users WHERE id=$1 AND is_active FOR SHARE",
        [actor.into()],
    ))
    .await
    .map_err(crate::db_error::map)?
    .ok_or(AppError::Forbidden)?;
    tx.execute(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO issue_status_history(id,issue_id,from_status_id,to_status_id,changed_by_id,created_at)
         SELECT $1,i.id,i.status_id,s.id,$3,clock_timestamp() FROM issues i JOIN statuses s ON s.id=$4
         WHERE i.id=$2 AND i.status_id<>s.id",
        [Uuid::now_v7().into(),task.into(),actor.into(),status.into()]))
        .await.map_err(crate::db_error::map)?;
    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE issues SET status_id=$2,
            sprint_id=CASE WHEN (SELECT enabled FROM task_lifecycle_control WHERE singleton)
                THEN sprint_id ELSE NULL END,
            updated_at=clock_timestamp() WHERE id=$1",
        [task.into(), status.into()],
    ))
    .await
    .map_err(crate::db_error::map)?;
    Ok(())
}
