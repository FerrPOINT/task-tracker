use sea_orm_migration::prelude::*;

/// Core referential integrity: FK constraints for the primary entity graph
/// (issues / comments / worklogs / attachments / sprints / project members).
///
/// Existing deployments may contain orphans; each constraint is added with
/// `NOT VALID` first and then validated lazily via `VALIDATE CONSTRAINT`,
/// which takes only a SHARE UPDATE EXCLUSIVE lock (writes are not blocked).
/// If validation would fail on legacy orphans, the migration fails loudly —
/// repair the data first (see docs/MIGRATIONS.md).
///
/// Delete semantics:
/// - cascade for owned children (comments, worklogs, attachments, watchers,
///   votes, labels, links, custom-field values, history, notifications by issue);
/// - `SET NULL` for optional cross-references (assignee);
/// - `RESTRICT` where the parent must be explicitly deleted first (project ->
///   issues is handled by the application cascade; here RESTRICT protects
///   against accidental project removal leaving issues behind).
#[derive(DeriveMigrationName)]
pub struct Migration;

macro_rules! add_fk {
    ($manager:expr, $table:expr, $name:expr, $col:expr, $ref_table:expr, $ref_col:expr, $action:expr) => {{
        let sql = format!(
            "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({}) {} NOT VALID",
            $table, $name, $col, $ref_table, $ref_col, $action
        );
        $manager.get_connection().execute_unprepared(&sql).await?;
        let sql = format!("ALTER TABLE {} VALIDATE CONSTRAINT {}", $table, $name);
        $manager.get_connection().execute_unprepared(&sql).await?;
    }};
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // issues -> projects (application cascade deletes issues explicitly;
        // RESTRICT is the safety net against orphans)
        add_fk!(
            manager,
            "issues",
            "fk_issues_project",
            "project_id",
            "projects",
            "id",
            "ON DELETE RESTRICT"
        );
        // issues -> statuses (statuses are global reference data)
        add_fk!(
            manager,
            "issues",
            "fk_issues_status",
            "status_id",
            "statuses",
            "id",
            "ON DELETE RESTRICT"
        );
        // optional assignee reference; removing a user keeps the issue
        add_fk!(
            manager,
            "issues",
            "fk_issues_assignee",
            "assignee_id",
            "users",
            "id",
            "ON DELETE SET NULL"
        );
        add_fk!(
            manager,
            "issues",
            "fk_issues_reporter",
            "reporter_id",
            "users",
            "id",
            "ON DELETE RESTRICT"
        );
        // sprint membership cleared when a sprint is deleted
        add_fk!(
            manager,
            "issues",
            "fk_issues_sprint",
            "sprint_id",
            "sprints",
            "id",
            "ON DELETE SET NULL"
        );

        add_fk!(
            manager,
            "comments",
            "fk_comments_issue",
            "issue_id",
            "issues",
            "id",
            "ON DELETE CASCADE"
        );
        add_fk!(
            manager,
            "comments",
            "fk_comments_author",
            "author_id",
            "users",
            "id",
            "ON DELETE RESTRICT"
        );

        add_fk!(
            manager,
            "worklogs",
            "fk_worklogs_issue",
            "issue_id",
            "issues",
            "id",
            "ON DELETE CASCADE"
        );
        add_fk!(
            manager,
            "worklogs",
            "fk_worklogs_author",
            "author_id",
            "users",
            "id",
            "ON DELETE RESTRICT"
        );

        add_fk!(
            manager,
            "attachments",
            "fk_attachments_issue",
            "issue_id",
            "issues",
            "id",
            "ON DELETE CASCADE"
        );
        add_fk!(
            manager,
            "attachments",
            "fk_attachments_author",
            "author_id",
            "users",
            "id",
            "ON DELETE RESTRICT"
        );

        add_fk!(
            manager,
            "sprints",
            "fk_sprints_project",
            "project_id",
            "projects",
            "id",
            "ON DELETE CASCADE"
        );
        add_fk!(
            manager,
            "boards",
            "fk_boards_project",
            "project_id",
            "projects",
            "id",
            "ON DELETE CASCADE"
        );

        add_fk!(
            manager,
            "project_members",
            "fk_members_project",
            "project_id",
            "projects",
            "id",
            "ON DELETE CASCADE"
        );
        add_fk!(
            manager,
            "project_members",
            "fk_members_user",
            "user_id",
            "users",
            "id",
            "ON DELETE CASCADE"
        );

        add_fk!(
            manager,
            "issue_status_history",
            "fk_history_issue",
            "issue_id",
            "issues",
            "id",
            "ON DELETE CASCADE"
        );

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (table, name) in [
            ("issue_status_history", "fk_history_issue"),
            ("project_members", "fk_members_user"),
            ("project_members", "fk_members_project"),
            ("boards", "fk_boards_project"),
            ("sprints", "fk_sprints_project"),
            ("attachments", "fk_attachments_author"),
            ("attachments", "fk_attachments_issue"),
            ("worklogs", "fk_worklogs_author"),
            ("worklogs", "fk_worklogs_issue"),
            ("comments", "fk_comments_author"),
            ("comments", "fk_comments_issue"),
            ("issues", "fk_issues_sprint"),
            ("issues", "fk_issues_reporter"),
            ("issues", "fk_issues_assignee"),
            ("issues", "fk_issues_status"),
            ("issues", "fk_issues_project"),
        ] {
            let sql = format!("ALTER TABLE {} DROP CONSTRAINT IF EXISTS {}", table, name);
            manager.get_connection().execute_unprepared(&sql).await?;
        }
        Ok(())
    }
}
