use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Base migration creates `labels` (id/project_id/name/color). Here we only
        // add the created_at column for installs where the base table already exists;
        // IF NOT EXISTS makes the CREATE a no-op in that case.
        manager
            .alter_table(
                Table::alter()
                    .table(Labels::Table)
                    .add_column(
                        ColumnDef::new(Labels::CreatedAt)
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
                    .col(
                        ColumnDef::new(Labels::Color)
                            .string_len(7)
                            .not_null()
                            .default("#6b7280"),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_labels_project")
                            .from(Labels::Table, Labels::ProjectId)
                            .to(Projects::Table, Projects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_labels_project")
                    .table(Labels::Table)
                    .col(Labels::ProjectId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_labels_project_name")
                    .table(Labels::Table)
                    .col(Labels::ProjectId)
                    .col(Labels::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Join table issue <-> label
        manager
            .create_table(
                Table::create()
                    .table(IssueLabels::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(IssueLabels::IssueId).uuid().not_null())
                    .col(ColumnDef::new(IssueLabels::LabelId).uuid().not_null())
                    .primary_key(
                        Index::create()
                            .name("pk_issue_labels")
                            .col(IssueLabels::IssueId)
                            .col(IssueLabels::LabelId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_issue_labels_issue")
                            .from(IssueLabels::Table, IssueLabels::IssueId)
                            .to(Issues::Table, Issues::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_issue_labels_label")
                            .from(IssueLabels::Table, IssueLabels::LabelId)
                            .to(Labels::Table, Labels::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IssueLabels::Table).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Labels::Table)
                    .drop_column(Labels::CreatedAt)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Labels {
    Table,
    Id,
    ProjectId,
    Name,
    Color,
    CreatedAt,
}

#[derive(DeriveIden)]
enum IssueLabels {
    Table,
    IssueId,
    LabelId,
}

#[derive(DeriveIden)]
enum Projects {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Issues {
    Table,
    Id,
}
