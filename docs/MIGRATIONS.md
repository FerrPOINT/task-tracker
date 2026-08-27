# Database Migrations — Task Tracker

## 1. Overview

Миграции управляют схемой PostgreSQL. Используется **SeaORM Migrator** (`sea-orm-migration` 1.1). Миграции — Rust-файлы с типизированным API, регистрируются в `migration/src/lib.rs`.

## 2. Tooling

| Tool | Purpose |
|------|---------|
| `sea-orm-migration` | Применение миграций при старте сервера |
| `cargo build --bin gen-openapi` | Генерация OpenAPI spec |
| `sea-orm-cli generate entity` | Генерация сущностей из схемы (опционально) |

## 3. Folder Structure

```
backend/migration/src/
├── lib.rs                        # Migrator registration
├── m20250723_000001_create_tables.rs
├── m20250723_0000015_workflow_and_issue_types.rs
├── m20250723_0000016_labels.rs
├── m20250723_0000017_issue_links.rs
├── m20250723_0000018_fulltext_search.rs
├── m20260824_0000020_notifications.rs
├── m20260824_0000021_admin_audit_settings.rs
├── m20260824_0000022_performance_indexes.rs
├── m20260825_0000023_watchers_votes.rs
├── m20260825_0000024_issue_soft_delete.rs
├── m20260825_0000025_components_versions.rs
├── m20260825_0000026_custom_fields.rs
├── m20260826_0000027_fk_indexes.rs
```

## 4. Naming Convention

```
m{YYYYMMDD}_{NNNNNN}_{description}.rs
```

- Дата — дата создания миграции.
- NNNNNN — порядковый номер (6 цифр), строго последовательный.
- Description — snake_case.
- Пример: `m20260826_0000027_fk_indexes.rs`.

## 5. Migration Rules

### 5.1 Must

- Каждая миграция регистрируется в `migration/src/lib.rs` (`Vec<Box<dyn MigrationTrait>>`).
- Все изменения обратимы или безопасны для отката (`down` метод).
- Добавлять новые колонки nullable или с default.
- Создавать индексы concurrently в production.

### 5.2 Must Not

- Не изменять существующие миграции после коммита — только новая миграция.
- Не удалять миграции из `lib.rs` — только помечать как deprecated.
- Не использовать raw SQL без необходимости — предпочитать SeaORM API.

## 6. Applying Migrations

Миграции применяются автоматически при старте сервера:

```rust
// backend/infra/src/db.rs
migration::Migrator::up(&db_conn, None).await?;
```

Вручную (через migration CLI):

```bash
# Применить все миграции
DATABASE_URL=postgres://... cargo run -p migration -- up

# Откатить последнюю
DATABASE_URL=postgres://... cargo run -p migration -- down

# Пересоздать БД + применить все миграции
DATABASE_URL=postgres://... cargo run -p migration -- fresh

# Проверить статус
DATABASE_URL=postgres://... cargo run -p migration -- status
```

CI проверяет применение всех миграций на чистой PostgreSQL (job `migrations`).

## 7. History Table

`seaql_migrations` — автоматически создаётся SeaORM Migrator. Хранит версию и контрольную сумму каждой применённой миграции.

## 8. Creating a New Migration

```bash
# Создать файл
touch backend/migration/src/m20260101_0000028_description.rs
```

Шаблон:

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // DDL operations
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Rollback
    }
}
```

Регистрация в `lib.rs`:

```rust
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            // ...
            Box::new(m20260101_0000028_description::Migration),
        ]
    }
}
```

## 8a. m20260827_0000028 — core FK constraints

Добавляет 16 FK-констрейнтов основного графа (issues/comments/worklogs/attachments/sprints/boards/project_members/issue_status_history) с явными delete-семантиками:

- `CASCADE` — дочерние сущности (comments, worklogs, attachments, history, sprint/board/members при удалении проекта);
- `SET NULL` — опциональные ссылки (issues.assignee_id, issues.sprint_id);
- `RESTRICT` — целостность родителя (issues.project_id/status_id/reporter_id, authors).

Констрейнты добавляются `NOT VALID` + `VALIDATE CONSTRAINT` (не блокирует запись). Если в данных есть сироты, миграция падает с именем констрейнта — данные нужно починить:

```sql
-- найти сирот reporter
SELECT i.id, i.key FROM issues i LEFT JOIN users u ON i.reporter_id=u.id WHERE u.id IS NULL;
-- переназначить на реального пользователя
UPDATE issues SET reporter_id='<uuid>' WHERE ...;
```

DB-backed регрессия: `backend/infra/tests/fk_regression.rs` (docker-стек, `--include-ignored`).

## 9. Production

- Миграции применяются при старте сервера автоматически.
- В production откатываются через **compensating migration**, а не `down`.
- Перед деплоем: тест на пустой БД (`cargo run -p migration -- fresh`).
- Резервная копия перед миграцией обязательна.

## 10. Environments

| Environment | When |
|-------------|------|
| local | `Migrator::up` при старте dev-сервера |
| CI | `Migrator::up` на testcontainers PostgreSQL |
| production | `Migrator::up` при старте backend контейнера |

## References

- [ARCHITECTURE](ARCHITECTURE.md)
- [LOCAL_SETUP](LOCAL_SETUP.md)
