# Monitoring — Task Tracker

## 1. Overview

Мониторинг покрывает metrics, logs, traces, alerting. Стек: Prometheus, Grafana, Loki, Alertmanager, OpenTelemetry.

## 2. Metrics

### 2.1 Backend Metrics (Prometheus)

| Metric | Type | Description |
|--------|------|-------------|
| `http_requests_total` | counter | total requests by method, route, status |
| `http_request_duration_seconds` | histogram | request latency |
| `http_request_size_bytes` | histogram | request body size |
| `http_response_size_bytes` | histogram | response body size |
| `active_websocket_connections` | gauge | current WS connections |
| `websocket_messages_total` | counter | messages sent/received |
| `db_pool_connections` | gauge | active/idle DB connections |
| `db_query_duration_seconds` | histogram | query latency |
| `redis_pool_connections` | gauge | Redis connections |
| `cache_hit_total` | counter | cache hits by namespace |
| `cache_miss_total` | counter | cache misses by namespace |
| `background_jobs_total` | counter | jobs processed |
| `background_jobs_failed_total` | counter | failed jobs |
| `rate_limited_requests_total` | counter | rate-limited requests |

### 2.2 Frontend Metrics

- Core Web Vitals (LCP, INP, CLS) — `web-vitals` library.
- API error rate.
- Query cache hit/miss (TanStack Query devtools).

### 2.3 Business Metrics

| Metric | Description |
|--------|-------------|
| `issues_created_total` | created issues |
| `issues_resolved_total` | resolved issues |
| `active_projects` | projects with activity |
| `active_users` | DAU/MAU |

## 3. Logging

### 3.1 Format

JSON structured logs:

```json
{
  "timestamp": "2026-07-13T10:00:00Z",
  "level": "INFO",
  "target": "task_tracker_api::issue_service",
  "message": "issue created",
  "request_id": "req-uuid",
  "user_id": "user-uuid",
  "project_id": "project-uuid",
  "issue_id": "issue-uuid",
  "duration_ms": 42
}
```

### 3.2 Levels

| Level | Use |
|-------|-----|
| ERROR | failures requiring attention |
| WARN | recoverable issues |
| INFO | significant operations |
| DEBUG | dev diagnostics |
| TRACE | very verbose |

### 3.3 Frontend Logs

- Console logs only in dev.
- Production: send errors to Sentry-compatible endpoint (future).

## 4. Tracing

- OpenTelemetry для распределённой трассировки.
- Trace ID прокидывается через `x-trace-id`.
- Spans: controller → service → repository → DB/Redis.

## 5. Alerting

### 5.1 Critical

- API down > 1 min.
- DB unavailable.
- 5xx rate > 1%.
- Disk > 85%.

### 5.2 Warning

- 4xx rate > 10%.
- P95 latency > 1s.
- Cache hit ratio < 70%.
- Background job failures > 5/hour.

## 6. Dashboards

| Dashboard | Panels |
|-----------|--------|
| API Overview | RPS, latency, errors, WS connections |
| Database | query time, pool, slow queries |
| Cache | hit/miss, size, eviction |
| Infrastructure | CPU, memory, disk, network |
| Business | DAU, issue velocity, reports |

## 7. Log Aggregation

- Promtail/Loki for backend logs.
- Grafana Alloy for frontend errors (future).
- Retention: 30 days hot, 1 year cold.

## 8. Health Endpoints

| Endpoint | Purpose |
|----------|---------|
| `GET /health` | liveness |
| `GET /health/ready` | readiness (DB + Redis) |
| `GET /metrics` | Prometheus |

## 9. Uptime Monitoring

- Blackbox exporter или external service (UptimeRobot).
- Check `/health` каждые 60s.

## 10. Profiling

- CPU/memory profiling через `pprof` (future).
- Async flamegraphs для Rust.

## 11. CI/CD Observability

- Build duration.
- Test pass/fail rate.
- Deployment frequency.
- Mean time to recovery (MTTR).

## 12. Configuration

```yaml
# backend/config/monitoring.toml
[metrics]
enabled = true
bind = "0.0.0.0:9090"
endpoint = "/metrics"

[tracing]
enabled = true
exporter = "otlp"
otlp_endpoint = "http://tempo:4317"

[logging]
format = "json"
level = "info"
```

## 13. Local Monitoring

```bash
docker compose -f docker-compose.yml -f docker-compose.monitoring.yml up -d
```

Доступ:

- Grafana: http://localhost:3001
- Prometheus: http://localhost:9090
- Loki: http://localhost:3100

## 14. Privacy

- Не логировать персональные данные.
- Не логировать пароли, токены, cookies.
- Маскировать email в логах.
## References

- `docs/ARCHITECTURE.md`
- `docs/DEPLOYMENT.md`
- `docs/PERFORMANCE.md`
