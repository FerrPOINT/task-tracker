use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // project_components: logical parts of a project (Frontend, Backend, …).
        manager
            .create_table(
                Table::create()
                    .table(ProjectComponents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProjectComponents::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(
                        ColumnDef::new(ProjectComponents::ProjectId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ProjectComponents::Name).string().not_null())
                    .col(ColumnDef::new(ProjectComponents::Description).text())
                    .col(
                        ColumnDef::new(ProjectComponents::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_project_components_project")
                            .from(ProjectComponents::Table, ProjectComponents::ProjectId)
                            .to(Projects::Table, Projects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_project_components_project")
                    .table(ProjectComponents::Table)
                    .col(ProjectComponents::ProjectId)
                    .to_owned(),
            )
            .await?;

        // project_versions: releases / milestones.
        manager
            .create_table(
                Table::create()
                    .table(ProjectVersions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProjectVersions::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(ColumnDef::new(ProjectVersions::ProjectId).uuid().not_null())
                    .col(ColumnDef::new(ProjectVersions::Name).string().not_null())
                    .col(ColumnDef::new(ProjectVersions::Description).text())
                    .col(
                        ColumnDef::new(ProjectVersions::Released)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(ProjectVersions::ReleaseDate).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(ProjectVersions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_project_versions_project")
                            .from(ProjectVersions::Table, ProjectVersions::ProjectId)
                            .to(Projects::Table, Projects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_project_versions_project")
                    .table(ProjectVersions::Table)
                    .col(ProjectVersions::ProjectId)
                    .to_owned(),
            )
            .await?;

        // Nullable FK columns on issues.
        manager
            .alter_table(
                Table::alter()
                    .table(Issues::Table)
                    .add_column(ColumnDef::new(Issues::ComponentId).uuid())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Issues::Table)
                    .add_column(ColumnDef::new(Issues::AffectedVersionId).uuid())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Issues::Table)
                    .add_column(ColumnDef::new(Issues::FixVersionId).uuid())
                    .to_owned(),
            )
            .await?;

        // Foreign keys from issues to project_components / project_versions.
        manager
            .alter_table(
                Table::alter()
                    .table(Issues::Table)
                    .add_foreign_key(
                        &TableForeignKey::new()
                            .name("fk_issues_component")
                            .from_tbl(Issues::Table)
                            .from_col(Issues::ComponentId)
                            .to_tbl(ProjectComponents::Table)
                            .to_col(ProjectComponents::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Issues::Table)
                    .add_foreign_key(
                        &TableForeignKey::new()
                            .name("fk_issues_affected_version")
                            .from_tbl(Issues::Table)
                            .from_col(Issues::AffectedVersionId)
                            .to_tbl(ProjectVersions::Table)
                            .to_col(ProjectVersions::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Issues::Table)
                    .add_foreign_key(
                        &TableForeignKey::new()
                            .name("fk_issues_fix_version")
                            .from_tbl(Issues::Table)
                            .from_col(Issues::FixVersionId)
                            .to_tbl(ProjectVersions::Table)
                            .to_col(ProjectVersions::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop FKs first.
        manager
            .alter_table(
                Table::alter()
                    .table(Issues::Table)
                    .drop_foreign_key(Alias::new("fk_issues_fix_version"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Issues::Table)
                    .drop_foreign_key(Alias::new("fk_issues_affected_version"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Issues::Table)
                    .drop_foreign_key(Alias::new("fk_issues_component"))
                    .to_owned(),
            )
            .await?;

        for col in [
            Issues::FixVersionId,
            Issues::AffectedVersionId,
            Issues::ComponentId,
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Issues::Table)
                        .drop_column(col)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .drop_table(Table::drop().table(ProjectVersions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ProjectComponents::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ProjectComponents {
    Table,
    Id,
    ProjectId,
    Name,
    Description,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ProjectVersions {
    Table,
    Id,
    ProjectId,
    Name,
    Description,
    Released,
    ReleaseDate,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Issues {
    Table,
    ComponentId,
    AffectedVersionId,
    FixVersionId,
}

#[derive(DeriveIden)]
enum Projects {
    Table,
    Id,
}
