# Local Setup

Локальный запуск и разработка Task Tracker.

## 1. Требования

| Инструмент | Минимальная версия | Примечание |
|---|---|---|
| Docker + Compose | 24.x | весь стек: Postgres, Redis, backend, frontend |
| Rust | 1.86+ | backend (workspace в `backend/`) |
| Node.js | 22 LTS | frontend |
| pnpm | 9.x+ | frontend package manager |
| git | 2.40+ | — |

## 2. Быстрый старт (Docker, весь стек)

```bash
git clone git@github.com:FerrPOINT/task-tracker.git /opt/dev/task-tracker
cd /opt/dev/task-tracker

cp .env.example .env
# обязательно задай POSTGRES_PASSWORD и TASKTRACKER_JWT_SECRET (без рабочих дефолтов)

docker compose up -d --build
```

После старта:

- Frontend: `http://localhost:19877`
- Backend API: `http://localhost:3456/api/v1`
- Health: `http://localhost:3456/api/v1/health` (без rate-limit)
- Swagger UI: `http://localhost:3456/swagger-ui`
- Postgres/Redis: внутренние (compose-сеть), наружу не публикуются; доступ — `docker compose exec postgres psql -U tasktracker`

Демо-аккаунт (если выполнен seed): `demo@example.com` / пароль из `scripts/seed-demo.sh`.

## 3. Переменные окружения

Формат: `TASKTRACKER_SECTION__KEY` (двойное подчёркивание — вложенный ключ; `TASKTRACKER_SERVER_ADDRESS` с одним подчёркиванием НЕ парсится).

Основные переменные — в `.env.example` (Postgres-параметры compose, JWT-секрет, лимиты, порты `BACKEND_PORT`/`FRONTEND_PORT`). Секреты обязательны: `docker compose` откажется стартовать без `POSTGRES_PASSWORD` и `TASKTRACKER_JWT_SECRET`.

## 4. Backend (разработка без Docker)

```bash
cd backend

cargo build

# миграции применяются автоматически при старте сервера; вручную:
cargo run -p migration -- up

# API сервер (порт 3456)
cargo run --bin server

# тесты (юнит + integration на in-memory стеке)
cargo test --workspace

# docker-backed инфра-тесты (нужен Postgres из compose)
cargo test -p infra --test repos -- --include-ignored --test-threads=1
```

## 5. Frontend (разработка)

```bash
cd frontend

pnpm install
pnpm dev          # Vite-прокси на backend :3456

pnpm typecheck
pnpm lint
pnpm test -- --run          # vitest
pnpm build                  # генерирует src/api/generated.ts из openapi/openapi.json

# E2E против живого Docker-стека
pnpm exec playwright test --project=chromium
```

## 6. Генерация OpenAPI-контракта

Источник истины — Rust-хендлеры (utoipa):

```bash
cd backend && cargo run -p api --bin gen-openapi > ../openapi/openapi.json
cd ../frontend && pnpm generate:api && pnpm typecheck
```

Не редактируйте `openapi/openapi.json` и `frontend/src/api/generated.ts` вручную.

## 7. Типичные проблемы

| Симптом | Решение |
|---|---|
| Порт 19877/3456 занят | `FRONTEND_PORT` / `BACKEND_PORT` в `.env` |
| compose не стартует: `POSTGRES_PASSWORD is required` | задай секрет в `.env` |
| backend unhealthy | `docker compose logs backend`; healthcheck — `wget http://127.0.0.1:3456/api/v1/health` |
| `TASKTRACKER_SERVER_ADDRESS` игнорируется | используй `TASKTRACKER_SERVER__ADDRESS` |

## References

- [Архитектура](ARCHITECTURE.md)
- [Деплой](DEPLOYMENT.md)
- [Бэкап и восстановление](BACKUP_RESTORE.md)
- [Runbook](OPS_RUNBOOK.md)
