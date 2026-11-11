use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(&include_str!("reviewer_handoff.sql").replace("\r\n", "\n"))
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "Reviewer handoff is forward-only; accepted history is unchanged".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DbBackend, Statement};

    #[test]
    fn reviewer_handoff_follows_both_116_migrations_in_both_lineages() {
        for migrations in [
            crate::PublishedMigrator::migrations(),
            crate::ProjectRepositoryMigrator::migrations(),
        ] {
            let names: Vec<_> = migrations.iter().map(|m| m.name()).collect();
            assert_eq!(
                names
                    .iter()
                    .filter(|name| **name == Migration.name())
                    .count(),
                1
            );
            assert_eq!(
                &names[names.len() - 3..],
                &[
                    "m20261108_0000116_reviewer_terminal",
                    "m20261110_0000116_untimed_delivery_binding",
                    Migration.name(),
                ],
            );
        }
    }

    async fn assert_upgrade<M: MigratorTrait>() {
        let url = std::env::var("TT_PROJECT_COMPAT_TEST_DATABASE_URL")
            .expect("fresh owned *_pdlc4_compat_test database required");
        let db = Database::connect(url).await.unwrap();
        let row = db.query_one(Statement::from_string(DbBackend::Postgres,
            "SELECT current_database() AS name,to_regclass('public.seaql_migrations') IS NULL AS fresh".to_owned()))
            .await.unwrap().unwrap();
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
        M::up(
            &db,
            Some(<u32 as std::convert::TryFrom<usize>>::try_from(before).unwrap()),
        )
        .await
        .unwrap();
        let ledger = db
            .query_all(Statement::from_string(
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
            .collect::<Vec<_>>();
        let old = db.query_one(Statement::from_string(DbBackend::Postgres,
            "SELECT pg_get_functiondef('sdlc_delivery_result_gate()'::regprocedure) AS definition".to_owned()))
            .await.unwrap().unwrap().try_get::<String>("", "definition").unwrap();
        assert!(old.contains("role IN ('reviewer','devops')"));
        assert!(!old.contains("expires_at"));
        db.execute_unprepared(&old.replace("\r\n", "\n").replace('\n', "\r\n"))
            .await
            .unwrap();
        // Use the public ledger-aware migrator, including the legacy lineage selection.
        crate::Migrator::up(&db, None).await.unwrap();
        let new = db.query_one(Statement::from_string(DbBackend::Postgres,
            "SELECT pg_get_functiondef('sdlc_delivery_result_gate()'::regprocedure) AS definition".to_owned()))
            .await.unwrap().unwrap().try_get::<String>("", "definition").unwrap();
        assert!(new.contains("WHEN 'reviewer' THEN 'Testing'"));
        assert!(new.contains("IF role IN ('reviewer','tester') THEN"));
        assert!(!new.contains("expires_at"));
        assert!(!new.contains("\r\n"));
        for (name, applied_at) in ledger {
            let row = db
                .query_one(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT applied_at FROM seaql_migrations WHERE version=$1",
                    vec![name.into()],
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(row.try_get::<i64>("", "applied_at").unwrap(), applied_at);
        }
        crate::Migrator::up(&db, None).await.unwrap();
        assert!(
            crate::Migrator::get_pending_migrations(&db)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    #[ignore = "requires fresh owned TT_PROJECT_COMPAT_TEST_DATABASE_URL ending _pdlc4_compat_test"]
    async fn reviewer_handoff_upgrades_published_116_without_restoring_time_guards() {
        assert_upgrade::<crate::PublishedMigrator>().await;
    }

    #[tokio::test]
    #[ignore = "requires fresh owned TT_PROJECT_COMPAT_TEST_DATABASE_URL ending _pdlc4_compat_test"]
    async fn reviewer_handoff_upgrades_project_repository_116_without_restoring_time_guards() {
        assert_upgrade::<crate::ProjectRepositoryMigrator>().await;
    }
}
