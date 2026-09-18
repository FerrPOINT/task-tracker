# Runtime Behavior — Task Tracker

## 1. Overview

Как приложение стартует, работает и корректно завершается в production.

## 2. Health Probes

### 2.1 Endpoints

| Probe | Path | Success | Failure |
|-------|------|---------|---------|
| Liveness | `GET /api/v1/health` | HTTP 200; текущий public liveness API | Process unavailable |
| Catalog compatibility | `GET /health` | HTTP 200; alias for the fixed Base `health.read` path | Process unavailable |
| Readiness / startup | not exposed separately | Liveness does not prove DB, Redis, email or central-auth readiness | Use owned dependency and deployment checks |

### 2.2 Historical target probes

The former `/health/startup` and `/health/ready` descriptions are target-only
material and do not describe the current Task Tracker runtime. Operators use
the documented public liveness endpoint plus owned dependency and deployment
checks; they must not infer readiness from a successful liveness response.

### 2.3 Liveness behavior

- Both `GET /api/v1/health` and the Base catalog alias `GET /health` return the
  same public process-liveness response.
- Docker and monitoring may use either current route according to their
  deployment contract.
- A successful response does not assert PostgreSQL, Redis, SMTP or central-auth
  readiness.

## 3. Startup Order

1. Загрузка конфигурации (`TASKTRACKER_*`).
2. Подключение к PostgreSQL с retry:
   - Initial delay: 1s.
   - Max delay: 30s.
   - Max retries: 30.
3. Применение миграций (`sea-orm-migration`).
4. Seed default data (admin, default issue types, workflow).
5. Подключение к Redis с retry.
6. Запуск HTTP-сервера (API + SSE `/api/v1/events`).
7. Mark startup probe as ready.

## 4. Retry / Backoff

| Dependency | Strategy |
|------------|----------|
| PostgreSQL | exponential backoff 1s → 30s |
| Redis | exponential backoff 1s → 10s |
| SMTP | 3 attempts with 5s delay |
| External webhooks | 3 attempts with exponential backoff |

## 5. Graceful Shutdown

1. Получение `SIGTERM` / `SIGINT`.
2. Stop accepting new HTTP connections.
3. Wait for active requests (timeout 30s).
4. Закрыть SSE-соединения.
5. Stop background workers (apalis).
6. Flush pending events to Redis/bus.
7. Close DB connection pool.
8. Exit.

## 6. Resource Limits

| Resource | Limit | Why |
|----------|-------|-----|
| `nofile` | 65536 | WS + uploads |
| `max_connections` PostgreSQL | 200 | connection pool |
| Backend connection pool | 20-50 | per instance |
| Redis pool | 20 | per instance |
| Request body | 10 MB | JSON payloads |
| Upload file | 50 MB | attachments |

## 7. Background Workers

- `apalis` для email, webhooks, audit export.
- Worker count: 4 per instance.
- Retry policy: 3 attempts, then dead-letter queue.

## 8. Watchdogs

- Если readiness падает более 2 минут — алерт.
- Если queue size растёт более 1000 — алерт.
- Если liveness падает — автоматический restart.

## 9. Multi-instance Notes

- Stateless HTTP tier.
- События SSE публикуются in-process (broadcast); Redis pub/sub не используется.
- Background jobs должны быть idempotent при scale-out.

## References

- `docs/DEPLOYMENT.md`
- `docs/OPS_RUNBOOK.md`
- `docs/MONITORING.md`
- `docs/EVENTS.md`
