# Resilience & Fault Tolerance — Task Tracker

## 1. Overview

Как система ведёт себя при сбоях зависимостей и сети.

## 2. Idempotency

### 2.1 Idempotency Keys

- Каждый mutation request может содержать заголовок `Idempotency-Key: <uuid>`.
- Сервер сохраняет mapping `key → response` на 24 часа.
- При повторном запросе с тем же ключом возвращается сохранённый ответ.
- Применяется к:
  - `POST /api/v1/issues`
  - `POST /api/v1/projects`
  - `POST /api/v1/comments`
  - bulk operations

### 2.2 Natural Idempotency

- `PUT` обновления с ETag/If-Match.
- `DELETE` повторный — 404 без side effects.

## 3. Circuit Breakers

| Dependency | Failure Threshold | Recovery |
|------------|-------------------|----------|
| PostgreSQL | 5 errors in 30s | retry every 5s |
| Redis | 10 errors in 30s | retry every 5s |
| SMTP | 3 errors | defer to queue |
| External webhooks | 5 errors in 60s | half-open after 30s |

## 4. Graceful Degradation

| Component Fails | Fallback Behavior |
|-------------------|-------------------|
| Redis | Cache miss → DB query; sessions stateless via JWT |
| Email SMTP | Queue to DB-backed retry; no immediate error |
| WebSocket | Polling fallback for notifications |
| Search service | Degrade to `LIKE`/`tsvector` query |
| File storage S3 | Switch to local filesystem |
| Audit queue | Synchronous audit insert |

## 5. Bulk Operations

- `POST /api/v1/issues/bulk` — создание до 100 задач.
- `PATCH /api/v1/issues/bulk` — массовое обновление статуса/assignee.
- Ответ содержит `processed`, `failed`, `errors`.
- Bulk requests обязательно с `Idempotency-Key`.

## 6. Optimistic Locking

- Issue/project/config имеют `version` поле.
- `PUT` с `If-Match: <version>`.
- При conflict — `409` с актуальной версией.

## 7. Request Limits

| Limit | Value |
|-------|-------|
| Max request body | 10 MB |
| Max attachments total | 50 MB |
| Max bulk items | 100 |
| Max JQL result set | 1000 (paginated) |
| Max page size | 100 |
| Default page size | 20 |

## 8. Retry for Background Jobs

- apalis job retry: 3 attempts.
- Delay: 5s, 25s, 125s.
- After 3 failures — move to dead-letter table.
- Dead-letter admin UI: retry/ignore/delete.

## 9. Cross-Project Isolation

- Каждый repository query фильтрует по `project_id`.
- Service layer二次检查 permission.
- `project_id` взятый из URL проверяется на доступность.
- Тест: попытка доступа к issue другого project возвращает 404 (не 403, чтобы не leak ID).

## 10. Soft Delete

- `projects`, `issues`, `comments`, `attachments` имеют `deleted_at`.
- DELETE endpoint помечает `deleted_at`.
- Trash UI позволяет восстановить в течение 30 дней.
- Hard delete после retention policy.

## References

- `docs/ARCHITECTURE.md`
- `docs/API.md`
- `docs/STORAGE.md`
- `docs/NOTIFICATIONS.md`
- `docs/TESTING.md`
