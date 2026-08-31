<p align="center">
  <img src="https://capsule-render.vercel.app/api?type=waving&height=190&text=Task%20Tracker&desc=Self-hosted%20planning%2C%20kanban%20and%20issue%20operations&fontColor=F8FAFC&fontSize=52&fontAlignY=35&descAlignY=56&color=0:111827,50:2563EB,100:14B8A6" alt="Task Tracker banner" />
</p>

<p align="center">
  <a href="#features"><img src="https://img.shields.io/badge/%E2%9C%A8%20Features-0B1220?style=for-the-badge" alt="Features" /></a>
  <a href="#stack"><img src="https://img.shields.io/badge/%F0%9F%94%A7%20Stack-111827?style=for-the-badge" alt="Stack" /></a>
  <a href="#screenshots"><img src="https://img.shields.io/badge/%F0%9F%96%BC%EF%B8%8F%20Screens-334155?style=for-the-badge" alt="Screenshots" /></a>
  <a href="#cli"><img src="https://img.shields.io/badge/%F0%9F%96%A5%EF%B8%8F%20CLI-1F2937?style=for-the-badge" alt="CLI" /></a>
  <a href="#architecture"><img src="https://img.shields.io/badge/%F0%9F%8F%97%EF%B8%8F%20Architecture-374151?style=for-the-badge" alt="Architecture" /></a>
  <a href="#quality"><img src="https://img.shields.io/badge/%F0%9F%9B%A1%EF%B8%8F%20Quality-4B5563?style=for-the-badge" alt="Quality" /></a>
  <a href="#license"><img src="https://img.shields.io/badge/%F0%9F%94%92%20License-Proprietary%20source--available-7F1D1D?style=for-the-badge" alt="License" /></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-2024-000000?style=flat-square&logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/Axum-111827?style=flat-square" alt="Axum" />
  <img src="https://img.shields.io/badge/SeaORM-2563EB?style=flat-square" alt="SeaORM" />
  <img src="https://img.shields.io/badge/PostgreSQL-17-4169E1?style=flat-square&logo=postgresql&logoColor=white" alt="PostgreSQL" />
  <img src="https://img.shields.io/badge/Redis-8-DC382D?style=flat-square&logo=redis&logoColor=white" alt="Redis" />
  <img src="https://img.shields.io/badge/React-19-61DAFB?style=flat-square&logo=react&logoColor=111827" alt="React" />
  <img src="https://img.shields.io/badge/Vite-646CFF?style=flat-square&logo=vite&logoColor=white" alt="Vite" />
  <img src="https://img.shields.io/badge/OpenAPI-6BA539?style=flat-square&logo=openapiinitiative&logoColor=white" alt="OpenAPI" />
</p>

---

## 🎯 Позиционирование

**Task Tracker** — self-hosted task tracker для FerrPOINT: проекты, задачи, kanban, backlog, sprints, comments, attachments, notifications, reports and admin tooling.

Это продуктовый MVP/hardening branch, а не hosted multi-tenant SaaS. Env-префикс: `TASKTRACKER_`.

## 📌 Snapshot

| Поле | Значение |
|---|---|
| Backend | Rust 2024, Axum, SeaORM |
| Data | PostgreSQL 17, Redis 8 |
| Frontend | React 19, Vite, Tailwind CSS |
| API | [openapi/openapi.json](openapi/openapi.json) |
| Ports | Frontend `19877`, backend `3456`, PostgreSQL/Redis internal |
| License | FerrPOINT Proprietary Source-Available Evaluation License v1.0 |

## 🚪 Default Ports

| Сервис | Доступ | Описание |
|---|---|---|
| Frontend Docker | `19877` | Nginx static frontend |
| Backend | `3456` | API |
| PostgreSQL | internal compose network | DB, not published externally |
| Redis | internal compose network | cache, not published externally |

<a name="features"></a>
## ✨ Features

| Feature | Описание |
|---|---|
| Projects and issues | Projects, members, issue detail, comments, attachments and search. |
| Planning | Kanban boards, backlog, sprints and worklog. |
| Metadata | Priorities, labels, issue types, links, watchers, votes, components and versions. |
| Reporting | Velocity, burndown, cumulative flow and control chart surfaces. |
| Notifications | In-app center, SSE push, email digest worker and per-user delivery settings. |
| Administration | Users, instance settings, audit log, security headers, rate limits and Prometheus metrics. |
| CLI | `task-tracker` binary with JSON/table/compact output. |

## 🧩 Capability Details

| Area | Details |
|---|---|
| Projects and issues | Projects with kanban boards, backlog, dashboard and search; issue create/edit/status transitions, comments, attachments, priorities, labels, issue types, links, assignees and worklog. |
| Kanban and sprints | Drag-and-drop board columns, sprint planning and reports: velocity, burndown, cumulative flow and control chart. |
| Notifications | In-app center, unread counters, SSE `NotificationCreated`, hourly/daily email digest, `email_frequency`, `disabled_event_types` and `notify_own_changes`. |
| Watchers and votes | Watch subscriptions, issue votes and vote counters. |
| Custom fields | Project-level text, number, select, multi-select and date fields with required flags and issue-level values. |
| Components and versions | Project components, release/milestone versions, `released`/`release_date`, affected/fix version links. |
| Soft delete | `deleted_at` trash model, restore and permanent purge. |
| Search/admin | JQL search, admin panel, users, instance settings, audit log, security headers, rate limiting and Prometheus metrics. |

<a name="stack"></a>
## 🔧 Core Stack

| Zone | Tech | Роль |
|---|---|---|
| API | Rust + Axum | HTTP routes, auth, DTO boundary |
| Domain/App | Rust workspace crates | services, policies and repository contracts |
| Persistence | SeaORM + PostgreSQL | runtime data and migrations |
| Cache/Push | Redis + SSE | cache and real-time delivery |
| Frontend | React + Vite + Tailwind | dashboard, boards and admin UI |
| Contract | OpenAPI | generated frontend API client |

## ⚡ Quick Start

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

Port override:

```env
BACKEND_PORT=3456
FRONTEND_PORT=19877
```

After changing host ports, recreate services with `docker compose up -d`. Inside the compose network the backend still listens on `3456`; backend settings use the `TASKTRACKER_SECTION__KEY` format, for example `TASKTRACKER_SERVER__CORS_ALLOWED_ORIGINS`.

<a name="screenshots"></a>
## 🖼️ Screenshots

| Surface | Preview |
|---|---|
| Login | ![Вход](docs/screenshots/01-login.png) |
| Dashboard | ![Дашборд](docs/screenshots/02-dashboard.png) |
| Projects | ![Проекты](docs/screenshots/03-projects.png) |
| Kanban board | ![Канбан-доска](docs/screenshots/04-board.png) |
| Backlog | ![Бэклог](docs/screenshots/05-backlog.png) |
| Trash | ![Корзина](docs/screenshots/06-trash.png) |
| Custom fields | ![Кастомные поля](docs/screenshots/07-custom-fields.png) |
| Search | ![Поиск](docs/screenshots/08-search.png) |
| Notifications | ![Уведомления](docs/screenshots/09-notifications.png) |
| Reports | ![Отчёты](docs/screenshots/10-reports.png) |
| Administration | ![Администрирование](docs/screenshots/11-admin.png) |
| Issue create | ![Создание задачи](docs/screenshots/12-issue-create.png) |
| Issue detail | ![Страница задачи](docs/screenshots/13-issue-detail.png) |
| Register | ![Регистрация](docs/screenshots/14-register.png) |
| Mobile board | ![Доска на мобильном](docs/screenshots/15-board-mobile.png) |
| Mobile projects | ![Проекты на мобильном](docs/screenshots/16-projects-mobile.png) |

<a name="cli"></a>
## 🖥️ CLI

```bash
cd backend
cargo build --bin task-tracker

export TASKTRACKER_API_URL=http://localhost:3456/api/v1
export TASKTRACKER_TOKEN=<jwt_token>

./target/debug/task-tracker project list
./target/debug/task-tracker issue create --project-key DEMO --summary "Fix bug" --priority high
./target/debug/task-tracker issue list --project-key DEMO --output table
./target/debug/task-tracker board get --project-key DEMO
```

CLI command documentation: [docs/CLI.md](docs/CLI.md). AI usage notes: [cli/SKILL.md](cli/SKILL.md).

<a name="architecture"></a>
## 🏗️ Architecture

```mermaid
flowchart TD
    UI[React SPA] --> API[Axum API]
    CLI[task-tracker CLI] --> API
    API --> App[Application services]
    App --> Domain[Domain contracts]
    App --> Repo[SeaORM repositories]
    Repo --> DB[(PostgreSQL)]
    API --> Redis[(Redis)]
    App --> Notify[SSE + email digest]
    API --> OpenAPI[OpenAPI contract]
    OpenAPI --> Gen[Generated frontend client]
```

## 🧱 Границы

- PostgreSQL and Redis are internal in Compose by default; expose them only deliberately.
- Before shared deployments, replace all `[CHANGE_ME]` values and review JWT, CORS, cookies, TLS and reverse-proxy settings.
- Generated frontend API code must be refreshed after OpenAPI changes.

<a name="quality"></a>
## 🛡️ Quality Bar

| Проверка | Команда |
|---|---|
| Setup | `just setup` |
| Dependencies | `just db-up` / `just db-down` |
| Backend dev | `just backend-dev` |
| Frontend dev | `just frontend-dev` |
| API codegen | `just api-codegen` |
| Test suite | `just test` |
| E2E | `just e2e` |
| CI-like gate | `just gate` |
| Build | `just build` |

## 🧭 Project Map

```text
task-tracker/
├── backend/     # Rust workspace: api, app, domain, infra, shared, server, cli, migration
├── frontend/    # React SPA: pages, widgets, generated API client
├── cli/         # CLI binary notes and agent skill
├── openapi/     # canonical API contract
├── docs/        # architecture, deployment, testing, roadmap
└── docker-compose.yml
```

## 📚 Документы

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — architecture.
- [docs/TZ.md](docs/TZ.md) — technical specification.
- [docs/DATA_MODEL.md](docs/DATA_MODEL.md) — data model.
- [docs/API.md](docs/API.md) — API notes.
- [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) — deployment.
- [docs/TESTING.md](docs/TESTING.md) — checks.
- [docs/ROADMAP.md](docs/ROADMAP.md) — roadmap.
- [docs/AGENTS.md](docs/AGENTS.md) — agent instructions.

Screenshots live in [docs/screenshots](docs/screenshots).

<a name="license"></a>
## 🔒 License

Proprietary source-available. Not open source.

Viewing/evaluation only.

Commercial, production, resale, redistribution, SaaS/hosting use require written license from FerrPOINT. См. [LICENSE](LICENSE), [NOTICE](NOTICE) и [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
