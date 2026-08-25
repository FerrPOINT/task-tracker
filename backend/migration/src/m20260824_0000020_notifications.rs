use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Notifications::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Notifications::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(ColumnDef::new(Notifications::RecipientId).uuid().not_null())
                    .col(ColumnDef::new(Notifications::EventType).string().not_null())
                    .col(
                        ColumnDef::new(Notifications::EntityType)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Notifications::EntityId).uuid())
                    .col(ColumnDef::new(Notifications::ActorId).uuid())
                    .col(ColumnDef::new(Notifications::Title).string().not_null())
                    .col(ColumnDef::new(Notifications::Body).text())
                    .col(
                        ColumnDef::new(Notifications::IsRead)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(Notifications::ReadAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Notifications::ActionUrl).string())
                    .col(
                        ColumnDef::new(Notifications::Metadata)
                            .json()
                            .not_null()
                            .default(SimpleExpr::Custom("'{}'::jsonb".to_owned())),
                    )
                    .col(
                        ColumnDef::new(Notifications::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_notifications_recipient")
                            .from(Notifications::Table, Notifications::RecipientId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_notifications_actor")
                            .from(Notifications::Table, Notifications::ActorId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_notifications_recipient_unread_created")
                    .table(Notifications::Table)
                    .col(Notifications::RecipientId)
                    .col(Notifications::IsRead)
                    .col(Notifications::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(NotificationUserSettings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NotificationUserSettings::UserId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(NotificationUserSettings::EmailFrequency)
                            .string()
                            .not_null()
                            .default("immediate"),
                    )
                    .col(
                        ColumnDef::new(NotificationUserSettings::DisabledEventTypes)
                            .json()
                            .not_null()
                            .default(SimpleExpr::Custom("'[]'::jsonb".to_owned())),
                    )
                    .col(
                        ColumnDef::new(NotificationUserSettings::NotifyOwnChanges)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_notification_user_settings_user")
                            .from(
                                NotificationUserSettings::Table,
                                NotificationUserSettings::UserId,
                            )
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(NotificationUserSettings::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Notifications::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Notifications {
    Table,
    Id,
    RecipientId,
    EventType,
    EntityType,
    EntityId,
    ActorId,
    Title,
    Body,
    IsRead,
    ReadAt,
    ActionUrl,
    Metadata,
    CreatedAt,
}

#[derive(DeriveIden)]
enum NotificationUserSettings {
    Table,
    UserId,
    EmailFrequency,
    DisabledEventTypes,
    NotifyOwnChanges,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
