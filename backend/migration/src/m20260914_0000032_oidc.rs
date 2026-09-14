use sea_orm_migration::prelude::*;

/// m32: OIDC single-provider SSO (SYSTEM_ADMIN 4.2).
///
/// - `oidc_identities`: one row per (provider, subject) linked to a local
///   user; JIT-provisioned on first callback. Unique on (provider, subject)
///   so a provider identity can only bind to one local account.
/// - `oidc_state`: short-lived authorization states (PKCE verifier + nonce),
///   single-use, TTL enforced by `expires_at` sweep in the app layer.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OidcIdentity::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(OidcIdentity::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(OidcIdentity::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(OidcIdentity::Provider)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OidcIdentity::Subject)
                            .string_len(320)
                            .not_null(),
                    )
                    .col(ColumnDef::new(OidcIdentity::Email).string_len(320))
                    .col(
                        ColumnDef::new(OidcIdentity::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OidcIdentity::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oidc_identity_user")
                            .from(OidcIdentity::Table, OidcIdentity::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_oidc_provider_subject")
                    .table(OidcIdentity::Table)
                    .col(OidcIdentity::Provider)
                    .col(OidcIdentity::Subject)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(OidcState::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(OidcState::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(OidcState::State)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OidcState::CodeVerifier)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OidcState::Nonce)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OidcState::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OidcState::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_oidc_state_state")
                    .table(OidcState::Table)
                    .col(OidcState::State)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OidcState::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(OidcIdentity::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OidcIdentity {
    Table,
    Id,
    UserId,
    Provider,
    Subject,
    Email,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum OidcState {
    Table,
    Id,
    State,
    CodeVerifier,
    Nonce,
    ExpiresAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
