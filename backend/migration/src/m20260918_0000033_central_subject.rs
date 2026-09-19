use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "ALTER TABLE users ADD COLUMN IF NOT EXISTS central_sub text; \
             ALTER TABLE users DROP CONSTRAINT IF EXISTS users_email_key; \
             CREATE UNIQUE INDEX IF NOT EXISTS users_legacy_email_idx ON users (lower(email)) WHERE central_sub IS NULL; \
             CREATE UNIQUE INDEX IF NOT EXISTS users_central_sub_idx ON users (central_sub) WHERE central_sub IS NOT NULL;"
        ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DO $$ BEGIN IF EXISTS (SELECT 1 FROM users WHERE central_sub IS NOT NULL) \
                 THEN RAISE EXCEPTION 'cannot remove central subject while central profiles exist'; END IF; END $$; \
             DROP INDEX IF EXISTS users_central_sub_idx; \
             DROP INDEX IF EXISTS users_legacy_email_idx; \
             ALTER TABLE users DROP COLUMN IF EXISTS central_sub; \
             ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE (email);",
            )
            .await?;
        Ok(())
    }
}
