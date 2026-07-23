# Task Tracker

Self-hosted таск-трекер, аналог Jira. Backend — Rust (Axum + SeaORM + PostgreSQL), frontend — React 19.1.0 + Vite 6.2.0 + Tailwind CSS 4.1.0. Порт по умолчанию `19876`, env-префикс `TASKTRACKER_`, локали `ru`/`en`.

## Подход

Documentation-first: перед кодом фиксируем функционал, дата-модель, API, UI/UX, деплой и безопасность. Код пишем поэтапно от MVP к полному функционалу Jira-like таск-трекера.

## Структура

- `backend/` — Rust workspace (Axum + SeaORM + PostgreSQL)
- `frontend/` — React 19.1.0 + Vite 6.2.0
- `cli/` — CLI-утилита (Rust)
- `docs/` — архитектура, ТЗ, дата-модель, API, workflow, JQL, user stories, UI/UX, frontend-библиотеки, design tokens, React styling guide, frontend architecture, project admin, system admin, notifications, reports, CLI spec, AGENTS.md, deployment, migrations, file storage, caching, routing, error handling, i18n, code style, ADR, security, monitoring, производительность, тестирование, библиотеки, отчёт по Vikunja, Jira structural samples

## Документы

- [Архитектура](docs/ARCHITECTURE.md)
- [Техническое задание](docs/TZ.md)
- [Дата-модель](docs/DATA_MODEL.md)
- [API](docs/API.md)
- [Workflow](docs/WORKFLOW.md)
- [JQL](docs/JQL.md)
- [User Stories](docs/USER_STORIES.md)
- [UI/UX Specification](docs/UI_UX.md)
- [Frontend UI/UX Libraries](docs/UI_LIBRARIES.md)
- [Design Tokens](docs/DESIGN_TOKENS.md)
- [React Styling Guide](docs/REACT_STYLING.md)
- [Frontend Architecture](docs/FRONTEND_ARCHITECTURE.md)
- [Project Admin](docs/PROJECT_ADMIN.md)
- [System Admin](docs/SYSTEM_ADMIN.md)
- [Notifications](docs/NOTIFICATIONS.md)
- [Reports](docs/REPORTS.md)
- [CLI Spec](docs/CLI.md)
- [AGENTS.md](docs/AGENTS.md)
- [Deployment](docs/DEPLOYMENT.md)
- [Migrations](docs/MIGRATIONS.md)
- [File Storage](docs/STORAGE.md)
- [Caching](docs/CACHING.md)
- [Routing](docs/ROUTING.md)
- [Error Handling](docs/ERROR_HANDLING.md)
- [i18n](docs/I18N.md)
- [Code Style](docs/CODE_STYLE.md)
- [ADR](docs/ADR.md)
- [Security](docs/SECURITY.md)
- [Monitoring](docs/MONITORING.md)
- [Производительность](docs/PERFORMANCE.md)
- [Тестирование](docs/TESTING.md)
- [CI/CD](docs/CI_CD.md)
- [Библиотеки](docs/LIBRARIES.md)
- [Отчёт по сравнению с Vikunja](docs/VIKUNJA_GAP_ANALYSIS.md)
- [Jira UI Capture (структурные заметки)](docs/JIRA_UI_CAPTURE.md)
- [ROADMAP](docs/ROADMAP.md)
- [DATABASE INDEXES](docs/DATABASE_INDEXES.md)
- [GLOSSARY](docs/GLOSSARY.md)
- [API VERSIONING](docs/API_VERSIONING.md)
- [WEBSOCKET EVENTS](docs/WEBSOCKET_EVENTS.md)
- [EVENTS](docs/EVENTS.md)
- [OPS RUNBOOK](docs/OPS_RUNBOOK.md)
- [RELEASE](docs/RELEASE.md)
- [ONBOARDING](docs/ONBOARDING.md)
- [RUNTIME](docs/RUNTIME.md)
- [RESILIENCE](docs/RESILIENCE.md)
- [AUTH ADVANCED](docs/AUTH_ADVANCED.md)
- [PAGINATION](docs/PAGINATION.md)
- [DATA RETENTION](docs/DATA_RETENTION.md)
- [FEATURE FLAGS](docs/FEATURE_FLAGS.md)
- [LOAD BALANCING](docs/LOAD_BALANCING.md)
- [UX PRODUCT](docs/UX_PRODUCT.md)
- [API EDGE CASES](docs/API_EDGE_CASES.md)
- [SECURITY INCIDENT RESPONSE](docs/SECURITY_INCIDENT_RESPONSE.md)
- [JIRA GAP DETAILS](docs/JIRA_GAP_DETAILS.md)
- [API Standards](docs/API_STANDARDS.md)
- [Domain Model](docs/DOMAIN_MODEL.md)
- [Database Standards](docs/DATABASE_STANDARDS.md)
- [Local Setup](docs/LOCAL_SETUP.md)
- [Troubleshooting](docs/TROUBLESHOOTING.md)
- [FAQ](docs/FAQ.md)
- [Backup & Restore](docs/BACKUP_RESTORE.md)
- [Frontend Standards](docs/FRONTEND_STANDARDS.md)
- [Logging Standards](docs/LOGGING_STANDARDS.md)
- [Code Review Guidelines](docs/CODE_REVIEW.md)

## Architecture Decisions

Архитектурные решения зафиксированы в [docs/ADR.md](docs/ADR.md) и `docs/adr/`:

- [ADR-0001: Rust + Axum](docs/adr/0001-rust-axum.md)
- [ADR-0002: React 19.1.0 + Vite 6.2.0](docs/adr/0002-react-vite.md)
- [ADR-0003: PostgreSQL](docs/adr/0003-postgresql.md)
- [ADR-0004: SeaORM + SQLx](docs/adr/0004-seaorm-sqlx.md)
- [ADR-0005: Feature-Sliced Design](docs/adr/0005-feature-sliced-design.md)
- [ADR-0006: JWT access + httpOnly refresh cookie](docs/adr/0006-auth-jwt-refresh.md)
- [ADR-0007: Redis для кэша и WS pub/sub](docs/adr/0007-redis.md)
- [ADR-0008: shadcn/ui + Tailwind CSS 4.1.0](docs/adr/0008-shadcn-tailwind.md)
- [ADR-0009: TanStack Query + Zustand](docs/adr/0009-query-zustand.md)
- [ADR-0010: apalis для фоновых задач](docs/adr/0010-apalis.md)

## API / OpenAPI

- Backend is the source of truth for the API schema via `utoipa`.
- Generated schema: [`openapi/openapi.json`](openapi/openapi.json)
- OpenAPI workflow: [`openapi/README.md`](openapi/README.md)
- Frontend client is generated from the schema into `frontend/src/api/generated.ts`.

## Быстрый старт

```bash
cp .env.example .env
# отредактируй .env
docker compose up -d
```

Приложение доступно на `http://localhost:19876`.
Локали: `ru` (по умолчанию), `en`.

## Тесты

```bash
# Backend
cd backend && cargo test

# Frontend
cd frontend && pnpm typecheck && pnpm test -- --run && pnpm build
```

## OpenAPI generation

```bash
cd backend
cargo run --bin openapi-gen -- /opt/dev/task-tracker/openapi/openapi.json
cd ../frontend
pnpm generate:api
```

## Разработка

- [CONTRIBUTING.md](CONTRIBUTING.md)
- [CHANGELOG.md](CHANGELOG.md)

## Контакты / репозиторий

- Remote: `git@github.com:FerrPOINT/task-tracker.git`
- Ветка: `main`

## References

- `docs/ARCHITECTURE.md` — общая архитектура и стек.
- `docs/TZ.md` — техническое задание и scope.
- `CONTRIBUTING.md` — процесс разработки и review.
