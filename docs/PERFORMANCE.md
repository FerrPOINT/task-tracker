# Производительность и оптимизация Task Tracker

## 1. Цели

- P95 ответа API < 200 мс при 100 RPS на типичных запросах.
- Kanban-доска до 1000 задач загружается < 1 сек.
- Полнотекстовый поиск по 100k задач < 300 мс.
- 99.9% uptime на single-instance.

## 2. База данных

### Индексы
- `issues(project_id, status_id)` — kanban board
- `issues(assignee_id, status_id)` — my issues
- `issues(search_vector)` — GIN full-text
- `issues(updated_at DESC)` — activity feeds
- `comments(issue_id, created_at)`
- `attachments(issue_id)`

### Оптимизации
- Connection pool: SQLx pool size = `(cpu_count * 2) + effective_io_concurrency`
- Query timeout: 5 сек на уровне приложения
- N+1 prevention: fetch issues + assignees + labels in one query
- JSONB для кастомных полей задачи
- Read replicas для отчётов (опционально)

## 3. Кеширование

### Уровни
1. **In-memory**: `moka` для hot data (workflow, issue types, project config)
2. **Distributed**: Redis для сессий, rate limit, WS subscriptions
3. **HTTP**: ETag / Cache-Control для static assets

### Инвалидация
- Cache invalidation через доменные события
- Пример: `IssueStatusChanged` → invalidate board cache

## 4. API

- Pagination: cursor-based для больших списков
- Field selection: `?fields=summary,status,assignee`
- Bulk operations: create/update/delete multiple issues
- Request timeout: 30 сек
- Compression: gzip/brotli

## 5. WebSocket

- Redis pub/sub для cross-instance broadcast
- Topic per board / per issue
- Throttle: max 1 update/sec per issue

## 6. Поиск

- PostgreSQL `tsvector` для MVP
- Meilisearch/OpenSearch как опция
- JQL AST → SQL builder
- Индексация при изменении issue через события

## 7. Rate limiting

- `tower_governor` per IP / per user
- Default: 100 req/min anonymous, 1000 req/min authenticated
- Upload endpoints: отдельные лимиты

## 8. Фоновые задачи

- `apalis` с PostgreSQL backend
- Email notifications batching
- Reindex search
- Cleanup expired sessions
- Audit log rotation

## 9. Мониторинг

- `metrics` crate + Prometheus exporter
- Key metrics:
  - `http_requests_total` (method, route, status)
  - `http_request_duration_seconds` histogram
  - `db_pool_connections` gauge
  - `cache_hit_ratio`
  - `ws_connections_active`
  - `job_queue_depth`
- `/health`, `/ready`, `/metrics` endpoints

## 10. Load testing

- `k6` / `oha` / `drill`
- Scenarios:
  - login flow
  - create issue
  - load kanban board
  - search JQL
  - add comment
- Целевые параметры: 100 RPS, P95 < 200 мс

## 11. Масштабирование

- Stateless app instances → scale horizontally
- Redis для shared state
- PostgreSQL read replica для read-heavy queries
- CDN для статики
- Object storage для attachments
