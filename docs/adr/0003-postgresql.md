# ADR-0003: PostgreSQL as Primary Database

## Status

Accepted

## Context

Нужна надёжная реляционная БД для сложной дата-модели: проекты, задачи, workflow, permissions, history.

## Alternatives Considered

| Option | Pros | Cons |
|--------|------|------|
| SQLite | Простота | Не подходит для concurrency, нет advanced features |
| MySQL | Популярность | JSONB менее зрелый |
| PostgreSQL | JSONB, full-text search, maturity, extensions | Требует администрирования |

## Decision

PostgreSQL 17.6.

## Consequences

- JSONB для гибких схем (workflow, field config).
- Full-text search через `tsvector` / extension.
- Хорошая поддержка миграций и backup.
- Нужно настроить pg_dump backup и monitoring.

## Related

- `docs/DATA_MODEL.md`
- `docs/MIGRATIONS.md`
