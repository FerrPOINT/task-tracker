use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Anchors and pg_get_functiondef must use the same EOL after Git export.
        let sql = include_str!("agent_stage_status.sql").replace("\r\n", "\n");
        manager.get_connection().execute_unprepared(&sql).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "agent stage status preserves immutable starts/completions; forward-only".into(),
        ))
    }
}
