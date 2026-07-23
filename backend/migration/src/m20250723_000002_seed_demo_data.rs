use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        let demo_user_id = "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11";
        let password_hash = "$argon2id$v=19$m=65536,t=3,p=4$stN/enhZ9yOvgWC9E8Y6BA$IL9I0WONb/I6zoT4rdmdkrPcIFADFxsLCjrO0ySSl0Y";
        let tt_project_id = "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a12";
        let tt_board_id = "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a14";
        let demo_project_id = "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a13";
        let demo_board_id = "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a15";
        let sprint_id = "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a25";

        let status_todo = "00000000-0000-0000-0000-000000000001";
        let status_in_progress = "00000000-0000-0000-0000-000000000002";
        let status_done = "00000000-0000-0000-0000-000000000003";
        let status_review = "00000000-0000-0000-0000-000000000004";

        let tt_columns = serde_json::json!([
            {"id": status_todo, "name": "Todo", "category": "todo", "position": 0, "wip_limit": null},
            {"id": status_in_progress, "name": "In Progress", "category": "inprogress", "position": 1, "wip_limit": 5},
            {"id": status_review, "name": "Review", "category": "inprogress", "position": 2, "wip_limit": null},
            {"id": status_done, "name": "Done", "category": "done", "position": 3, "wip_limit": null}
        ]);
        let demo_columns = serde_json::json!([
            {"id": status_todo, "name": "Todo", "category": "todo", "position": 0, "wip_limit": null},
            {"id": status_in_progress, "name": "In Progress", "category": "inprogress", "position": 1, "wip_limit": 5},
            {"id": status_done, "name": "Done", "category": "done", "position": 2, "wip_limit": null}
        ]);

        let sqls = vec![
            format!(
                "INSERT INTO users (id, email, username, display_name, password_hash, created_at, updated_at) VALUES ('{}', 'demo@example.com', 'demo', 'Demo User', '{}', NOW(), NOW()) ON CONFLICT (id) DO NOTHING",
                demo_user_id, password_hash
            ),
            format!(
                "INSERT INTO projects (id, key, name, description, owner_id, default_board_id, created_at, updated_at) VALUES ('{}', 'TT', 'Task Tracker', 'Internal task tracker project', '{}', '{}', NOW(), NOW()) ON CONFLICT (id) DO NOTHING",
                tt_project_id, demo_user_id, tt_board_id
            ),
            format!(
                "INSERT INTO boards (id, project_id, name, columns) VALUES ('{}', '{}', 'TT Board', '{}') ON CONFLICT (id) DO NOTHING",
                tt_board_id,
                tt_project_id,
                tt_columns.to_string()
            ),
            format!(
                "INSERT INTO projects (id, key, name, description, owner_id, default_board_id, created_at, updated_at) VALUES ('{}', 'DEMO', 'Demo Project', 'Playground project for onboarding', '{}', '{}', NOW(), NOW()) ON CONFLICT (id) DO NOTHING",
                demo_project_id, demo_user_id, demo_board_id
            ),
            format!(
                "INSERT INTO boards (id, project_id, name, columns) VALUES ('{}', '{}', 'Demo Board', '{}') ON CONFLICT (id) DO NOTHING",
                demo_board_id,
                demo_project_id,
                demo_columns.to_string()
            ),
            format!(
                "INSERT INTO project_members (project_id, user_id, role) VALUES ('{}', '{}', 'owner') ON CONFLICT DO NOTHING",
                tt_project_id, demo_user_id
            ),
            format!(
                "INSERT INTO project_members (project_id, user_id, role) VALUES ('{}', '{}', 'owner') ON CONFLICT DO NOTHING",
                demo_project_id, demo_user_id
            ),
            format!(
                "INSERT INTO sprints (id, project_id, name, goal, state, start_date, end_date, velocity) VALUES ('{}', '{}', 'TT Sprint 1', 'Initial MVP sprint', 'active', NOW() - INTERVAL '3 days', NOW() + INTERVAL '11 days', 24) ON CONFLICT (id) DO NOTHING",
                sprint_id, tt_project_id
            ),
        ];

        for sql in sqls {
            conn.execute_unprepared(&sql).await?;
        }

        let issues = vec![
            (
                "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a30",
                "TT-1",
                status_todo,
                "task",
                "Set up React router",
                "Configure React Router and basic page structure",
                1000.0,
                Some(sprint_id),
            ),
            (
                "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a31",
                "TT-2",
                status_in_progress,
                "task",
                "Implement auth forms",
                "Login and register forms with validation",
                2000.0,
                Some(sprint_id),
            ),
            (
                "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a32",
                "TT-3",
                status_in_progress,
                "bug",
                "Fix mobile sidebar",
                "Sidebar does not collapse on 375px",
                3000.0,
                Some(sprint_id),
            ),
            (
                "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a33",
                "TT-4",
                status_review,
                "story",
                "Search page",
                "Global issue search with filters",
                4000.0,
                Some(sprint_id),
            ),
            (
                "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a34",
                "TT-5",
                status_done,
                "task",
                "Deploy to Docker",
                "Docker compose and health checks",
                5000.0,
                Some(sprint_id),
            ),
            (
                "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a35",
                "TT-6",
                status_todo,
                "task",
                "Write API docs",
                "OpenAPI and README updates",
                6000.0,
                None,
            ),
            (
                "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a36",
                "TT-7",
                status_todo,
                "story",
                "Add time tracking",
                "Log work dialog and estimates",
                7000.0,
                None,
            ),
            (
                "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a37",
                "TT-8",
                status_todo,
                "bug",
                "Dark theme flicker",
                "Theme toggles incorrectly on reload",
                8000.0,
                None,
            ),
        ];

        for (id, key, status_id, issue_type, summary, description, position, sprint_id) in issues {
            let sprint = sprint_id
                .map(|s| format!("'{}'", s))
                .unwrap_or_else(|| "NULL".to_string());
            conn.execute_unprepared(&format!(
                "INSERT INTO issues (id, project_id, key, issue_type, status_id, summary, description, reporter_id, assignee_id, priority, labels, sprint_id, position, time_spent_seconds, created_at, updated_at) VALUES ('{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', 'Medium', '[]', {}, {}, 0, NOW(), NOW()) ON CONFLICT (id) DO NOTHING",
                id, tt_project_id, key, issue_type, status_id, summary, description, demo_user_id, demo_user_id, sprint, position
            )).await?;
        }

        let demo_issues = vec![
            (
                "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a40",
                "DEMO-1",
                status_todo,
                "task",
                "Explore board",
                "Try dragging cards between columns",
                1000.0,
            ),
            (
                "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a41",
                "DEMO-2",
                status_in_progress,
                "story",
                "Create first issue",
                "Use the create issue form",
                2000.0,
            ),
            (
                "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a42",
                "DEMO-3",
                status_done,
                "bug",
                "Mark as done",
                "Close a task and check backlog",
                3000.0,
            ),
        ];

        for (id, key, status_id, issue_type, summary, description, position) in demo_issues {
            conn.execute_unprepared(&format!(
                "INSERT INTO issues (id, project_id, key, issue_type, status_id, summary, description, reporter_id, assignee_id, priority, labels, sprint_id, position, time_spent_seconds, created_at, updated_at) VALUES ('{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', 'Low', '[]', NULL, {}, 0, NOW(), NOW()) ON CONFLICT (id) DO NOTHING",
                id, demo_project_id, key, issue_type, status_id, summary, description, demo_user_id, demo_user_id, position
            )).await?;
        }

        let labels = vec![
            ("a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a50", "backend", "#3b82f6"),
            (
                "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a51",
                "frontend",
                "#22c55e",
            ),
            ("a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a52", "ui/ux", "#a855f7"),
        ];

        for (id, name, color) in labels {
            conn.execute_unprepared(&format!(
                "INSERT INTO labels (id, project_id, name, color) VALUES ('{}', '{}', '{}', '{}') ON CONFLICT (id) DO NOTHING",
                id, tt_project_id, name, color
            )).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared("DELETE FROM issues WHERE key LIKE 'TT-%' OR key LIKE 'DEMO-%'")
            .await?;
        conn.execute_unprepared("DELETE FROM sprints WHERE name = 'TT Sprint 1'")
            .await?;
        conn.execute_unprepared("DELETE FROM project_members WHERE role = 'owner'")
            .await?;
        conn.execute_unprepared("DELETE FROM boards WHERE name IN ('TT Board','Demo Board')")
            .await?;
        conn.execute_unprepared("DELETE FROM projects WHERE key IN ('TT','DEMO')")
            .await?;
        conn.execute_unprepared("DELETE FROM users WHERE username = 'demo'")
            .await?;
        Ok(())
    }
}
