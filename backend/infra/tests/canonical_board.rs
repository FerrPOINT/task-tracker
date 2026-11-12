use domain::sdlc_lifecycle::{LifecycleStage, LifecycleStatus};
use infra::repos::SeaOrmRepositories;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use shared::{IssueId, ProjectId};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires the owned tracker_board_test PostgreSQL fixture"]
async fn canonical_board_reads_only_authorized_project_and_selected_issues() {
    let url = std::env::var("TRACKER_BOARD_TEST_DATABASE_URL").expect("owned PostgreSQL URL");
    assert!(
        url.contains("/tracker_board_test?"),
        "fixture database must be explicit"
    );
    let db = Database::connect(&url).await.unwrap();
    for ddl in [
        "CREATE TABLE issues(id uuid PRIMARY KEY, project_id uuid NOT NULL)",
        "CREATE TABLE sdlc_instance(instance_id text PRIMARY KEY)",
        "CREATE TABLE sdlc_tasks(task_id uuid PRIMARY KEY, project_id uuid NOT NULL, tracker_instance_id text NOT NULL, state jsonb NOT NULL)",
    ] {
        db.execute_unprepared(ddl).await.unwrap();
    }
    db.execute_unprepared("INSERT INTO sdlc_instance VALUES ('owned')")
        .await
        .unwrap();
    let project = ProjectId::new();
    let other = ProjectId::new();
    let ids: Vec<_> = (0..5).map(|_| IssueId::new()).collect();
    for (n, id) in ids.iter().enumerate() {
        let owner_project = if n == 3 { other } else { project };
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO issues VALUES ($1,$2)",
            [id.as_uuid().into(), owner_project.as_uuid().into()],
        ))
        .await
        .unwrap();
        if n == 4 {
            continue;
        } // legitimate historical issue, no enrollment
        let instance = if n == 2 { "unowned" } else { "owned" };
        let state = if n == 0 {
            serde_json::json!({"stage":"Review","status":"InProgress"})
        } else {
            serde_json::json!({"stage":"Clarification","status":null})
        };
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO sdlc_tasks VALUES ($1,$2,$3,$4)",
            vec![
                id.as_uuid().into(),
                owner_project.as_uuid().into(),
                instance.into(),
                state.into(),
            ],
        ))
        .await
        .unwrap();
    }
    let repository_db = Database::connect(&url).await.unwrap();
    let repo = SeaOrmRepositories::new(repository_db).boards;
    let selected = repo.sdlc_tasks(project, &ids).await.unwrap();
    assert_eq!(selected.len(), 2);
    let native = selected
        .iter()
        .find(|t| t.issue_id == ids[0].as_uuid())
        .unwrap();
    assert!(native.enrolled);
    assert_eq!(native.stage, LifecycleStage::Review);
    assert_eq!(native.status, Some(LifecycleStatus::InProgress));
    let draft = selected
        .iter()
        .find(|t| t.issue_id == ids[1].as_uuid())
        .unwrap();
    assert!(draft.enrolled);
    assert_eq!(draft.stage, LifecycleStage::Draft);
    assert_eq!(draft.status, None);
    assert!(repo.sdlc_tasks(project, &[]).await.unwrap().is_empty());
    let only_one = repo.sdlc_tasks(project, &[ids[1]]).await.unwrap();
    assert_eq!(only_one.len(), 1);
    assert_eq!(only_one[0].issue_id, ids[1].as_uuid());
    // Invalid stored progress must fail rather than becoming a fresh Draft.
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE sdlc_tasks SET state = $1 WHERE task_id = $2",
        vec![
            serde_json::json!({"stage":"NotAStage"}).into(),
            ids[0].as_uuid().into(),
        ],
    ))
    .await
    .unwrap();
    assert!(repo.sdlc_tasks(project, &[ids[0]]).await.is_err());
    assert!(
        repo.sdlc_tasks(project, &[IssueId::from_uuid(Uuid::new_v4())])
            .await
            .unwrap()
            .is_empty()
    );
}
