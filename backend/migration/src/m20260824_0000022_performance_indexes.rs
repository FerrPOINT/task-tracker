use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Composite index for board queries: filter by project + status.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_issues_project_status")
                    .table(Issues::Table)
                    .col(Issues::ProjectId)
                    .col(Issues::StatusId)
                    .to_owned(),
            )
            .await?;

        // Index for sorting issues by created_at descending (recent-first listing).
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_issues_created_at_desc")
                    .table(Issues::Table)
                    .col((Issues::CreatedAt, IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        // Composite index for listing comments within an issue ordered by time.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_comments_issue_created")
                    .table(Comments::Table)
                    .col(Comments::IssueId)
                    .col(Comments::CreatedAt)
                    .to_owned(),
            )
            .await?;

        // Composite index for looking up audit logs by entity.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_audit_logs_entity")
                    .table(AuditLogs::Table)
                    .col(AuditLogs::EntityType)
                    .col(AuditLogs::EntityId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("idx_audit_logs_entity")
                    .table(AuditLogs::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("idx_comments_issue_created")
                    .table(Comments::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("idx_issues_created_at_desc")
                    .table(Issues::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("idx_issues_project_status")
                    .table(Issues::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Issues {
    Table,
    ProjectId,
    StatusId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Comments {
    Table,
    IssueId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum AuditLogs {
    Table,
    EntityType,
    EntityId,
}
