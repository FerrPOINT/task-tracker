# ADR-0007: Redis for Cache and WebSocket Pub/Sub

## Status

Accepted

## Context

Нужен shared state для multi-instance backend: кеш, сессии, real-time broadcast.

## Alternatives Considered

| Option | Pros | Cons |
|--------|------|------|
| In-memory only | Просто | Не работает в multi-instance |
| PostgreSQL LISTEN/NOTIFY | Без доп. сервиса | Меньше возможностей |
| Redis | Кеш, pub/sub, queues | Ещё один сервис |

## Decision

Redis 8.0 для кеша, WebSocket broadcast, rate-limit shared state.

## Consequences

- Простое масштабирование API instances.
- Единый источник для cache invalidation.
- Нужно мониторить Redis memory и availability.

## Related

- `docs/ARCHITECTURE.md`
- `docs/CACHING.md`
