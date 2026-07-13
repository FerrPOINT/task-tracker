# Runtime Behavior — Task Tracker

## 1. Overview

Как приложение стартует, работает и корректно завершается в production.

## 2. Health Probes

### 2.1 Endpoints

| Probe | Path | Success | Failure |
|-------|------|---------|---------|
| Liveness | `GET /health/live` | HTTP 200 | HTTP 503 |
| Readiness | `GET /health/ready` | DB, Redis OK | HTTP 503 |
| Startup | `GET /health/startup` | migrations done | HTTP 503 |

### 2.2 Startup Probe

- Выполняется только во время старта.
- Проверяет, что миграции применены и seed-данные на месте.
- Период: 10s, failureThreshold: 30 (≈5 минут).
- После success не повторяется.

### 2.3 Readiness Probe

- Проверяет соединение с PostgreSQL и Redis.
- Если БД недоступна — readiness 503, трафик не направляется.
- Период: 5s.

### 2.4 Liveness Probe

- Простой ping.
- Если не отвечает 3 раза подряд — контейнер перезапускается.

## 3. Startup Order

1. Загрузка конфигурации (`TASKTRACKER_*`).
2. Подключение к PostgreSQL с retry:
   - Initial delay: 1s.
   - Max delay: 30s.
   - Max retries: 30.
3. Применение миграций (`refinery`).
4. Seed default data (admin, default issue types, workflow).
5. Подключение к Redis с retry.
6. Запуск HTTP/WebSocket сервера.
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
2. Stop accepting new HTTP/WebSocket connections.
3. Wait for active requests (timeout 30s).
4. Close WebSocket connections with `1001 Going Away`.
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
- WebSocket state синхронизируется через Redis pub/sub.
- Background jobs должны быть idempotent при scale-out.

## References

- `docs/DEPLOYMENT.md`
- `docs/OPS_RUNBOOK.md`
- `docs/MONITORING.md`
- `docs/WEBSOCKET_EVENTS.md`
