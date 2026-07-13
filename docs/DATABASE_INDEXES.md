# Database Indexes — Task Tracker

## 1. Overview

Первичные и вспомогательные индексы PostgreSQL для обеспечения производительности и целостности. Индексы добавляются через миграции `refinery`.

## 2. Per-Table Indexes

| Table | Index | Type | Purpose |
|-------|-------|------|---------|
| `users` | `users_email_idx` | B-tree unique | Login by email |
| `users` | `users_username_idx` | B-tree unique | Public profile lookup |
| `users` | `users_active_idx` | B-tree partial `(active = true)` | Active users only |
| `projects` | `projects_key_idx` | B-tree unique | Project key uniqueness |
| `projects` | `projects_owner_id_idx` | B-tree | Owner projects list |
| `project_members` | `project_members_project_user_idx` | B-tree unique `(project_id, user_id)` | Membership lookup |
| `issues` | `issues_project_id_idx` | B-tree | Issues by project |
| `issues` | `issues_key_idx` | B-tree unique `(project_id, key)` | Key uniqueness per project |
| `issues` | `issues_status_id_idx` | B-tree | Filter by status |
| `issues` | `issues_assignee_id_idx` | B-tree | Assigned to me |
| `issues` | `issues_reporter_id_idx` | B-tree | Created by me |
| `issues` | `issues_search_idx` | GIN (tsvector) | Full-text search |
| `issues` | `issues_created_at_idx` | B-tree | Sort by date |
| `issues` | `issues_sprint_id_idx` | B-tree | Sprint issues |
| `issue_status_history` | `ish_issue_created_idx` | B-tree `(issue_id, created_at)` | Status history |
| `comments` | `comments_issue_id_idx` | B-tree | Comments by issue |
| `comments` | `comments_author_id_idx` | B-tree | Comments by author |
| `attachments` | `attachments_issue_id_idx` | B-tree | Attachments by issue |
| `worklogs` | `worklogs_issue_id_idx` | B-tree | Time logs by issue |
| `worklogs` | `worklogs_author_id_idx` | B-tree | Time logs by author |
| `sprints` | `sprints_project_id_idx` | B-tree | Sprints by project |
| `boards` | `boards_project_id_idx` | B-tree | Boards by project |
| `board_columns` | `board_columns_board_id_idx` | B-tree | Columns by board |
| `custom_field_values` | `cfv_issue_field_idx` | B-tree unique `(issue_id, field_id)` | One value per field |
| `changelog` | `changelog_issue_id_idx` | B-tree `(issue_id, created_at DESC)` | Issue history |
| `audit_log` | `audit_user_time_idx` | B-tree `(user_id, created_at DESC)` | User audit trail |
| `audit_log` | `audit_entity_idx` | B-tree `(entity_type, entity_id)` | Entity audit trail |

## 3. Composite Index Strategy

- JQL filters: создаём partial/composite индексы под частые запросы.
- Kanban board: `(project_id, status_id)` + `created_at DESC`.
- Backlog: `(project_id, sprint_id IS NULL, rank)`.
- Reports: по `sprint_id`, `status_id`, `resolved_at`.

## 4. JSONB Indexes

- `issues.custom_field_values` JSONB GIN для ad-hoc custom fields.
- `projects.settings` JSONB GIN.
- `workflow.transitions` JSONB GIN (если хранится в JSONB).

## 5. Index Maintenance

- `REINDEX CONCURRENTLY` во время low-traffic window.
- Мониторинг bloat через `pg_stat_user_indexes`.
- Автоматический `ANALYZE` после больших миграций.

## 6. Migration Examples

```sql
CREATE INDEX CONCURRENTLY idx_issues_search
  ON issues USING GIN (to_tsvector('russian', summary || ' ' || COALESCE(description, '')));

CREATE INDEX CONCURRENTLY idx_issues_project_status
  ON issues (project_id, status_id) WHERE deleted_at IS NULL;
```

## References

- `docs/DATA_MODEL.md`
- `docs/PERFORMANCE.md`
- `docs/MIGRATIONS.md`
