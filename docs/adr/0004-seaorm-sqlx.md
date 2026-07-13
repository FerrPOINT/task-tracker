# ADR-0004: SeaORM + SQLx for DB Access

## Status

Accepted

## Context

Нужен ORM и инструмент для миграций/сложных запросов в Rust.

## Alternatives Considered

| Option | Pros | Cons |
|--------|------|------|
| Diesel | Производительность, compile-time checks | Async требует доп. setup |
| SeaORM | Native async, ActiveRecord-like, SeaQuery | Меньше зрелость чем Diesel |
| SQLx | Compile-time checked queries | Без ORM слоя |

## Decision

SeaORM 2.x как основной ORM + SQLx 0.9.0 для миграций и сложных raw queries.

## Consequences

- Быстрый CRUD через SeaORM.
- SQLx гарантирует корректность сложных запросов на этапе компиляции.
- Два инструмента требуют discipline.

## Related

- `docs/DATA_MODEL.md`
- `docs/MIGRATIONS.md`
