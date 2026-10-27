use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("pm_manual_recovery_budget.sql"))
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "Manual recovery budget witnesses are immutable; destructive rollback is disabled"
                .into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, MockDatabase};

    #[tokio::test]
    async fn down_preserves_manual_budget_history_without_sql() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        assert!(Migration.down(&SchemaManager::new(&db)).await.is_err());
        assert!(db.into_transaction_log().is_empty());
    }

    #[test]
    fn both_lineages_append_manual_budget_once() {
        let migration = Migration;
        let name = migration.name();
        for migrations in [
            crate::PublishedMigrator::migrations(),
            crate::ProjectRepositoryMigrator::migrations(),
        ] {
            let position = migrations.iter().position(|m| m.name() == name).unwrap();
            assert_eq!(migrations.iter().filter(|m| m.name() == name).count(), 1);
            assert_eq!(
                migrations[position - 1].name(),
                "m20261023_0000103_pm_settled_input_recovery"
            );
        }
    }
}
