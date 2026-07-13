# Производительность и оптимизация Task Tracker

## 1. Цели

- P95 ответа API < 200 мс при 100 RPS на типичных запросах.
- Kanban-доска до 1000 задач загружается < 1 сек.
- Полнотекстовый поиск по 100K задач < 500 мс.
- First Contentful Paint frontend < 1.5 сек.
- WebSocket broadcast до 10K одновременных соединений.

## 2. Backend оптимизации

### 2.1 База данных

#### Индексы

```sql
-- Основные запросы доски
CREATE INDEX idx_issues_project_status ON issues(project_id, status_id);
CREATE INDEX idx_issues_project_assignee ON issues(project_id, assignee_id);

-- Поиск и фильтры
CREATE INDEX idx_issues_project_created ON issues(project_id, created_at DESC);
CREATE INDEX idx_issues_updated ON issues(updated_at DESC);

-- Full-text search
CREATE INDEX idx_issues_search_vector ON issues USING GIN(search_vector);

-- JSONB custom fields
CREATE INDEX idx_issues_custom_fields ON issues USING GIN(custom_fields);

-- Уведомления
CREATE INDEX idx_notifications_user_unread ON notifications(user_id, is_read) WHERE is_read = false;

-- Audit
CREATE INDEX idx_audit_entity ON audit_log(entity_type, entity_id, created_at DESC);
```

#### Полнотекстовый поиск

- `tsvector` на `title || ' ' || description || ' ' || comments`.
- Обновление через триггер или background job.
- Альтернатива: `pg_search` (ParadeDB) или Elasticsearch для масштаба.

#### Пагинация

- Offset-based только для таблиц с ограничением ≤ 1000 строк.
- Cursor-based для лент, истории, уведомлений.
- Keyset pagination для больших списков задач.

### 2.2 Кеширование

| Уровень | Технология | Что кешировать |
|---------|------------|----------------|
| In-memory | `moka` | Часто читаемые справочники: issue types, statuses, permissions |
| Distributed | Redis | Сессии, rate limit, WS pub/sub, rendered dashboards |

- TTL: справочники — 5 мин, permissions — 1 мин, dashboard — 30 сек.
- Cache invalidation на событиях изменения.

### 2.3 Запросы

- SQLx `query_as!` с compile-time проверкой.
- N+1 исключены через JOIN и batch-загрузку.
- Доска: один запрос на задачи + один на статусы + lazy для assignees.
- Lazy loading для комментариев и истории в карточке задачи.

### 2.4 WebSocket

- Hub на `tokio::sync::broadcast`.
- Каналы по проектам.
- Redis pub/sub при горизонтальном масштабировании.
- Message batching: группировка мелких событий.
- heartbeat каждые 30 сек.

### 2.5 Connection pool

- SQLx pool: `min=5`, `max=20` на инстанс.
- Acquire timeout: 5 сек.
- Statement timeout: 10 сек.

### 2.6 Async и concurrency

- CPU-bound задачи (рендеринг экспорта, поиск) — `tokio::task::spawn_blocking`.
- Background jobs через cron + PostgreSQL `pg_cron` или отдельный worker.

### 2.7 Логирование и observability

- `tracing` со structured JSON logs.
- OpenTelemetry: traces + metrics.
- Метрики Prometheus:
  - `http_requests_total`
  - `http_request_duration_seconds`
  - `db_pool_active_connections`
  - `ws_connections_total`
- Health endpoint `/health` + readiness `/ready`.

### 2.8 Rate limiting

- `tower-governor`:
  - auth: 10 req/min/IP.
  - API: 100 req/min/user.
  - search: 30 req/min/user.

### 2.9 Uploads

- Stream upload, ограничение 50 MB.
- Проверка MIME-типа magic bytes.
- Хранение вне web-root.
- Thumbnails генерируются async.

## 3. Frontend оптимизации

### 3.1 Бандл

- Vite 6 + Rollup.
- Code splitting по route и feature.
- Tree-shaking для UI-библиотек.
- Lazy load: admin, settings, dashboards.

### 3.2 Рендеринг

- React 19: `useTransition`, `useOptimistic`, `use`.
- Virtualized списки задач: `@tanstack/react-virtual`.
- Kanban: рендерим только видимые карточки.
- Memoization компонентов доски.

### 3.3 Server state

- TanStack Query:
  - staleTime: доска — 30 сек, фильтры — 5 мин, справочники — 10 мин.
  - Optimistic updates при move/transition.
  - Prefetch на hover.

### 3.4 Realtime

- WebSocket сообщения вызывают targeted invalidation.
- Debounce на частые события.

### 3.5 Assets

- SVG-иконки.
- WebP для аватаров.
- CDN для статики (опционально).

### 3.6 CSS

- Tailwind v4 с CSS-first конфигурацией.
- Purge unused styles в продакшене.
- CSS variables для тем.

## 4. Нагрузочное тестирование

### 4.1 Инструменты

- `k6` или `oha` для HTTP.
- Кастомный WS load generator на Rust/Tokio.
- Playwright для измерения метрик frontend.

### 4.2 Сценарии

| Сценарий | Целевая метрика |
|----------|-----------------|
| Login + create issue | p95 < 500 мс |
| Load board 1000 issues | < 1 сек |
| Search 100K issues | < 500 мс |
| 100 concurrent WS | CPU < 30%, latency < 50 мс |
| 1000 concurrent users | ошибки < 0.1% |

### 4.3 Профилирование

- Rust: `cargo flamegraph`, `tokio-console`.
- Frontend: Chrome DevTools Performance, Lighthouse.
- DB: `EXPLAIN ANALYZE`, `pg_stat_statements`.

## 5. Масштабирование

### 5.1 Горизонтальное масштабирование backend

- Stateless инстансы за балансировщиком.
- Shared PostgreSQL.
- Redis для сессий и WS pub/sub.

### 5.2 Read replicas

- Отчёты, поиск, audit log — readonly replica.
- SQLx pool настраивается на write/read разделение.

### 5.3 Шардинг (v3)

- Шардинг по `project_id` при миллионах проектов.

## 6. Мониторинг

- Prometheus + Grafana.
- Alerting: p95 > 500 мс, ошибки > 1%, CPU > 80%, DB connections > 80%.

## 7. Чек-лист перед релизом

- [ ] Все N+1 устранены.
- [ ] Индексы добавлены для новых запросов.
- [ ] Rate limits настроены.
- [ ] Cache invalidation проверен.
- [ ] Load tests пройдены.
- [ ] Lighthouse ≥ 90.
- [ ] Graceful shutdown работает.
- [ ] Логи и метрики собираются.
