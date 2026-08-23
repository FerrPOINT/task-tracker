use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        manager
            .create_table(
                Table::create()
                    .table(IssueTypes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(IssueTypes::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(
                        ColumnDef::new(IssueTypes::Name)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(IssueTypes::Description).text())
                    .col(ColumnDef::new(IssueTypes::Icon).string())
                    .col(ColumnDef::new(IssueTypes::Color).string_len(7))
                    .col(
                        ColumnDef::new(IssueTypes::IsSubtask)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(IssueTypes::HierarchyLevel)
                            .small_integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(IssueTypes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Statuses::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Statuses::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(ColumnDef::new(Statuses::Name).string().not_null())
                    .col(ColumnDef::new(Statuses::Category).string_len(16).not_null())
                    .col(
                        ColumnDef::new(Statuses::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Statuses::IsDefault)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Statuses::IsClosed)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Statuses::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(WorkflowTransitions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WorkflowTransitions::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(ColumnDef::new(WorkflowTransitions::Name).string())
                    .col(
                        ColumnDef::new(WorkflowTransitions::FromStatusId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowTransitions::ToStatusId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowTransitions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(IssueStatusHistory::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(IssueStatusHistory::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .extra("DEFAULT gen_random_uuid()".to_owned()),
                    )
                    .col(
                        ColumnDef::new(IssueStatusHistory::IssueId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(IssueStatusHistory::FromStatusId).uuid())
                    .col(
                        ColumnDef::new(IssueStatusHistory::ToStatusId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IssueStatusHistory::ChangedById)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IssueStatusHistory::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(SimpleExpr::Keyword(Keyword::CurrentTimestamp)),
                    )
                    .to_owned(),
            )
            .await?;

        create_idx(
            manager,
            WorkflowTransitions::Table,
            "idx_transitions_from_status",
            WorkflowTransitions::FromStatusId,
        )
        .await?;
        create_idx(
            manager,
            WorkflowTransitions::Table,
            "idx_transitions_to_status",
            WorkflowTransitions::ToStatusId,
        )
        .await?;
        create_idx(
            manager,
            IssueStatusHistory::Table,
            "idx_status_history_issue_id",
            IssueStatusHistory::IssueId,
        )
        .await?;

        // Seed system issue types
        let issue_types_sql = r#"
            INSERT INTO issue_types (id, name, description, icon, color, is_subtask, hierarchy_level, created_at) VALUES
            ('00000000-0000-0000-0000-000000000010', 'Task', 'Regular task', 'square', '#6366F1', false, 1, NOW()),
            ('00000000-0000-0000-0000-000000000011', 'Bug', 'Defect', 'bug', '#EF4444', false, 1, NOW()),
            ('00000000-0000-0000-0000-000000000012', 'Story', 'User story', 'circle', '#22C55E', false, 1, NOW()),
            ('00000000-0000-0000-0000-000000000013', 'Epic', 'Large feature', 'zap', '#F59E0B', false, 0, NOW()),
            ('00000000-0000-0000-0000-000000000014', 'Sub-task', 'Sub-task', 'arrow-down-right', '#8B5CF6', true, 2, NOW())
            ON CONFLICT (id) DO NOTHING
        "#;
        conn.execute_unprepared(issue_types_sql).await?;

        // Seed global statuses (matches hardcoded UUIDs in helpers.rs / demo data)
        let statuses_sql = r#"
            INSERT INTO statuses (id, name, category, position, is_default, is_closed, created_at) VALUES
            ('00000000-0000-0000-0000-000000000001', 'To Do', 'todo', 0, true, false, NOW()),
            ('00000000-0000-0000-0000-000000000002', 'In Progress', 'inprogress', 1, false, false, NOW()),
            ('00000000-0000-0000-0000-000000000004', 'Review', 'inprogress', 2, false, false, NOW()),
            ('00000000-0000-0000-0000-000000000003', 'Done', 'done', 3, false, true, NOW())
            ON CONFLICT (id) DO NOTHING
        "#;
        conn.execute_unprepared(statuses_sql).await?;

        // Seed global workflow transitions
        let transitions_sql = r#"
            INSERT INTO workflow_transitions (id, name, from_status_id, to_status_id, created_at) VALUES
            (gen_random_uuid(), 'Start progress', '00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000002', NOW()),
            (gen_random_uuid(), 'Request review', '00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000004', NOW()),
            (gen_random_uuid(), 'Complete', '00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000003', NOW()),
            (gen_random_uuid(), 'Complete review', '00000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000003', NOW()),
            (gen_random_uuid(), 'Reopen from done', '00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000002', NOW()),
            (gen_random_uuid(), 'Back to review', '00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000004', NOW())
            ON CONFLICT (id) DO NOTHING
        "#;
        conn.execute_unprepared(transitions_sql).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(IssueStatusHistory::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(WorkflowTransitions::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Statuses::Table).if_exists().to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(IssueTypes::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum IssueTypes {
    Table,
    Id,
    Name,
    Description,
    Icon,
    Color,
    IsSubtask,
    HierarchyLevel,
    CreatedAt,
}

#[derive(Iden)]
enum Statuses {
    Table,
    Id,
    Name,
    Category,
    Position,
    IsDefault,
    IsClosed,
    CreatedAt,
}

#[derive(Iden)]
enum WorkflowTransitions {
    Table,
    Id,
    Name,
    FromStatusId,
    ToStatusId,
    CreatedAt,
}

#[derive(Iden)]
enum IssueStatusHistory {
    Table,
    Id,
    IssueId,
    FromStatusId,
    ToStatusId,
    ChangedById,
    CreatedAt,
}

async fn create_idx<'a>(
    manager: &SchemaManager<'a>,
    table: impl IntoIden + 'a + 'static,
    name: &'a str,
    column: impl IntoIden + 'a + 'static,
) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name(name)
                .table(table)
                .col(column)
                .to_owned(),
        )
        .await
}
