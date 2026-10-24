use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("identity_deletion.sql"))
            .await?;
        manager
            .get_connection()
            .execute_unprepared(include_str!("identity_reference_fences.sql"))
            .await?;
        manager
            .get_connection()
            .execute_unprepared(include_str!("identity_json_references.sql"))
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "identity alias history requires compatible cohort rollback without data removal"
                .into(),
        ))
    }
}
