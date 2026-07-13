# ADR-0010: apalis for Background Jobs

## Status

Accepted

## Context

Нужен scheduler/queue для email, notifications, thumbnail generation, exports, cleanup.

## Alternatives Considered

| Option | Pros | Cons |
|--------|------|------|
| tokio-cron-scheduler | Просто | Нет persistence |
| apalis | Persistence, retries, cron | Меньше ecosystem |
| custom queue | Полный контроль | Велосипед |

## Decision

`apalis` 0.7.4 с Redis storage.

## Consequences

- Persistent background jobs.
- Retries с exponential backoff.
- Cron scheduling.
- Нужно мониторить failed jobs.

## Related

- `docs/ARCHITECTURE.md`
- `docs/NOTIFICATIONS.md`
- `docs/STORAGE.md`
