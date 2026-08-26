use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Foreign-key columns that lacked an index in the original migrations.
        // These are frequently queried in JOIN / WHERE clauses and should be
        // indexed for performance.

        // issues.reporter_id — used for dashboard & notification queries
        create_idx(
            manager,
            Issues::Table,
            "idx_issues_reporter_id",
            Issues::ReporterId,
        )
        .await?;

        // comments.author_id — used for listing comments by user
        create_idx(
            manager,
            Comments::Table,
            "idx_comments_author_id",
            Comments::AuthorId,
        )
        .await?;

        // attachments.author_id — used for listing attachments by user
        create_idx(
            manager,
            Attachments::Table,
            "idx_attachments_author_id",
            Attachments::AuthorId,
        )
        .await?;

        // worklogs.author_id — used for listing worklogs by user
        create_idx(
            manager,
            Worklogs::Table,
            "idx_worklogs_author_id",
            Worklogs::AuthorId,
        )
        .await?;

        // worklogs.started_at — used for ordering worklogs by start time
        create_idx(
            manager,
            Worklogs::Table,
            "idx_worklogs_started_at",
            Worklogs::StartedAt,
        )
        .await?;

        // projects.owner_id — used for listing projects by owner
        create_idx(
            manager,
            Projects::Table,
            "idx_projects_owner_id",
            Projects::OwnerId,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let names = [
            "idx_projects_owner_id",
            "idx_worklogs_started_at",
            "idx_worklogs_author_id",
            "idx_attachments_author_id",
            "idx_comments_author_id",
            "idx_issues_reporter_id",
        ];
        for name in names {
            manager
                .drop_index(
                    Index::drop()
                        .if_exists()
                        .name(name)
                        .table(Issues::Table) // table is ignored by name in PG
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

async fn create_idx<'a>(
    manager: &SchemaManager<'a>,
    table: impl IntoIden + 'a + 'static,
    name: &'a str,
    column: impl IntoIden + 'a + 'static,
) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name(name)
                .table(table)
                .col(column)
                .to_owned(),
        )
        .await
}

#[derive(DeriveIden)]
enum Issues {
    Table,
    ReporterId,
}

#[derive(DeriveIden)]
enum Comments {
    Table,
    AuthorId,
}

#[derive(DeriveIden)]
enum Attachments {
    Table,
    AuthorId,
}

#[derive(DeriveIden)]
enum Worklogs {
    Table,
    AuthorId,
    StartedAt,
}

#[derive(DeriveIden)]
enum Projects {
    Table,
    OwnerId,
}
