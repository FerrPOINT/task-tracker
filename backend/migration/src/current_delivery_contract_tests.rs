use super::*;
use sea_orm::{Database, DatabaseConnection};

#[test]
fn accepted_histories_have_distinct_immutable_tails_and_one_shared_119() {
    for (installed, migrations) in [
        (false, crate::PublishedMigrator::migrations()),
        (false, crate::ProjectRepositoryMigrator::migrations()),
        (true, crate::InstalledOwnershipMigrator::migrations()),
        (true, crate::InstalledProjectRepositoryMigrator::migrations()),
    ] {
        let names: Vec<_> = migrations.iter().map(|m| m.name()).collect();
        assert_eq!(names[names.len()-3], Migration.name());
        assert_eq!(names[names.len()-2], "m20261114_0000120_routing_resume");
        assert_eq!(names.last().copied(), Some("m20261114_0000121_explicit_recovery"));
        assert_eq!(names.iter().filter(|name| **name == Migration.name()).count(), 1);
        assert_eq!(names.contains(&"m20261113_0000118_delivery_unsent_stop"), installed);
        assert_eq!(names.contains(&"m20261111_0000118_plan_content_dispatch"), !installed);
        assert_eq!(names.len(), names.iter().collect::<std::collections::HashSet<_>>().len());
    }
}

#[test]
fn known_guards_preserve_authority_and_foreign_bodies_are_rejected() {
    let primary = primary_head().unwrap();
    assert!(primary.contains("next_lease=previous_lease-ARRAY['heartbeat_at','expires_at']"));
    assert!(primary.contains("sdlc_critical_pm_returns"));
    assert!(primary.contains("canonical transaction permit"));
    assert!(primary.contains("SDLC exact untimed delivery binding required"));
    require_guard(&primary, &primary).unwrap();
    assert!(require_guard(&primary.replace("FOR UPDATE", ""), &primary).is_err());
    assert!(replace_once(&primary, "absent", "").is_err());
}

async fn ledger(db: &DatabaseConnection) -> Vec<(String, i64)> {
    db.query_all(Statement::from_string(DbBackend::Postgres,
        "SELECT version,applied_at FROM seaql_migrations ORDER BY version".to_owned()))
        .await.unwrap().into_iter().map(|row|
            (row.try_get("", "version").unwrap(), row.try_get("", "applied_at").unwrap())).collect()
}

async fn fresh_db() -> DatabaseConnection {
    let url = std::env::var("TT_PROJECT_COMPAT_TEST_DATABASE_URL")
        .expect("fresh owned *_pdlc4_compat_test database required");
    let db = Database::connect(url).await.unwrap();
    let row = db.query_one(Statement::from_string(DbBackend::Postgres,
        "SELECT current_database() AS name,to_regclass('public.seaql_migrations') IS NULL AS fresh".to_owned()))
        .await.unwrap().unwrap();
    assert!(row.try_get::<String>("", "name").unwrap().ends_with("_pdlc4_compat_test"));
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    db
}

async fn qualification<M: MigratorTrait>(fresh: bool, foreign: bool) {
    let db = fresh_db().await;
    if fresh {
        crate::Migrator::up(&db, None).await.unwrap();
    } else {
        let before = M::migrations().iter().position(|m| m.name() == Migration.name()).unwrap();
        M::up(&db, Some(before.try_into().unwrap())).await.unwrap();
        let original = ledger(&db).await;
        if foreign {
            db.execute_unprepared("CREATE OR REPLACE FUNCTION public.sdlc_delivery_head_gate() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RETURN NEW; END$$;")
                .await.unwrap();
            let changed = function(&db, "sdlc_delivery_head_gate").await.unwrap();
            assert!(crate::Migrator::up(&db, None).await.is_err());
            assert_eq!(ledger(&db).await, original);
            assert_eq!(function(&db, "sdlc_delivery_head_gate").await.unwrap(), changed);
            return;
        }
        crate::Migrator::up(&db, None).await.unwrap();
        let after = ledger(&db).await;
        let tail = [Migration.name(), "m20261114_0000120_routing_resume", "m20261114_0000121_explicit_recovery"];
        assert_eq!(after.iter().filter(|(name, _)| !tail.contains(&name.as_str())).cloned().collect::<Vec<_>>(), original);
        assert_eq!(after.len(), original.len() + tail.len());
        for name in tail {
            assert_eq!(after.iter().filter(|(version, _)| version == name).count(), 1);
        }
    }
    assert_eq!(function(&db, "sdlc_delivery_head_gate").await.unwrap(), primary_head().unwrap());
    let reconciliation = function(&db, "sdlc_delivery_reconciliation_gate").await.unwrap();
    assert!(reconciliation.contains("SDLC unsent Stop requires the unbound original reservation"));
    assert!(reconciliation.contains("IS DISTINCT FROM 'stop'"));
    assert!(reconciliation.contains("= (head#>>'{lease,version}')::bigint"));
    let accepted = ledger(&db).await;
    crate::Migrator::up(&db, None).await.unwrap();
    assert_eq!(ledger(&db).await, accepted);
}

#[tokio::test]
#[ignore = "requires fresh owned TT_PROJECT_COMPAT_TEST_DATABASE_URL ending _pdlc4_compat_test"]
async fn fresh_history_selects_current_contract() {
    qualification::<crate::PublishedMigrator>(true, false).await;
}

#[tokio::test]
#[ignore = "requires fresh owned TT_PROJECT_COMPAT_TEST_DATABASE_URL ending _pdlc4_compat_test"]
async fn published_118_appends_only_current_contract() {
    qualification::<crate::PublishedMigrator>(false, false).await;
}

#[tokio::test]
#[ignore = "requires fresh owned TT_PROJECT_COMPAT_TEST_DATABASE_URL ending _pdlc4_compat_test"]
async fn installed_118_appends_only_current_contract() {
    qualification::<crate::InstalledOwnershipMigrator>(false, false).await;
}

#[tokio::test]
#[ignore = "requires fresh owned TT_PROJECT_COMPAT_TEST_DATABASE_URL ending _pdlc4_compat_test"]
async fn installed_118_refuses_a_foreign_guard_without_writes() {
    qualification::<crate::InstalledOwnershipMigrator>(false, true).await;
}

// Exact names observed by Root on native Relevanter; no production rows or credentials.
const RELEVANTER_30: [&str; 30] = [
    "m20250723_0000015_workflow_and_issue_types",
    "m20250723_0000016_labels",
    "m20250723_0000017_issue_links",
    "m20250723_0000018_fulltext_search",
    "m20250723_000001_create_tables",
    "m20260824_0000020_notifications",
    "m20260824_0000021_admin_audit_settings",
    "m20260824_0000022_performance_indexes",
    "m20260825_0000023_watchers_votes",
    "m20260825_0000024_issue_soft_delete",
    "m20260825_0000025_components_versions",
    "m20260825_0000026_custom_fields",
    "m20260826_0000027_fk_indexes",
    "m20260827_0000028_core_fk_constraints",
    "m20260831_0000029_notification_digest_state",
    "m20260914_0000030_totp",
    "m20260914_0000031_password_reset",
    "m20260914_0000032_oidc",
    "m20260918_0000033_central_subject",
    "m20261001_0000034_sdlc_clarification",
    "m20261008_0000090_namespace",
    "m20261010_0000091_pm_native_admission",
    "m20261010_0000092_pm_state_write_permits",
    "m20261012_0000093_fleet_mcp_admission",
    "m20261012_0000094_analysis_result",
    "m20261012_0000095_pm_completion",
    "m20261013_0000096_single_task_delivery",
    "m20261017_0000097_task_source_binding",
    "m20261018_0000098_pm_unsubmitted_recovery",
    "m20261018_0000098_project_repository_catalog",
];

struct Relevanter30;

#[async_trait::async_trait]
impl MigratorTrait for Relevanter30 {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let migrations = crate::PublishedMigrator::migrations();
        let count = migrations.iter().position(|m|
            m.name() == "m20261018_0000098_project_repository_catalog").unwrap() + 1;
        migrations.into_iter().take(count).collect()
    }
}

#[test]
fn relevanter_ledger_is_the_exact_accepted_published_prefix() {
    let mut expected = RELEVANTER_30.to_vec();
    let migrations = Relevanter30::migrations();
    let mut names: Vec<_> = migrations.iter().map(|m| m.name()).collect();
    expected.sort_unstable();
    names.sort_unstable();
    assert_eq!(names, expected);
}

#[tokio::test]
#[ignore = "requires fresh owned TT_PROJECT_COMPAT_TEST_DATABASE_URL ending _pdlc4_compat_test"]
async fn relevanter_exact_30_name_ledger_upgrades_without_rewriting_history() {
    let db = fresh_db().await;
    Relevanter30::up(&db, None).await.unwrap();
    let original = ledger(&db).await;
    let mut expected = RELEVANTER_30.to_vec();
    expected.sort_unstable();
    assert_eq!(original.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), expected);
    crate::Migrator::up(&db, None).await.unwrap();
    let current = ledger(&db).await;
    assert_eq!(current.iter().filter(|(name, _)| RELEVANTER_30.contains(&name.as_str()))
        .cloned().collect::<Vec<_>>(), original);
    assert_eq!(function(&db, "sdlc_delivery_head_gate").await.unwrap(), primary_head().unwrap());
    let accepted = ledger(&db).await;
    crate::Migrator::up(&db, None).await.unwrap();
    assert_eq!(ledger(&db).await, accepted);
}

#[tokio::test]
#[ignore = "requires fresh owned TT_PROJECT_COMPAT_TEST_DATABASE_URL ending _pdlc4_compat_test"]
async fn current_120_refuses_a_foreign_recovery_guard_without_writes() {
    let db = fresh_db().await;
    let count = crate::PublishedMigrator::migrations().iter().position(|m|
        m.name() == "m20261114_0000121_explicit_recovery").unwrap();
    crate::PublishedMigrator::up(&db, Some(count.try_into().unwrap())).await.unwrap();
    let original = ledger(&db).await;
    db.execute_unprepared("CREATE OR REPLACE FUNCTION public.sdlc_delivery_reconciliation_gate() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RETURN NEW; END$$;")
        .await.unwrap();
    let changed = function(&db, "sdlc_delivery_reconciliation_gate").await.unwrap();
    assert!(crate::Migrator::up(&db, None).await.is_err());
    assert_eq!(ledger(&db).await, original);
    assert_eq!(function(&db, "sdlc_delivery_reconciliation_gate").await.unwrap(), changed);
}
