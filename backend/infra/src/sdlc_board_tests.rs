use super::*;
use domain::sdlc_lifecycle::{LifecycleStage, LifecycleStatus};
use sea_orm::{MockDatabase, Transaction};
use std::collections::BTreeMap;

fn repository(db: DatabaseConnection) -> PostgresSdlcRepository {
    PostgresSdlcRepository {
        db: crate::guarded_db::GuardedDb::new(db),
        config: SdlcConfig {
            instance_id: "board-reader-test".into(),
            orchestrator_subject: "orchestrator".into(),
            verifier_subject: "verifier".into(),
            reservation_scheduler_subject: "scheduler".into(),
            single_task_project_ids: Default::default(),
        },
        file_storage: None,
    }
}

fn row(id: Uuid, stage: Json, status: Option<Json>) -> BTreeMap<&'static str, Value> {
    BTreeMap::from([
        ("task_id", id.into()),
        ("stage", stage.into()),
        ("status", status.map(Value::from).unwrap_or(Value::Json(None))),
    ])
}

#[tokio::test]
async fn board_reader_batches_only_authorized_issue_ids_in_project_and_instance() {
    let project = Uuid::new_v4();
    let draft = Uuid::new_v4();
    let active = Uuid::new_v4();
    let ordinary = Uuid::new_v4();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![
            row(draft, json!("Clarification"), None),
            row(active, json!("Development"), Some(json!("InProgress"))),
        ]])
        .into_connection();
    let repo = repository(db);
    let tasks = repo.board_tasks(project, &[draft, active, ordinary]).await.unwrap();
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].issue_id, draft);
    assert_eq!(tasks[0].stage, LifecycleStage::Draft);
    assert_eq!(tasks[0].status, None);
    assert_eq!(tasks[1].issue_id, active);
    assert_eq!(tasks[1].stage, LifecycleStage::Development);
    assert_eq!(tasks[1].status, Some(LifecycleStatus::InProgress));
    // No synthesized stage or status for an ordinary issue absent from the SDLC table.
    assert!(!tasks.iter().any(|task| task.issue_id == ordinary));
    assert_eq!(repo.db.raw().into_transaction_log(), vec![Transaction::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT task_id, state->'stage' AS stage,
                        NULLIF(state->'status', 'null'::jsonb) AS status
                 FROM sdlc_tasks
                 WHERE project_id = $1 AND tracker_instance_id = $2 AND task_id = ANY($3)",
        vec![project.into(), "board-reader-test".into(), Value::Array(
            sea_orm::sea_query::ArrayType::Uuid,
            Some(Box::new(vec![draft.into(), active.into(), ordinary.into()])),
        )],
    )]);
}

#[tokio::test]
async fn board_reader_reports_invalid_persisted_stage_instead_of_empty_success() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![row(Uuid::new_v4(), json!("UnknownStage"), None)]])
        .into_connection();
    assert!(repository(db).board_tasks(Uuid::new_v4(), &[]).await.is_err());
}
