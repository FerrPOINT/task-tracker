use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(&include_str!("plan_content_dispatch.sql").replace("\r\n", "\n"))
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "Plan content authorization is forward-only; saved history is unchanged".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DatabaseConnection, DbBackend, Statement};

    #[test]
    fn plan_dispatch_follows_linear_delivery_in_both_lineages() {
        for migrations in [
            crate::PublishedMigrator::migrations(),
            crate::ProjectRepositoryMigrator::migrations(),
        ] {
            let names: Vec<_> = migrations.iter().map(|m| m.name()).collect();
            let index = names
                .iter()
                .position(|name| *name == Migration.name())
                .unwrap();
            assert_eq!(
                names
                    .iter()
                    .filter(|name| **name == Migration.name())
                    .count(),
                1
            );
            assert_eq!(names[index - 1], "m20261110_0000117_linear_delivery");
        }
    }

    async fn ledger(db: &DatabaseConnection) -> Vec<(String, i64)> {
        db.query_all(Statement::from_string(
            DbBackend::Postgres,
            "SELECT version,applied_at FROM seaql_migrations ORDER BY version".to_owned(),
        ))
        .await
        .unwrap()
        .into_iter()
        .map(|row| {
            (
                row.try_get::<String>("", "version").unwrap(),
                row.try_get::<i64>("", "applied_at").unwrap(),
            )
        })
        .collect()
    }

    async fn definition(db: &DatabaseConnection) -> (i64, String) {
        let row = db
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT p.oid::bigint AS oid,pg_get_functiondef(p.oid) AS definition FROM pg_proc p WHERE p.oid='tracker_authorize_project_content(text,text,jsonb,jsonb)'::regprocedure".to_owned(),
            ))
            .await
            .unwrap()
            .unwrap();
        (
            row.try_get("", "oid").unwrap(),
            row.try_get("", "definition").unwrap(),
        )
    }

    async fn assert_upgrade<M: MigratorTrait>() {
        let url = std::env::var("TT_PROJECT_COMPAT_TEST_DATABASE_URL")
            .expect("fresh owned *_pdlc4_compat_test database required");
        let db = Database::connect(url).await.unwrap();
        let row = db
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT current_database() AS name,to_regclass('public.seaql_migrations') IS NULL AS fresh".to_owned(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(
            row.try_get::<String>("", "name")
                .unwrap()
                .ends_with("_pdlc4_compat_test")
        );
        assert!(row.try_get::<bool>("", "fresh").unwrap());
        let before = M::migrations()
            .iter()
            .position(|m| m.name() == Migration.name())
            .unwrap();
        assert_eq!(
            M::migrations()[before - 1].name(),
            "m20261110_0000117_linear_delivery"
        );
        M::up(
            &db,
            Some(<u32 as std::convert::TryFrom<usize>>::try_from(before).unwrap()),
        )
        .await
        .unwrap();
        let original_ledger = ledger(&db).await;
        let (oid, old) = definition(&db).await;
        assert!(old.contains("tracker_authorize_native_task_content"));
        assert!(!old.contains("issue_plan_revisions"));

        crate::Migrator::up(&db, None).await.unwrap();
        let (same_oid, restored) = definition(&db).await;
        assert_eq!(oid, same_oid);
        assert!(restored.contains("'issue_plan_revisions','issue_plan_operations'"));
        assert!(
            restored.contains("tracker_authorize_project_content_before_decomposition('comments'")
        );
        assert!(
            restored
                .contains("tracker_authorize_native_task_content(table_name,operation,desired)")
        );
        assert!(restored.contains("tracker_authorize_project_content_before_configured_comments"));
        let current_ledger = ledger(&db).await;
        assert_eq!(
            current_ledger
                .iter()
                .filter(|(name, _)| name != Migration.name())
                .cloned()
                .collect::<Vec<_>>(),
            original_ledger,
        );
        assert_eq!(current_ledger.len(), original_ledger.len() + 1);
        crate::Migrator::up(&db, None).await.unwrap();
        assert_eq!(definition(&db).await, (oid, restored));
        assert_eq!(ledger(&db).await, current_ledger);
        assert!(Migration.down(&SchemaManager::new(&db)).await.is_err());
        assert_eq!(ledger(&db).await, current_ledger);
    }

    #[tokio::test]
    #[ignore = "requires fresh owned TT_PROJECT_COMPAT_TEST_DATABASE_URL ending _pdlc4_compat_test"]
    async fn plan_dispatch_upgrades_published_117_preserving_ledger() {
        assert_upgrade::<crate::PublishedMigrator>().await;
    }

    #[tokio::test]
    #[ignore = "requires fresh owned TT_PROJECT_COMPAT_TEST_DATABASE_URL ending _pdlc4_compat_test"]
    async fn plan_dispatch_upgrades_project_repository_117_preserving_ledger() {
        assert_upgrade::<crate::ProjectRepositoryMigrator>().await;
    }
}
