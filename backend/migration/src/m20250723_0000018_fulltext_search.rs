use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        // Add tsvector column for full-text search on issues.
        conn.execute_unprepared("ALTER TABLE issues ADD COLUMN IF NOT EXISTS tsv_search tsvector")
            .await?;

        // Populate the tsvector from summary and description.
        conn.execute_unprepared(
            "UPDATE issues SET tsv_search = \
             setweight(to_tsvector('simple', coalesce(summary, '')), 'A') || \
             setweight(to_tsvector('simple', coalesce(description, '')), 'B')",
        )
        .await?;

        // GIN index for fast full-text queries.
        conn.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_issues_tsv_search ON issues USING GIN (tsv_search)",
        )
        .await?;

        // Trigger to keep tsv_search in sync on INSERT/UPDATE.
        conn.execute_unprepared(
            "CREATE OR REPLACE FUNCTION issues_tsv_search_update() RETURNS trigger AS $$ \
             BEGIN \
               NEW.tsv_search := \
                 setweight(to_tsvector('simple', coalesce(NEW.summary, '')), 'A') || \
                 setweight(to_tsvector('simple', coalesce(NEW.description, '')), 'B'); \
               RETURN NEW; \
             END; \
             $$ LANGUAGE plpgsql",
        )
        .await?;

        conn.execute_unprepared("DROP TRIGGER IF EXISTS issues_tsv_search_insert ON issues")
            .await?;
        conn.execute_unprepared(
            "CREATE TRIGGER issues_tsv_search_insert \
             BEFORE INSERT ON issues \
             FOR EACH ROW EXECUTE FUNCTION issues_tsv_search_update()",
        )
        .await?;

        conn.execute_unprepared("DROP TRIGGER IF EXISTS issues_tsv_search_update ON issues")
            .await?;
        conn.execute_unprepared(
            "CREATE TRIGGER issues_tsv_search_update \
             BEFORE UPDATE OF summary, description ON issues \
             FOR EACH ROW EXECUTE FUNCTION issues_tsv_search_update()",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        conn.execute_unprepared("DROP TRIGGER IF EXISTS issues_tsv_search_update ON issues")
            .await?;
        conn.execute_unprepared("DROP TRIGGER IF EXISTS issues_tsv_search_insert ON issues")
            .await?;
        conn.execute_unprepared("DROP FUNCTION IF EXISTS issues_tsv_search_update()")
            .await?;
        conn.execute_unprepared("DROP INDEX IF EXISTS idx_issues_tsv_search")
            .await?;
        conn.execute_unprepared("ALTER TABLE issues DROP COLUMN IF EXISTS tsv_search")
            .await?;

        Ok(())
    }
}
