use super::*;
use sea_orm::{Database, DatabaseConnection, DbBackend, Statement};

async fn db() -> DatabaseConnection {
    let url =
        std::env::var("TT_PROJECT_COMPAT_TEST_DATABASE_URL").expect("disposable test DB required");
    let db = Database::connect(url).await.unwrap();
    let row = db.query_one(Statement::from_string(DbBackend::Postgres,
        "SELECT current_database() AS name,to_regclass('public.seaql_migrations') IS NULL AS fresh".to_owned())).await.unwrap().unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_pdlc4_compat_test")
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    db
}
async fn value(db: &DatabaseConnection, sql: &str) -> serde_json::Value {
    db.query_one(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "v")
        .unwrap()
}
struct LegacyBeforeSource;
struct CatalogBeforePmRecovery;
struct LegacyCatalogBeforePmRecovery;

#[async_trait::async_trait]
impl MigratorTrait for CatalogBeforePmRecovery {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        PublishedMigrator::migrations()
            .into_iter()
            .filter(|migration| {
                !matches!(
                    migration.name(),
                    "m20261018_0000098_pm_unsubmitted_recovery"
                        | "m20261021_0000099_pm_bootstrap_retry"
                        | "m20261019_0000099_unified_spaces"
                        | "m20261021_0000100_human_sdlc_spaces"
                        | "m20261021_0000101_native_sdlc_spaces"
                        | "m20261022_0000102_request_commit_clock"
                )
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl MigratorTrait for LegacyCatalogBeforePmRecovery {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        ProjectRepositoryMigrator::migrations()
            .into_iter()
            .filter(|migration| {
                !matches!(
                    migration.name(),
                    "m20261018_0000098_pm_unsubmitted_recovery"
                        | "m20261021_0000099_pm_bootstrap_retry"
                        | "m20261019_0000099_unified_spaces"
                        | "m20261021_0000100_human_sdlc_spaces"
                        | "m20261021_0000101_native_sdlc_spaces"
                        | "m20261022_0000102_request_commit_clock"
                )
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl MigratorTrait for LegacyBeforeSource {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let mut list = PublishedMigrator::migrations();
        let index = list
            .iter()
            .position(|m| m.name() == "m20261017_0000097_task_source_binding")
            .unwrap();
        list.truncate(index);
        list.push(Box::new(m20261016_0000096_project_repositories::Migration));
        list
    }
}
async fn project_fixture(db: &DatabaseConnection) {
    db.execute_unprepared(r#"
INSERT INTO users(id,email,username,display_name,password_hash) VALUES
('00000000-0000-4000-8000-000000000001','compat@example.test','compat','compat','!');
INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES
('00000000-0000-4000-8000-000000000002','CMP','Same repository name','00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000003'),
('00000000-0000-4000-8000-000000000004','EMP','Empty neighbor','00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000005');
INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'compat','[]' FROM projects;
"#).await.unwrap();
}
#[tokio::test]
#[ignore = "requires fresh owned *_pdlc4_compat_test PostgreSQL"]
async fn project_compat_legacy_preserves_links_and_ledger_without_invented_task_links() {
    let db = db().await;
    LegacyBeforeSource::up(&db, None).await.unwrap();
    project_fixture(&db).await;
    db.execute_unprepared(r#"INSERT INTO project_repository_links(project_id,forge_instance_id,repository_id,snapshot,created_by) VALUES
('00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000011','{"public_name":"same/name","kind":"hosted"}','00000000-0000-4000-8000-000000000001'),
('00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000012','{"public_name":"same/name","kind":"external"}','00000000-0000-4000-8000-000000000001');"#).await.unwrap();
    let before = value(
        &db,
        "SELECT jsonb_agg(to_jsonb(p) ORDER BY repository_id) AS v FROM project_repository_links p",
    )
    .await;
    let ledger = value(
        &db,
        "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS v FROM seaql_migrations m",
    )
    .await;
    Migrator::up(&db, None).await.unwrap();
    assert_eq!(before,value(&db,"SELECT jsonb_agg(to_jsonb(p) ORDER BY repository_id) AS v FROM project_repository_links p").await);
    assert_eq!(
        value(
            &db,
            "SELECT to_jsonb(count(*)) AS v FROM task_repository_links"
        )
        .await,
        serde_json::json!(0)
    );
    assert_eq!(ledger,value(&db,"SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS v FROM seaql_migrations m WHERE version NOT IN ('m20261016_0000096_task_source_compatibility','m20261017_0000097_task_source_binding','m20261018_0000098_pm_unsubmitted_recovery','m20261018_0000098_project_repository_catalog','m20261021_0000099_pm_bootstrap_retry','m20261019_0000099_unified_spaces','m20261021_0000100_human_sdlc_spaces','m20261021_0000101_native_sdlc_spaces','m20261022_0000102_request_commit_clock')").await);
    let once = value(
        &db,
        "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS v FROM seaql_migrations m",
    )
    .await;
    Migrator::up(&db, None).await.unwrap();
    assert_eq!(
        once,
        value(
            &db,
            "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS v FROM seaql_migrations m"
        )
        .await
    );
}
#[tokio::test]
#[ignore = "requires fresh owned *_pdlc4_compat_test PostgreSQL"]
async fn project_compat_published_source_history_remains_valid() {
    let db = db().await;
    PublishedMigrator::up(
        &db,
        Some(
            PublishedMigrator::migrations()
                .iter()
                .position(|m| m.name() == "m20261018_0000098_project_repository_catalog")
                .unwrap() as u32,
        ),
    )
    .await
    .unwrap();
    let ledger = value(
        &db,
        "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS v FROM seaql_migrations m",
    )
    .await;
    Migrator::up(&db, None).await.unwrap();
    assert_eq!(ledger,value(&db,"SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS v FROM seaql_migrations m WHERE version NOT IN ('m20261018_0000098_project_repository_catalog','m20261021_0000099_pm_bootstrap_retry','m20261019_0000099_unified_spaces','m20261021_0000100_human_sdlc_spaces','m20261021_0000101_native_sdlc_spaces','m20261022_0000102_request_commit_clock')").await);
    assert_eq!(value(&db,"SELECT to_jsonb(count(*)) AS v FROM seaql_migrations WHERE version='m20261016_0000096_project_repositories'").await,serde_json::json!(0));
}

async fn assert_catalog_before_pm_history(db: &DatabaseConnection) {
    project_fixture(db).await;
    db.execute_unprepared(r#"INSERT INTO project_repository_links(project_id,forge_instance_id,repository_id,snapshot,created_by) VALUES
('00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000011','{"public_name":"preserved/repository"}','00000000-0000-4000-8000-000000000001');"#).await.unwrap();
    let links = value(
        db,
        "SELECT jsonb_agg(to_jsonb(p) ORDER BY repository_id) AS v FROM project_repository_links p",
    )
    .await;
    let ledger_before = value(
        db,
        "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS v FROM seaql_migrations m",
    )
    .await;
    Migrator::up(db, None).await.unwrap();
    assert_eq!(ledger_before, value(db,
        "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS v FROM seaql_migrations m WHERE version NOT IN ('m20261018_0000098_pm_unsubmitted_recovery','m20261021_0000099_pm_bootstrap_retry','m20261019_0000099_unified_spaces','m20261021_0000100_human_sdlc_spaces','m20261021_0000101_native_sdlc_spaces','m20261022_0000102_request_commit_clock')").await);
    assert_eq!(links, value(db,
        "SELECT jsonb_agg(to_jsonb(p) ORDER BY repository_id) AS v FROM project_repository_links p").await);
    assert_eq!(value(db,
        "SELECT to_jsonb(count(*)) AS v FROM information_schema.columns WHERE table_schema='public' AND table_name='sdlc_pm_lease_operations' AND column_name='recovery'").await,serde_json::json!(1));
    let once = value(
        db,
        "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS v FROM seaql_migrations m",
    )
    .await;
    Migrator::up(db, None).await.unwrap();
    assert_eq!(
        once,
        value(
            db,
            "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS v FROM seaql_migrations m"
        )
        .await
    );
    assert_eq!(links, value(db,
        "SELECT jsonb_agg(to_jsonb(p) ORDER BY repository_id) AS v FROM project_repository_links p").await);
}

#[tokio::test]
#[ignore = "requires fresh owned *_pdlc4_compat_test PostgreSQL"]
async fn project_compat_catalog_before_pm_history_remains_valid() {
    let db = db().await;
    CatalogBeforePmRecovery::up(&db, None).await.unwrap();
    assert_catalog_before_pm_history(&db).await;
}

#[tokio::test]
#[ignore = "requires fresh owned *_pdlc4_compat_test PostgreSQL"]
async fn project_compat_legacy_catalog_before_pm_history_remains_valid() {
    let db = db().await;
    LegacyBeforeSource::up(&db, None).await.unwrap();
    LegacyCatalogBeforePmRecovery::up(&db, None).await.unwrap();
    assert_catalog_before_pm_history(&db).await;
}

#[tokio::test]
#[ignore = "requires fresh owned *_pdlc4_compat_test PostgreSQL"]
async fn project_compat_fresh_and_unknown_history() {
    let db = db().await;
    Migrator::up(&db, None).await.unwrap();
    assert_eq!(value(&db,"SELECT to_jsonb(count(*)) AS v FROM information_schema.tables WHERE table_schema='public' AND table_name IN ('project_repository_links','task_repository_links','sdlc_task_source_bindings')").await,serde_json::json!(3));
    db.execute_unprepared(
        "INSERT INTO seaql_migrations(version,applied_at) VALUES('unknown_legacy_migration',0)",
    )
    .await
    .unwrap();
    assert!(Migrator::up(&db, None).await.is_err());
}
