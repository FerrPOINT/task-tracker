use sea_orm_migration::prelude::*;

/// Password reset tokens (docs/SYSTEM_ADMIN.md §1.2): SHA-256 hashed,
/// single-use, TTL-bound. Raw token never persisted.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PasswordResetToken::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PasswordResetToken::TokenHash)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PasswordResetToken::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(PasswordResetToken::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PasswordResetToken::UsedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(PasswordResetToken::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_password_reset_user")
                            .from(PasswordResetToken::Table, PasswordResetToken::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_password_reset_user_unique")
                    .table(PasswordResetToken::Table)
                    .col(PasswordResetToken::UserId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PasswordResetToken::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum PasswordResetToken {
    Table,
    TokenHash,
    UserId,
    ExpiresAt,
    UsedAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
