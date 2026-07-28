# Task Tracker

Self-hosted таск-трекер: Rust (axum + SeaORM + PostgreSQL) + React (Vite + Tailwind). Env-префикс `TASKTRACKER_`.

## Порты по умолчанию

| Сервис | Внешний порт | Описание |
|---|---|---|
| Frontend (docker) | `19877` | Nginx статика |
| Backend | `3456` | API |
| PostgreSQL | `3457` | БД |
| Redis | `6379` | Кеш/сессии |
| Traefik (профиль) | `8080` | Reverse proxy |

## Текущее состояние MVP

- Проекты, канбан-доска, бэклог, поиск, дашборд, создание задач, детальная страница задачи.
- Авторизация JWT (access + refresh cookie).
- OpenAPI: `openapi/openapi.json`, TypeScript клиент — `pnpm generate:api`.
- E2E и скриншоты: Playwright (375 / 1920 / 2560, светлая/тёмная тема).

## Быстрый старт

```bash
# 1. Скопировать env
cp .env.example .env

# 2. Поднять инфраструктуру и backend
docker compose up -d

# 3. Проверить API
curl http://127.0.0.1:3456/api/v1/health

# 4. Frontend dev
cd frontend
pnpm install
pnpm generate:api
pnpm dev
```

Frontend dev откроется на `http://localhost:5173`, API проксируется через Vite dev server.

## Команды

Основные команды завёрнуты в `justfile`:

| Команда | Описание |
|---|---|
| `just setup` | Установить зависимости backend + frontend |
| `just setup-env` | Создать `.env` из `.env.example` |
| `just db-up` | Поднять Docker Compose (`postgres`, `redis`, `backend`, `frontend`) |
| `just db-down` | Остановить Docker Compose |
| `just backend-dev` | Поднять backend + БД в Docker |
| `just frontend-dev` | Запустить frontend dev server |
| `just test` | Unit + интеграционные тесты backend + frontend |
| `just e2e` | Playwright E2E тесты |
| `just gate` | CI-like gate: fmt-check, clippy, typecheck, тесты |
| `just build` | Собрать backend release + frontend production |
| `just api-codegen` | Перегенерировать `frontend/src/api/generated.ts` из OpenAPI |

## Смена порта

### Backend

```yaml
# docker-compose.yml
services:
  backend:
    ports:
      - "19876:3456"
```

Или env `TASKTRACKER_SERVER_PORT=3456` внутри контейнера с внешней привязкой на нужный порт.

### Frontend

```yaml
# docker-compose.yml
services:
  frontend:
    ports:
      - "80:80"
```

## Структура

- `backend/` — Rust workspace (`api`, `app`, `domain`, `infra`, `shared`, `server`, `cli`, `migration`)
- `frontend/` — React SPA (`src/pages/`, `src/api/`, `src/widgets/`)
- `openapi/` — канонический `openapi.json`
- `docs/` — архитектура, ТЗ, дата-модель, UI/UX, deployment, AGENTS.md

## Документы

- [Архитектура](docs/ARCHITECTURE.md)
- [Техническое задание](docs/TZ.md)
- [Дата-модель](docs/DATA_MODEL.md)
- [UI/UX](docs/UI_UX.md)
- [Deployment](docs/DEPLOYMENT.md)
- [Testing](docs/TESTING.md)
- [AGENTS.md](docs/AGENTS.md)

## Лицензия

MIT — см. [LICENSE](LICENSE).
