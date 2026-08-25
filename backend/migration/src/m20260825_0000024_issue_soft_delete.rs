use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add nullable `deleted_at` column for soft-delete / trash support.
        // NULL  → live issue; non-NULL → trashed (hidden from normal queries).
        manager
            .alter_table(
                Table::alter()
                    .table(Issues::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Issues::DeletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Partial index: only trashed rows, so trash listings stay fast without
        // bloating the indexes used by live-issue queries.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_issues_deleted_at")
                    .table(Issues::Table)
                    .col(Issues::DeletedAt)
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
                    .name("idx_issues_deleted_at")
                    .table(Issues::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Issues::Table)
                    .drop_column(Issues::DeletedAt)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Issues {
    Table,
    DeletedAt,
}
