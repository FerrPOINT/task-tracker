use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(IssueLinks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(IssueLinks::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(ColumnDef::new(IssueLinks::SourceId).uuid().not_null())
                    .col(ColumnDef::new(IssueLinks::TargetId).uuid().not_null())
                    .col(
                        ColumnDef::new(IssueLinks::LinkType)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IssueLinks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_issue_links_source")
                            .from(IssueLinks::Table, IssueLinks::SourceId)
                            .to(Issues::Table, Issues::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_issue_links_target")
                            .from(IssueLinks::Table, IssueLinks::TargetId)
                            .to(Issues::Table, Issues::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_issue_links_pair_type")
                    .table(IssueLinks::Table)
                    .col(IssueLinks::SourceId)
                    .col(IssueLinks::TargetId)
                    .col(IssueLinks::LinkType)
                    .unique()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IssueLinks::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum IssueLinks {
    Table,
    Id,
    SourceId,
    TargetId,
    LinkType,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Issues {
    Table,
    Id,
}
