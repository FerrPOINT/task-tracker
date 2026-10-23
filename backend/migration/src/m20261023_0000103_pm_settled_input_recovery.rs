use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("pm_settled_input_recovery.sql"))
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "Settled-input recovery receipts are immutable; destructive rollback is disabled"
                .into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, MockDatabase};

    #[tokio::test]
    async fn down_preserves_settled_input_receipts_without_sql() {
        let db = MockDatabase::new(DbBackend::Postgres).into_connection();
        assert!(Migration.down(&SchemaManager::new(&db)).await.is_err());
        assert!(db.into_transaction_log().is_empty());
    }
}
