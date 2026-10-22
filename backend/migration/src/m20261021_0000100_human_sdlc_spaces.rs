use sea_orm_migration::prelude::*;
#[derive(DeriveMigrationName)]
pub struct Migration;
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("human_sdlc_spaces.sql"))
            .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let original = include_str!("unified_spaces.sql");
        let start = original
            .find("CREATE OR REPLACE FUNCTION tracker_namespace_write_guard()")
            .unwrap();
        let end = original[start..].find("\n\n-- The original links").unwrap() + start;
        manager
            .get_connection()
            .execute_unprepared(&original[start..end])
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP FUNCTION tracker_authorize_human_sdlc(text,text,jsonb,jsonb)")
            .await?;
        Ok(())
    }
}
