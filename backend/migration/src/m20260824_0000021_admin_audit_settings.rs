use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column(
                        ColumnDef::new(Users::IsSystemAdmin)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .add_column(
                        ColumnDef::new(Users::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(AuditLogs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuditLogs::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AuditLogs::ActorId).uuid().not_null())
                    .col(ColumnDef::new(AuditLogs::Action).string().not_null())
                    .col(ColumnDef::new(AuditLogs::EntityType).string().not_null())
                    .col(ColumnDef::new(AuditLogs::EntityId).uuid())
                    .col(
                        ColumnDef::new(AuditLogs::Metadata)
                            .json()
                            .not_null()
                            .default(SimpleExpr::Custom("'{}'::jsonb".to_owned())),
                    )
                    .col(
                        ColumnDef::new(AuditLogs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_audit_logs_actor")
                            .from(AuditLogs::Table, AuditLogs::ActorId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_audit_logs_actor_created")
                    .table(AuditLogs::Table)
                    .col(AuditLogs::ActorId)
                    .col(AuditLogs::CreatedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_audit_logs_created")
                    .table(AuditLogs::Table)
                    .col(AuditLogs::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SystemSettings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SystemSettings::Key)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SystemSettings::Value)
                            .json()
                            .not_null()
                            .default(SimpleExpr::Custom("'{}'::jsonb".to_owned())),
                    )
                    .col(
                        ColumnDef::new(SystemSettings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SystemSettings::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(AuditLogs::Table).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .drop_column(Users::IsSystemAdmin)
                    .drop_column(Users::IsActive)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    IsSystemAdmin,
    IsActive,
}

#[derive(DeriveIden)]
enum AuditLogs {
    Table,
    Id,
    ActorId,
    Action,
    EntityType,
    EntityId,
    Metadata,
    CreatedAt,
}

#[derive(DeriveIden)]
enum SystemSettings {
    Table,
    Key,
    Value,
    UpdatedAt,
}
