//! Real old migration objects and repository writes; never a fabricated ledger.
use super::*;
use domain::background_intake::*;
use sea_orm_migration::MigrationTrait;
use std::collections::BTreeMap;

#[path = "../../migration/src/m20261016_0000096_project_repositories.rs"]
mod project_repositories;
#[path = "../../migration/src/m20261016_0000096_task_source_compatibility.rs"]
mod task_source_compatibility;

// Exact PublishedMigrator order from released background SHA 47b71063.
const RELEASED_BACKGROUND: &[&str] = &[
    "m20250723_000001_create_tables",
    "m20250723_0000015_workflow_and_issue_types",
    "m20250723_0000016_labels",
    "m20250723_0000017_issue_links",
    "m20250723_0000018_fulltext_search",
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
    "m20261021_0000099_pm_bootstrap_retry",
    "m20261019_0000099_unified_spaces",
    "m20261021_0000100_human_sdlc_spaces",
    "m20261021_0000101_native_sdlc_spaces",
    "m20261022_0000102_request_commit_clock",
    "m20261022_0000103_identity_deletion",
    "m20261023_0000103_pm_settled_input_recovery",
    "m20261026_0000104_pm_manual_recovery_budget",
    "m20261026_0000105_configured_cli",
    "m20261027_0000106_configured_stage_comments",
    "m20261027_0000107_configured_business_history",
    "m20261027_0000108_configured_critical_lifecycle",
    "m20261028_0000109_configured_initial_adoption",
    "m20261028_0000110_configured_checkpoint_absence",
    "m20261029_0000111_configured_analysis_start",
    "m20261103_0000112_workspace_source_reference",
    "m20261103_0000113_decomposition",
    "m20261103_0000112_configured_analysis_auto_start",
    "m20261104_0000113_native_task_artifacts",
    "m20261106_0000114_native_read_scope_attenuation",
    "m20261107_0000115_agent_stage_status",
    "m20261109_0000116_background_finding_intake",
];

struct ReleasedBackground<const LEGACY: bool>;
struct MainBeforeBackground<const LEGACY: bool>;

fn legacy<const LEGACY: bool>(
    mut list: Vec<Box<dyn MigrationTrait>>,
) -> Vec<Box<dyn MigrationTrait>> {
    if LEGACY {
        let index = list
            .iter()
            .position(|m| m.name() == "m20261017_0000097_task_source_binding")
            .unwrap();
        list.insert(index, Box::new(project_repositories::Migration));
        list.insert(index + 1, Box::new(task_source_compatibility::Migration));
    }
    list
}

#[async_trait::async_trait]
impl<const LEGACY: bool> MigratorTrait for ReleasedBackground<LEGACY> {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let mut list = migration::Migrator::migrations();
        list.retain(|m| RELEASED_BACKGROUND.contains(&m.name()));
        assert_eq!(
            list.iter().map(|m| m.name()).collect::<Vec<_>>(),
            RELEASED_BACKGROUND
        );
        legacy::<LEGACY>(list)
    }
}

#[async_trait::async_trait]
impl<const LEGACY: bool> MigratorTrait for MainBeforeBackground<LEGACY> {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let mut list = migration::Migrator::migrations();
        list.retain(|m| m.name() != "m20261109_0000116_background_finding_intake");
        legacy::<LEGACY>(list)
    }
}

#[test]
fn published_background_history_keeps_its_original_registry_and_objects() {
    assert_eq!(ReleasedBackground::<false>::migrations().len(), 52);
    assert_eq!(ReleasedBackground::<true>::migrations().len(), 54);
}

async fn value(db: &DatabaseConnection, sql: String) -> Json {
    db.query_one(statement(&sql, vec![]))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "document")
        .unwrap()
}

async fn columns(db: &DatabaseConnection) -> BTreeMap<String, String> {
    db.query_all(statement(
        "SELECT c.table_name,string_agg(quote_ident(c.column_name),',' ORDER BY c.ordinal_position) AS columns
         FROM information_schema.columns c JOIN information_schema.tables t
           ON t.table_schema=c.table_schema AND t.table_name=c.table_name
         WHERE c.table_schema='public' AND t.table_type='BASE TABLE' AND c.table_name<>'seaql_migrations'
         GROUP BY c.table_name", vec![],
    )).await.unwrap().into_iter().map(|row| (row.try_get("", "table_name").unwrap(),row.try_get("", "columns").unwrap())).collect()
}

async fn data(
    db: &DatabaseConnection,
    columns: &BTreeMap<String, String>,
) -> BTreeMap<String, Json> {
    let mut values = BTreeMap::new();
    for (table, fields) in columns {
        // Original columns only: additive schema is allowed, every prior value is exact.
        let table_name = table.replace('"', "\"\"");
        let sql = format!(
            "SELECT COALESCE(jsonb_agg(to_jsonb(retained) ORDER BY to_jsonb(retained)::text),'[]'::jsonb) AS document FROM (SELECT {fields} FROM public.\"{table_name}\") retained"
        );
        values.insert(table.clone(), value(db, sql).await);
    }
    values
}

async fn ledger(db: &DatabaseConnection) -> Json {
    value(
        db,
        "SELECT jsonb_agg(to_jsonb(m) ORDER BY version) AS document FROM seaql_migrations m".into(),
    )
    .await
}

fn finding(destination: FindingDestination) -> FindingIntake {
    FindingIntake {
        source: FindingOrigin {
            application: FindingApplication::FleetControl,
            finding_id: Uuid::new_v4(),
            finding_revision: 1,
        },
        destination,
        kind: FindingKind::Bug,
        title: "Retained human finding".into(),
        description: "Human-reviewed description".into(),
        expected: "Keep the contract".into(),
        criteria: "Preserve the original values".into(),
        evidence: "Fixture source SHA and path".into(),
        custom_fields: Default::default(),
    }
}

async fn assert_upgrade<const BACKGROUND: bool, const LEGACY: bool>() {
    let f = if BACKGROUND {
        Fixture::new_with_history::<ReleasedBackground<LEGACY>>(
            None,
            "owner",
            "background-history-test",
        )
        .await
    } else {
        Fixture::new_with_history::<MainBeforeBackground<LEGACY>>(
            None,
            "owner",
            "background-history-test",
        )
        .await
    };
    let draft = f
        .draft("Retained operator input", Some(DraftFlow::SingleTaskV1))
        .await;
    let mut receipts = vec![];
    if BACKGROUND {
        for destination in [FindingDestination::Draft, FindingDestination::Backlog] {
            let operation = Uuid::new_v4();
            let request = finding(destination);
            let (receipt, replay) = f
                .repo
                .intake_finding(f.project, &f.owner, operation, request)
                .await
                .unwrap();
            assert!(!replay);
            receipts.push((operation, receipt));
        }
    }
    let selected_columns = columns(&f.db).await;
    let before = data(&f.db, &selected_columns).await;
    let old_ledger = ledger(&f.db).await;
    migration::Migrator::up(&f.db, None).await.unwrap();
    assert!(
        migration::Migrator::get_pending_migrations(&f.db)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(data(&f.db, &selected_columns).await, before);
    let current_ledger = ledger(&f.db).await;
    for row in old_ledger.as_array().unwrap() {
        assert!(
            current_ledger.as_array().unwrap().contains(row),
            "prior ledger row changed: {row}"
        );
    }
    let state = f.repo.read(draft, &f.owner).await.unwrap();
    assert_eq!(state.stage, Stage::Draft);
    for (operation, receipt) in receipts {
        assert_eq!(
            f.repo
                .finding_intake_operation(f.project, &f.owner, operation)
                .await
                .unwrap(),
            receipt
        );
    }
    migration::Migrator::up(&f.db, None).await.unwrap();
    assert_eq!(ledger(&f.db).await, current_ledger);
    assert_eq!(data(&f.db, &selected_columns).await, before);
}

#[tokio::test]
#[ignore = "requires fresh owned TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn published_background_intakes_survive_current_upgrade() {
    assert_upgrade::<true, false>().await;
}

#[tokio::test]
#[ignore = "requires fresh owned TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn legacy_background_intakes_survive_current_upgrade() {
    assert_upgrade::<true, true>().await;
}

#[tokio::test]
#[ignore = "requires fresh owned TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn published_main_drafts_survive_background_upgrade() {
    assert_upgrade::<false, false>().await;
}

#[tokio::test]
#[ignore = "requires fresh owned TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn legacy_main_drafts_survive_background_upgrade() {
    assert_upgrade::<false, true>().await;
}
