# Monitoring — Task Tracker

## 1. Что реально есть

- **Prometheus-метрики** backend: `GET /metrics` (без авторизации только для trusted/local exposure; в production закрывайте edge-маршрут или ставьте `TASKTRACKER_METRICS__PUBLIC=false`).
- **Логи** — `tracing` в stdout (`RUST_LOG`, по умолчанию `info`); `docker compose logs backend`.
- **Health** — `GET /api/v1/health` (вне rate-limit, для Docker healthcheck и мониторинга).

Фактические метрики (экспортер `axum-prometheus`):

| Metric | Type | Description |
|--------|------|-------------|
| `axum_http_requests_total` | counter | всего HTTP-запросов (method, route, status) |
| `axum_http_requests_duration_seconds` | histogram | задержка запросов |
| `axum_http_requests_duration_seconds_count/_sum` | counter | счётчики гистограммы |
| `axum_http_requests_pending` | gauge | запросы в полёте |

## 2. Скрейпинг Prometheus

```yaml
scrape_configs:
  - job_name: task-tracker
    scrape_interval: 15s
    static_configs:
      - targets: ['127.0.0.1:3456']
```

Если backend публикуется наружу напрямую, оставьте health public, но отключите публичный metrics route:

```bash
TASKTRACKER_METRICS__PUBLIC=false docker compose up -d backend
```

Grafana/Loki/Alertmanager/OpenTelemetry в поставку не входят — подключаются администратором при необходимости.

## 3. Метрики НЕ реализованы

Следующие метрики из ранних версий доков отсутствуют в рантайме: WebSocket-счётчики (WS не используется — доставка событий через SSE `/api/v1/events`), пулы DB/Redis, cache hit/miss, background jobs, бизнес-метрики (`issues_created_total` и т.п.), Core Web Vitals фронтенда.

## References

- [OPS_RUNBOOK](OPS_RUNBOOK.md) — ежедневные проверки
- [EVENTS](EVENTS.md) — SSE-события
- [LOGGING_STANDARDS](LOGGING_STANDARDS.md)
