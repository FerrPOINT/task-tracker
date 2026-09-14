use sea_orm_migration::prelude::*;

/// TOTP MFA (docs/SECURITY.md, docs/SYSTEM_ADMIN.md): secret stored
/// encrypted-at-rest by the application (AES-256-GCM with TASKTRACKER_ env
/// key), plus one-time recovery codes (hashed).
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Totp::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Totp::UserId).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Totp::SecretCipher).text().not_null())
                    .col(
                        ColumnDef::new(Totp::Enabled)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Totp::ConfirmedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Totp::LastUsedStep)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Totp::RecoveryCodes)
                            .text()
                            .not_null()
                            .default("[]"),
                    )
                    .col(
                        ColumnDef::new(Totp::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Totp::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_totp_user")
                            .from(Totp::Table, Totp::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Totp::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Totp {
    Table,
    UserId,
    SecretCipher,
    Enabled,
    ConfirmedAt,
    LastUsedStep,
    RecoveryCodes,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
