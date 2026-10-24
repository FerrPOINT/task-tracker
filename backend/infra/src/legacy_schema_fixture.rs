//! Historical seed adapter for the pre-admission migration fixtures only.
//! Current application code cannot prepare an old schema using a policy table
//! that did not exist. Native owner/lease/business checks still run unchanged.
//! This module is absent from production builds; even inside its explicit scope,
//! a current or previously upgraded schema uses the real policy and fails closed.
//! A missing current table never makes an already migrated schema historical.
use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement};
use std::future::Future;

tokio::task_local! { static HISTORICAL_SEED: (); }

pub(crate) fn scope<T>(seed: impl Future<Output = T>) -> impl Future<Output = T> {
    HISTORICAL_SEED.scope((), Box::pin(seed))
}

pub(crate) async fn pre_admission_schema(db: &impl ConnectionTrait) -> Result<bool, DbErr> {
    if HISTORICAL_SEED.try_with(|_| ()).is_err() {
        return Ok(false);
    }
    let row = db.query_one(Statement::from_string(DbBackend::Postgres,
        "SELECT current_database() AS database_name, to_regclass('public.tracker_space_admission_state') IS NULL AND NOT EXISTS(SELECT 1 FROM public.seaql_migrations WHERE version='m20261019_0000099_unified_spaces') AS pre_admission"))
        .await?.ok_or_else(|| DbErr::Custom("historical_seed_schema_unavailable".into()))?;
    let database: String = row.try_get("", "database_name")?;
    if !database.ends_with("_candidates_test") && !database.ends_with("_draft_flow_test") {
        return Err(DbErr::Custom(
            "historical_seed_requires_owned_disposable_database".into(),
        ));
    }
    row.try_get("", "pre_admission")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{MockDatabase, Value};
    use std::collections::BTreeMap;

    fn metadata(database: &str, pre_admission: bool) -> BTreeMap<&'static str, Value> {
        BTreeMap::from([
            ("database_name", database.into()),
            ("pre_admission", pre_admission.into()),
        ])
    }

    #[tokio::test]
    async fn ordinary_tests_do_not_select_the_legacy_adapter_or_add_queries() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        assert!(!pre_admission_schema(&db).await.unwrap());
        assert!(db.into_transaction_log().is_empty());
    }

    #[tokio::test]
    async fn explicit_seed_scope_is_only_for_pre_admission_disposable_storage() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([
                [metadata("owned_candidates_test", true)],
                [metadata("owned_candidates_test", false)],
                [metadata("live_database", true)],
            ])
            .into_connection();
        scope(async {
            assert!(pre_admission_schema(&db).await.unwrap());
            assert!(!pre_admission_schema(&db).await.unwrap());
            assert!(pre_admission_schema(&db).await.is_err());
        })
        .await;
    }
    #[tokio::test]
    #[ignore = "requires fresh owned TT_LEGACY_SCHEMA_TEST_DATABASE_URL ending _candidates_test"]
    async fn actual_migration_ledger_does_not_treat_current_schema_loss_as_a_predecessor() {
        use sea_orm_migration::MigratorTrait;
        let url = std::env::var("TT_LEGACY_SCHEMA_TEST_DATABASE_URL").unwrap();
        let db = sea_orm::Database::connect(&url).await.unwrap();
        let row = db.query_one(Statement::from_string(DbBackend::Postgres,
            "SELECT current_database() AS database_name,to_regclass('public.seaql_migrations') IS NULL AS fresh"))
            .await.unwrap().unwrap();
        assert!(
            row.try_get::<String>("", "database_name")
                .unwrap()
                .ends_with("_candidates_test")
        );
        assert!(row.try_get::<bool>("", "fresh").unwrap());
        let before_spaces = migration::Migrator::migrations()
            .iter()
            .position(|m| m.name() == "m20261019_0000099_unified_spaces")
            .unwrap();
        migration::Migrator::up(&db, Some(before_spaces.try_into().unwrap()))
            .await
            .unwrap();
        assert!(scope(pre_admission_schema(&db)).await.unwrap());
        migration::Migrator::up(&db, None).await.unwrap();
        assert!(!scope(pre_admission_schema(&db)).await.unwrap());
        // Corrupt only this disposable fixture after a real successful upgrade.
        // Its retained migration ledger must prevent historical-mode selection.
        db.execute_unprepared("DROP TABLE tracker_space_admission_state")
            .await
            .unwrap();
        assert!(!scope(pre_admission_schema(&db)).await.unwrap());
    }
}
