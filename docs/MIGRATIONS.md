# Database Migrations — Task Tracker

## 1. Overview

Миграции управляют схемой PostgreSQL. Используем `refinery` 0.8.15 (или SeaORM Migrator для проектов на SeaORM). Все миграции — SQL-файлы с контролем версий и контрольной суммой.

## 2. Tooling

| Tool | Purpose |
|------|---------|
| `refinery_cli` | CLI для применения/отката миграций |
| `sqlx` | Compile-time checked queries |
| `sea-orm-cli` | Генерация сущностей из схемы (опционально) |

## 3. Folder Structure

```
backend/migrations/
├── V1__initial_schema.sql
├── V2__add_issue_comments.sql
├── V3__add_workflow.sql
├── V4__add_attachments.sql
├── V5__add_notifications.sql
├── V6__add_search_index.sql
├── V7__add_reports_cache.sql
└── refinery.toml
```

## 4. Naming Convention

```
V{version}__{description}.sql
```

- Версия — целое число, строго последовательное.
- Описание — snake_case.
- Пример: `V12__add_issue_links.sql`.

## 5. Migration Rules

### 5.1 Must

- Каждая миграция идемпотентна в пределах своей версии.
- Все изменения обратимо или безопасны для отката.
- Использовать `IF NOT EXISTS` / `IF EXISTS` там, где это уместно.
- Добавлять новые колонки nullable или с default.
- Создавать индексы concurrently в production.

### 5.2 Must Not

- Нельзя удалять колонки, на которые есть активные зависимости.
- Нельзя переименовывать таблицы в одной миграции без backward-compatible alias.
- Нельзя менять тип колонки с потерей данных.
- Нельзя делать heavy ALTER на больших таблицах без отдельного runbook.

## 6. Example Migration

```sql
-- V2__add_issue_comments.sql
CREATE TABLE IF NOT EXISTS issue_comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    author_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    body TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_issue_comments_issue_id ON issue_comments(issue_id);
```

## 7. Applying Migrations

### 7.1 Local

```bash
cd backend
refinery setup -c refinery.toml
refinery migrate -c refinery.toml -p migrations
```

### 7.2 Docker

```bash
docker compose run --rm migrator
```

### 7.3 CLI

```bash
task-tracker migrate status
task-tracker migrate up
task-tracker migrate down --count 1
task-tracker migrate redo
task-tracker migrate create add_sprints_table
```

## 8. Migration Table

`refinery_schema_history` автоматически создаётся refinery:

| Column | Description |
|--------|-------------|
| `version` | номер миграции |
| `name` | имя файла |
| `applied_on` | дата применения |
| `checksum` | SHA256 содержимого |

## 9. Zero-Downtime Migrations

### 9.1 Pattern: Add → Dual Write → Migrate → Remove

1. **Deploy code** that writes to both old and new schema.
2. **Backfill** data in background job.
3. **Deploy code** that reads from new schema.
4. **Drop** old columns in later migration.

### 9.2 Example

```sql
-- V10__add_issue_display_name.sql
ALTER TABLE issues ADD COLUMN IF NOT EXISTS display_key VARCHAR(32);

-- Backfill via application job, not in migration.
-- V15__drop_issue_key.sql (later)
-- ALTER TABLE issues DROP COLUMN IF EXISTS old_key;
```

## 10. Rollbacks

### 10.1 Policy

- Откат миграций допускается только на staging/dev.
- В production откатываются изменения через **compensating migration**, а не `refinery undo`.

### 10.2 Compensating Migration

```sql
-- V11__revert_add_display_key.sql
ALTER TABLE issues DROP COLUMN IF EXISTS display_key;
```

## 11. Data Migrations

Если нужно перенести данные:

- Делать в отдельной миграции после DDL.
- Для больших объёмов — batch update с `LIMIT` и `OFFSET`.
- Запускать вне пиковой нагрузки.

## 12. Seeding

### 12.1 Fixtures

```
backend/fixtures/
├── dev/
│   ├── users.sql
│   ├── projects.sql
│   └── issues.sql
└── test/
    └── minimal.sql
```

### 12.2 Apply

```bash
psql $TASKTRACKER_DATABASE_URL -f backend/fixtures/dev/users.sql
```

## 13. Testing Migrations

- Каждая миграция тестируется в CI на fresh PostgreSQL testcontainer.
- Проверка `up` + `down` (или compensating) + `up`.
- Проверка, что приложение стартует после миграций.

## 14. CI/CD

```yaml
migrate:
  image: task-tracker-migrator:latest
  script:
    - refinery migrate -c refinery.toml -p migrations
```

## 15. Environment-Specific Notes

| Env | Approach |
|-----|----------|
| local | `refinery migrate` при старте dev-сервера |
| test | fresh DB + all migrations перед каждым прогоном |
| staging | same as production |
| production | separate migrator job, migrations before app deploy |
## References

- `docs/ARCHITECTURE.md`
- `docs/DATA_MODEL.md`
- `docs/DEPLOYMENT.md`
