# Logging Standards — Task Tracker

> Стартовый документ. До конца разработки часть соглашений может измениться — актуализировать при стабилизации observability-стека.

## 1. Scope

Соглашения по логированию, tracing и структурированным событиям в backend, frontend и CLI Task Tracker.

## 2. Цели

- Быстро находить причину ошибок.
- Не допускать утечки sensitive-данных в логи.
- Сопоставлять запросы между слоями по request ID.
- Снижать объём логов в production без потери диагностической ценности.

## 3. Уровни логирования

| Level | Когда использовать | Примеры |
|---|---|---|
| `TRACE` | Детальная отладка алгоритмов | Вход/выход из функций, циклы |
| `DEBUG` | Разработка и локальный дебаг | SQL-запросы, кэш-хиты |
| `INFO` | Нормальная работа | Запрос обработан, job завершён |
| `WARN` | Нештатная ситуация, но сервис работает | Retry, deprecated endpoint, rate limit близко |
| `ERROR` | Ошибка, требующая внимания | 5xx, неработающий external API, DB fail |
| `FATAL` | Система не может продолжать работу | Невозможно стартовать, паника |

## 4. Структура лога

В production используется JSON. Обязательные поля:

```json
{
  "timestamp": "2026-07-14T12:34:56.789Z",
  "level": "INFO",
  "target": "tasktracker::api::issues",
  "message": "Issue created",
  "request_id": "0192a7b4-...",
  "trace_id": "...",
  "span_id": "...",
  "user_id": "...",
  "project_id": "...",
  "duration_ms": 42,
  "method": "POST",
  "path": "/api/v1/projects/1/issues"
}
```

- `timestamp` — ISO-8601 UTC.
- `level` — uppercase.
- `target` — Rust module / frontend component / CLI command.
- `request_id` — UUID, прокидывается через все слои.
- `duration_ms` — время обработки запроса/операции.

## 5. Что логировать

### Обязательно

- HTTP-запросы: метод, путь, статус, duration, request_id.
- Изменения состояния задачи: кто, когда, старый/новый статус.
- Ошибки аутентификации/авторизации без паролей и токенов.
- Запуск/остановка сервиса, миграции.
- Фоновые job: начало, успех, ошибка, retry.

### Запрещено

- Пароли, plain tokens, API keys.
- PII: email, phone, полные имена (если не анонимизированы).
- Полные SQL-запросы с параметрами в production.
- Большие бинарные payloads, file content.

## 6. Request ID propagation

- Gateway (Traefik) генерирует `X-Request-ID`.
- Backend извлекает или создаёт `request_id` на уровне middleware.
- Frontend добавляет `X-Request-ID` ко всем API-вызовам.
- `request_id` включается в ответ заголовком `X-Request-ID`.
- CLI передаёт `X-Request-ID` в заголовках.

## 7. Tracing

- Backend: `tracing` + OpenTelemetry.
- Один top-level span на HTTP-запрос.
- Вложенные spans: DB query, external HTTP call, cache lookup, queue job.
- Frontend: OpenTelemetry Web SDK для user interactions (опционально).

## 8. Локальная разработка

- Pretty-формат для читаемости.
- `RUST_LOG=tasktracker=debug,sqlx=trace`.
- Логи пишутся в stdout.

## 9. Production

- JSON в stdout → сборщик (Loki / Fluent Bit).
- Sample rate для `INFO`-запросов: 100% для ошибок, 10% для успешных health/metrics.
- Retention: 30 дней hot, 90 дней cold.
- Алерты на рост `ERROR`/`FATAL` и аномалии latency.

## 10. Sensitive data policy

- Все sensitive-поля заменяются на `[REDACTED]`.
- Проверка перед коммитом: `scripts/scan-secrets.sh`.
- Security incident: см. `docs/SECURITY_INCIDENT_RESPONSE.md`.

## 11. CLI и фоновые job

- CLI команда пишет `INFO` о старте/завершении.
- Job log содержит `job_id`, `queue`, `attempt`, `duration_ms`.
- Dead-letter queue логируется как `ERROR`.

## 12. References

- `docs/ARCHITECTURE.md` — общая архитектура и observability.
- `docs/MONITORING.md` — метрики, dashboards, алерты.
- `docs/SECURITY.md` — обработка sensitive данных.
- `docs/ERROR_HANDLING.md` — формат ошибок и retry-политика.
- `docs/OPS_RUNBOOK.md` — операционные процедуры.
- `docs/DEPLOYMENT.md` — инфраструктура и log aggregation.
