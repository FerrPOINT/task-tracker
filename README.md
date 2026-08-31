# Task Tracker

Self-hosted task tracker для FerrPOINT: проекты, задачи, канбан, backlog, sprints, comments, attachments, notifications, reports and admin tooling in a Rust + React stack.

| Поле | Значение |
|---|---|
| Статус | MVP/product hardening branch, env-префикс `TASKTRACKER_` |
| Backend | Rust 2024, Axum, SeaORM, PostgreSQL 17, Redis 8 |
| Frontend | React 19, Vite, Tailwind CSS |
| API | Canonical OpenAPI artifact in [openapi/openapi.json](openapi/openapi.json) |
| Порты | Frontend `19877`, backend `3456`, PostgreSQL/Redis внутри compose-сети |
| Лицензия | [FerrPOINT Proprietary Source-Available Evaluation License v1.0](LICENSE) |

## Что есть

- Projects, members, kanban boards, backlog, issue detail, comments, attachments and search.
- Issue metadata: priorities, labels, issue types, links, watchers, votes and worklog.
- Sprints with planning/reporting surfaces: velocity, burndown, cumulative flow and control chart.
- Notifications: in-app center, SSE push, email digest worker and per-user delivery settings.
- Project custom fields, components, versions/milestones and soft-delete trash flow.
- Admin area: users, instance settings, audit log, security headers, rate limits and Prometheus metrics.
- CLI binary `task-tracker` with API-driven commands and JSON/table/compact output.

## Границы

- Это self-hosted MVP, а не hosted multi-tenant SaaS.
- PostgreSQL and Redis are internal in Compose by default; expose them only deliberately.
- Before shared deployments, replace all `[CHANGE_ME]` values, set `TASKTRACKER_JWT_SECRET`, review CORS/cookie settings and put the stack behind TLS/reverse proxy.
- Generated frontend API code must be refreshed after OpenAPI changes.

## Быстрый старт

```bash
cp .env.example .env
# Replace POSTGRES_PASSWORD and TASKTRACKER_JWT_SECRET in .env
docker compose up -d
curl http://127.0.0.1:3456/api/v1/health
```

Frontend dev:

```bash
cd frontend
pnpm install
pnpm generate:api
pnpm dev
```

Vite opens on `http://localhost:5173` and proxies API calls to the backend.

## CLI

```bash
cd backend
cargo build --bin task-tracker

export TASKTRACKER_API_URL=http://localhost:3456/api/v1
export TASKTRACKER_TOKEN=<jwt_token>

./target/debug/task-tracker project list
./target/debug/task-tracker issue list --project-key DEMO --output table
```

AI/CLI usage notes live in [cli/SKILL.md](cli/SKILL.md).

## Работа

| Команда | Назначение |
|---|---|
| `just setup` | Install backend/frontend dependencies |
| `just setup-env` | Create `.env` from `.env.example` |
| `just db-up` / `just db-down` | Start/stop Compose dependencies |
| `just backend-dev` | Run backend with database |
| `just frontend-dev` | Run frontend dev server |
| `just api-codegen` | Regenerate frontend API client from OpenAPI |
| `just test` | Backend and frontend tests |
| `just e2e` | Playwright E2E tests |
| `just gate` | CI-like fmt/clippy/typecheck/test gate |
| `just build` | Backend release and frontend production build |

## Структура

```text
task-tracker/
├── backend/     # Rust workspace: api, app, domain, infra, shared, server, cli, migration
├── frontend/    # React SPA: pages, widgets, generated API client
├── cli/         # CLI binary notes and agent skill
├── openapi/     # Canonical API contract
├── docs/        # architecture, deployment, testing, roadmap
└── docker-compose.yml
```

## Документы

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) - архитектура.
- [docs/TZ.md](docs/TZ.md) - техническое задание.
- [docs/DATA_MODEL.md](docs/DATA_MODEL.md) - дата-модель.
- [docs/API.md](docs/API.md) - API notes.
- [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) - deployment.
- [docs/TESTING.md](docs/TESTING.md) - проверки.
- [docs/ROADMAP.md](docs/ROADMAP.md) - roadmap.
- [docs/AGENTS.md](docs/AGENTS.md) - агентские инструкции.

Скриншоты лежат в [docs/screenshots](docs/screenshots); README оставляет их в документации, а не растягивает главную страницу.

## Лицензия

Proprietary source-available. Not open source.

Viewing/evaluation only.

Commercial, production, resale, redistribution, SaaS/hosting use require written license from FerrPOINT. См. [LICENSE](LICENSE), [NOTICE](NOTICE) и [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
