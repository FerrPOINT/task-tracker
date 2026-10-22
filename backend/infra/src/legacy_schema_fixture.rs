//! Historical seed adapter for the pre-admission migration fixtures only.
//! Current application code cannot prepare an old schema using a policy table
//! that did not exist. Native owner/lease/business checks still run unchanged.
//! This module is absent from production builds; even inside its explicit scope,
//! a current schema always uses the real space policy and fails closed normally.
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
        "SELECT current_database() AS database_name, to_regclass('public.tracker_space_admission_state') IS NULL AS pre_admission"))
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
}
