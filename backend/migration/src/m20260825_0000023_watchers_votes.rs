use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // issue_watchers: composite PK (issue_id, user_id)
        manager
            .create_table(
                Table::create()
                    .table(IssueWatchers::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(IssueWatchers::IssueId).uuid().not_null())
                    .col(ColumnDef::new(IssueWatchers::UserId).uuid().not_null())
                    .primary_key(
                        Index::create()
                            .name("pk_issue_watchers")
                            .col(IssueWatchers::IssueId)
                            .col(IssueWatchers::UserId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_issue_watchers_issue")
                            .from(IssueWatchers::Table, IssueWatchers::IssueId)
                            .to(Issues::Table, Issues::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_issue_watchers_user")
                            .from(IssueWatchers::Table, IssueWatchers::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_issue_watchers_user")
                    .table(IssueWatchers::Table)
                    .col(IssueWatchers::UserId)
                    .to_owned(),
            )
            .await?;

        // issue_votes: composite PK (issue_id, user_id), with voted_at timestamp
        manager
            .create_table(
                Table::create()
                    .table(IssueVotes::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(IssueVotes::IssueId).uuid().not_null())
                    .col(ColumnDef::new(IssueVotes::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(IssueVotes::VotedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_issue_votes")
                            .col(IssueVotes::IssueId)
                            .col(IssueVotes::UserId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_issue_votes_issue")
                            .from(IssueVotes::Table, IssueVotes::IssueId)
                            .to(Issues::Table, Issues::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_issue_votes_user")
                            .from(IssueVotes::Table, IssueVotes::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_issue_votes_user")
                    .table(IssueVotes::Table)
                    .col(IssueVotes::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IssueVotes::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(IssueWatchers::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum IssueWatchers {
    Table,
    IssueId,
    UserId,
}

#[derive(DeriveIden)]
enum IssueVotes {
    Table,
    IssueId,
    UserId,
    VotedAt,
}

#[derive(DeriveIden)]
enum Issues {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
