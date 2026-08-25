use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Project-level custom field definitions.
        manager
            .create_table(
                Table::create()
                    .table(CustomFields::Table)
                    .col(
                        ColumnDef::new(CustomFields::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(ColumnDef::new(CustomFields::ProjectId).uuid().not_null())
                    .col(ColumnDef::new(CustomFields::Name).string().not_null())
                    .col(ColumnDef::new(CustomFields::FieldType).string().not_null())
                    .col(
                        ColumnDef::new(CustomFields::Options)
                            .json_binary()
                            .not_null()
                            .default(serde_json::json!([])),
                    )
                    .col(
                        ColumnDef::new(CustomFields::IsRequired)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(CustomFields::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_custom_fields_project")
                            .from(CustomFields::Table, CustomFields::ProjectId)
                            .to(Projects::Table, Projects::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_custom_fields_project")
                    .table(CustomFields::Table)
                    .col(CustomFields::ProjectId)
                    .to_owned(),
            )
            .await?;

        // Issue-level custom field values. Composite primary key (issue_id, field_id).
        manager
            .create_table(
                Table::create()
                    .table(IssueCustomFieldValues::Table)
                    .col(
                        ColumnDef::new(IssueCustomFieldValues::IssueId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IssueCustomFieldValues::FieldId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IssueCustomFieldValues::Value)
                            .json_binary()
                            .not_null()
                            .default(serde_json::json!(null)),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_issue_custom_field_values")
                            .col(IssueCustomFieldValues::IssueId)
                            .col(IssueCustomFieldValues::FieldId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_issue_custom_field_values_issue")
                            .from(
                                IssueCustomFieldValues::Table,
                                IssueCustomFieldValues::IssueId,
                            )
                            .to(Issues::Table, Issues::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_issue_custom_field_values_field")
                            .from(
                                IssueCustomFieldValues::Table,
                                IssueCustomFieldValues::FieldId,
                            )
                            .to(CustomFields::Table, CustomFields::Id)
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
                    .table(IssueCustomFieldValues::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(CustomFields::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum CustomFields {
    Table,
    Id,
    ProjectId,
    Name,
    FieldType,
    Options,
    IsRequired,
    CreatedAt,
}

#[derive(DeriveIden)]
enum IssueCustomFieldValues {
    Table,
    IssueId,
    FieldId,
    Value,
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
