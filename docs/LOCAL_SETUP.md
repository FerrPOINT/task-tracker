# Local Setup

Локальный запуск и разработка Task Tracker.

## 1. Требования

| Инструмент | Минимальная версия | Примечание |
|---|---|---|
| Docker + Compose | 24.x | весь стек: Postgres, Redis, backend, frontend |
| Rust | 1.88+ | backend (workspace в `backend/`; требуется `time 0.3.47`) |
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

### 3.1. Configured CLI Cohort

Замена native PM writers несовместима со старым Fleet. До переключения
приостановить новую legacy-выдачу, завершить активные исполнения и сверить
неизвестные результаты по исходным ключам на совместимом старом cohort.
Затем согласованно заменить Tracker/Fleet/Workflow и упакованные CLI, применяя
обычную forward-цепочку миграций. Старые квитанции, IDs, hashes и история
сохраняются для чтения; удалённые native endpoints не возвращаются.

Новый agent runtime получает `FLEET_CLI_PROTOCOL=configured-cli-v1`, текущий
`FLEET_AGENT_ID`, `TASKTRACKER_API_URL` и постоянный `TASKTRACKER_TOKEN` служебной
identity с обычными read/write scopes и членством в проекте. Workflow CLI имеет
свои `PROJECT_WORKFLOW_URL`, `PROJECT_WORKFLOW_API_TOKEN` и namespace. Tracker
читает receipt отдельным серверным credential: пара
`TASKTRACKER_SDLC__WORKFLOW_URL` / `TASKTRACKER_SDLC__WORKFLOW_READ_TOKEN`;
для технических профессий используются соответствующие серверные read tokens
из `TASKTRACKER_SDLC__WORKFLOW_READ_TOKENS_JSON`. Значения секретов не входят
в source, команды или evidence. Точный контракт: [Configured CLI V1](contracts/CONFIGURED_CLI_V1.md).

Новые задачи явно выбирают configured protocol. Включение проекта не конвертирует
существующие задачи; первое назначение владельцем может принять только ещё
не начатый Draft по существующему правилу миграции109. Начатые legacy-задачи
автоматически не переводятся. Owner confirmation остаётся Draft/InProgress;
проверенные Workflow receipt и итоговый комментарий допускают Backlog/Ready
без автоматического запуска Analyst. Права владельца, проекта и fencing сохраняются.

Source-проверка сохранения заполненной истории использует синтетические
populated seed fixtures и отдельные disposable БД, без копий рабочих данных.
Проверка упакованных entrypoints/JSON относится к CLI cohort; настоящая
Hermes-приёмка всех профессий и Rework относится к runtime release и сама
по себе не является условием merge CLI source без выявленного дефекта.

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
