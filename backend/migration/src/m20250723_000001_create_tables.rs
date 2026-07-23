use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Users::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(
                        ColumnDef::new(Users::Email)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Users::Username)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Users::DisplayName).string().not_null())
                    .col(ColumnDef::new(Users::PasswordHash).string().not_null())
                    .col(
                        ColumnDef::new(Users::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .col(
                        ColumnDef::new(Users::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Projects::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Projects::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(
                        ColumnDef::new(Projects::Key)
                            .string_len(10)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Projects::Name).string().not_null())
                    .col(ColumnDef::new(Projects::Description).text())
                    .col(ColumnDef::new(Projects::OwnerId).uuid().not_null())
                    .col(ColumnDef::new(Projects::DefaultBoardId).uuid().not_null())
                    .col(
                        ColumnDef::new(Projects::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .col(
                        ColumnDef::new(Projects::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Boards::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Boards::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(ColumnDef::new(Boards::ProjectId).uuid().not_null())
                    .col(ColumnDef::new(Boards::Name).string().not_null())
                    .col(ColumnDef::new(Boards::Columns).json().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Sprints::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Sprints::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(ColumnDef::new(Sprints::ProjectId).uuid().not_null())
                    .col(ColumnDef::new(Sprints::Name).string().not_null())
                    .col(ColumnDef::new(Sprints::Goal).text())
                    .col(ColumnDef::new(Sprints::State).string_len(16).not_null())
                    .col(ColumnDef::new(Sprints::StartDate).timestamp_with_time_zone())
                    .col(ColumnDef::new(Sprints::EndDate).timestamp_with_time_zone())
                    .col(ColumnDef::new(Sprints::Velocity).big_integer())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Issues::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Issues::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(ColumnDef::new(Issues::ProjectId).uuid().not_null())
                    .col(ColumnDef::new(Issues::Key).string().not_null().unique_key())
                    .col(ColumnDef::new(Issues::IssueType).string_len(16).not_null())
                    .col(ColumnDef::new(Issues::StatusId).uuid().not_null())
                    .col(ColumnDef::new(Issues::Summary).string().not_null())
                    .col(ColumnDef::new(Issues::Description).text())
                    .col(ColumnDef::new(Issues::AssigneeId).uuid())
                    .col(ColumnDef::new(Issues::ReporterId).uuid().not_null())
                    .col(ColumnDef::new(Issues::Priority).string_len(16).not_null())
                    .col(ColumnDef::new(Issues::Labels).json().not_null())
                    .col(ColumnDef::new(Issues::SprintId).uuid())
                    .col(ColumnDef::new(Issues::Position).double().not_null())
                    .col(ColumnDef::new(Issues::DueDate).timestamp_with_time_zone())
                    .col(ColumnDef::new(Issues::OriginalEstimateSeconds).big_integer())
                    .col(ColumnDef::new(Issues::RemainingEstimateSeconds).big_integer())
                    .col(
                        ColumnDef::new(Issues::TimeSpentSeconds)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Issues::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .col(
                        ColumnDef::new(Issues::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Comments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Comments::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(ColumnDef::new(Comments::IssueId).uuid().not_null())
                    .col(ColumnDef::new(Comments::AuthorId).uuid().not_null())
                    .col(ColumnDef::new(Comments::Body).text().not_null())
                    .col(
                        ColumnDef::new(Comments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .col(
                        ColumnDef::new(Comments::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Attachments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Attachments::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(ColumnDef::new(Attachments::IssueId).uuid().not_null())
                    .col(ColumnDef::new(Attachments::AuthorId).uuid().not_null())
                    .col(ColumnDef::new(Attachments::FileName).string().not_null())
                    .col(ColumnDef::new(Attachments::ContentType).string().not_null())
                    .col(
                        ColumnDef::new(Attachments::SizeBytes)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Attachments::StorageKey).string().not_null())
                    .col(
                        ColumnDef::new(Attachments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Labels::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Labels::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(ColumnDef::new(Labels::ProjectId).uuid().not_null())
                    .col(ColumnDef::new(Labels::Name).string().not_null())
                    .col(ColumnDef::new(Labels::Color).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProjectMembers::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ProjectMembers::ProjectId).uuid().not_null())
                    .col(ColumnDef::new(ProjectMembers::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(ProjectMembers::Role)
                            .string_len(16)
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_project_members")
                            .table(ProjectMembers::Table)
                            .col(ProjectMembers::ProjectId)
                            .col(ProjectMembers::UserId),
                    )
                    .to_owned(),
            )
            .await?;

        // indexes
        create_idx(
            manager,
            Issues::Table,
            "idx_issues_project_id",
            Issues::ProjectId,
        )
        .await?;
        create_idx(
            manager,
            Issues::Table,
            "idx_issues_status_id",
            Issues::StatusId,
        )
        .await?;
        create_idx(
            manager,
            Issues::Table,
            "idx_issues_assignee_id",
            Issues::AssigneeId,
        )
        .await?;
        create_idx(
            manager,
            Issues::Table,
            "idx_issues_sprint_id",
            Issues::SprintId,
        )
        .await?;
        create_idx(
            manager,
            Comments::Table,
            "idx_comments_issue_id",
            Comments::IssueId,
        )
        .await?;
        create_idx(
            manager,
            Attachments::Table,
            "idx_attachments_issue_id",
            Attachments::IssueId,
        )
        .await?;
        create_idx(
            manager,
            Boards::Table,
            "idx_boards_project_id",
            Boards::ProjectId,
        )
        .await?;
        create_idx(
            manager,
            Sprints::Table,
            "idx_sprints_project_id",
            Sprints::ProjectId,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ProjectMembers::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Labels::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Attachments::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Comments::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Issues::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Sprints::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Boards::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Projects::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Users::Table).if_exists().to_owned())
            .await?;
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
enum Users {
    Table,
    Id,
    Email,
    Username,
    DisplayName,
    PasswordHash,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Projects {
    Table,
    Id,
    Key,
    Name,
    Description,
    OwnerId,
    DefaultBoardId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Boards {
    Table,
    Id,
    ProjectId,
    Name,
    Columns,
}

#[derive(DeriveIden)]
enum Sprints {
    Table,
    Id,
    ProjectId,
    Name,
    Goal,
    State,
    StartDate,
    EndDate,
    Velocity,
}

#[derive(DeriveIden)]
enum Issues {
    Table,
    Id,
    ProjectId,
    Key,
    IssueType,
    StatusId,
    Summary,
    Description,
    AssigneeId,
    ReporterId,
    Priority,
    Labels,
    SprintId,
    Position,
    DueDate,
    OriginalEstimateSeconds,
    RemainingEstimateSeconds,
    TimeSpentSeconds,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Comments {
    Table,
    Id,
    IssueId,
    AuthorId,
    Body,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Attachments {
    Table,
    Id,
    IssueId,
    AuthorId,
    FileName,
    ContentType,
    SizeBytes,
    StorageKey,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Labels {
    Table,
    Id,
    ProjectId,
    Name,
    Color,
}

#[derive(DeriveIden)]
enum ProjectMembers {
    Table,
    ProjectId,
    UserId,
    Role,
}
