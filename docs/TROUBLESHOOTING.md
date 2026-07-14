# Troubleshooting

## 1. Сборка и запуск

### `cargo build` падает с ошибкой линковки

- Убедиться, что установлены dev-зависимости: `openssl-dev`, `pkg-config` (Debian/Ubuntu: `libssl-dev pkg-config`).
- Проверить версию Rust: `rustc --version` ≥ 1.80.

### Frontend dev-сервер не стартует

- Проверить Node.js: `node --version` ≥ 22.
- Удалить `node_modules` и lockfile: `rm -rf node_modules pnpm-lock.yaml`, затем `pnpm install`.
- Проверить, что порт 19876 не занят: `lsof -i :19876`.

### Docker compose не поднимается

```bash
docker compose down -v
docker compose pull
docker compose up -d --build
```

## 2. База данных

### Миграции не применяются

```bash
cd backend
cargo run --bin migrator -- --status
# или
sqlx migrate info
```

Если застряло — откатить вручную:

```bash
sqlx migrate revert
```

### Connection refused to postgres

- Проверить, что контейнер postgres healthy: `docker compose ps`.
- Проверить `TASKTRACKER_DATABASE_URL` — хост должен быть `localhost` для локального запуска, `postgres` для docker.
- Проверить credentials в `.env`.

### Медленные запросы

```sql
SELECT query, mean_exec_time, calls
FROM pg_stat_statements
ORDER BY mean_exec_time DESC
LIMIT 10;
```

См. `docs/DATABASE_INDEXES.md`.

## 3. Redis

### Redis connection refused

- `docker compose ps` — redis healthy?
- Проверить `TASKTRACKER_REDIS_URL`.
- Проверить, что не путаете host `redis` vs `localhost`.

### WebSocket не рассылает между инстансами

- Убедиться, что `TASKTRACKER_REDIS_URL` настроен.
- Проверить pub/sub: `redis-cli PUBLISH test "hello"`.

## 4. Auth

### Access token rejected

- Проверить TTL (по умолчанию 15 минут).
- Проверить `Authorization: Bearer <token>`.
- Проверить `TASKTRACKER_JWT_SECRET` — должен совпадать у сервера, выпустившего токен.

### Refresh cookie не приходит

- Проверить `Secure` flag — в локальной HTTP-среде может быть выключен.
- Проверить `SameSite=Lax`.
- См. `docs/SECURITY.md`.

## 5. API

### 422 Validation Error

- Тело ответа содержит список полей и ошибок.
- Проверить required поля и формат UUID.

### 409 Conflict

- Чаще всего duplicate key (unique constraint).
- Проверить комбинации: `project_key`, `issue_key`, `email`, `username`.

### 429 Too Many Requests

- Проверить заголовки `X-RateLimit-*`.
- Подождать или использовать `Idempotency-Key`.

## 6. Frontend

### Белый экран после сборки

- Открыть DevTools → Console.
- Проверить, что `VITE_API_URL` доступен.
- Проверить 404 на `index.html` — настройка SPA fallback.

### Tailwind стили не применяются

- `pnpm dev` перезапустить.
- Проверить `@import "tailwindcss"` в `frontend/src/styles/index.css`.

### i18n не подгружается

- Проверить, что JSON-локали в `frontend/src/i18n/locales/`.
- Проверить fallback locale (`ru`).

## 7. Тесты

### Playwright flaky

```bash
pnpm exec playwright install --with-deps
pnpm exec playwright test --workers=1 --retries=2
```

### Cargo тесты падают на DB

- Убедиться, что `TEST_DATABASE_URL` настроен (обычно отдельная DB `tasktracker_test`).
- Запускать миграции перед тестами.

## 8. Диагностика

### Health checks

```bash
curl http://localhost:8080/health
curl http://localhost:8080/health/ready
curl http://localhost:8080/metrics
```

### Логи

```bash
# backend
cargo run --bin server 2>&1 | jq

# docker
docker compose logs -f api
```

## 9. References

- `docs/LOCAL_SETUP.md`
- `docs/DEPLOYMENT.md`
- `docs/DATABASE_INDEXES.md`
- `docs/SECURITY.md`
- `docs/ERROR_HANDLING.md`
- `docs/MONITORING.md`
- `docs/TESTING.md`
