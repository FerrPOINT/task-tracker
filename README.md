# Task Tracker

Self-hosted таск-трекер: Rust (axum + SeaORM + PostgreSQL) + React (Vite + Tailwind). Порт по умолчанию `19876`, env-префикс `TASKTRACKER_`.

## Текущее состояние MVP

- Проекты, канбан-доска, бэклог, поиск, дашборд, создание задач.
- Авторизация JWT (access 15 мин + refresh cookie 7 дней).
- Демо-пользователь: `demo@example.com` / `demo`.
- OpenAPI: `openapi/openapi.json`, TypeScript клиент генерируется в `frontend/src/api/generated.ts`.

## Быстрый старт

```bash
# 1. Скопировать env
backend/.env.example > backend/.env

# 2. Поднять БД и backend
docker compose up -d postgres redis backend

# 3. Проверить API
curl http://127.0.0.1:3456/api/v1/health

# 4. Frontend (dev)
cd frontend
pnpm install
pnpm generate:api
pnpm dev

# Или собрать и раздать статику
pnpm build
pnpm preview
```

## Команды

| Команда | Описание |
|---------|----------|
| `docker compose up -d postgres redis backend` | Поднять backend + БД |
| `cd backend && cargo test` | Unit-тесты backend |
| `cd frontend && pnpm typecheck` | TypeScript |
| `cd frontend && pnpm test -- --run` | Unit-тесты frontend |
| `cd frontend && pnpm build` | Production build |
| `cd frontend && pnpm exec playwright test e2e/smoke.spec.ts` | E2E smoke |

## Смена порта

```bash
# docker-compose.yml
services:
  backend:
    ports:
      - "19876:3456"
```

Или env `TASKTRACKER_SERVER__PORT=3456` внутри контейнера и внешний порт привязки.

## Структура

- `backend/` — Rust workspace (`api`, `app`, `domain`, `infra`, `shared`, `server`, `migration`)
- `frontend/` — React SPA (`src/pages/`, `src/shared/api/`, `src/widgets/`)
- `openapi/` — канонический `openapi.json`
- `docs/` — архитектура, ТЗ, дата-модель, UI/UX, деплойment

## Документы

- [Архитектура](docs/ARCHITECTURE.md)
- [Техническое задание](docs/TZ.md)
- [Дата-модель](docs/DATA_MODEL.md)
- [UI/UX](docs/UI_UX.md)
- [Deployment](docs/DEPLOYMENT.md)
- [AGENTS.md](docs/AGENTS.md)
