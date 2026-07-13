# Task Tracker

Self-hosted таск-трекер, аналог Jira. Backend — Rust (Axum + SeaORM + PostgreSQL), frontend — React 19 + Vite 6 + Tailwind CSS v4. Порт по умолчанию `19876`, env-префикс `TASKTRACKER_`, локали `ru`/`en`.

## Подход

Documentation-first: перед кодом фиксируем функционал, дата-модель, API, UI/UX, деплой и безопасность. Код пишем поэтапно от MVP к полному функционалу Jira-like таск-трекера.

## Структура

- `backend/` — Rust workspace (Axum + SeaORM + PostgreSQL)
- `frontend/` — React 19 + Vite 6
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

## Разработка

- [CONTRIBUTING.md](CONTRIBUTING.md)
- [CHANGELOG.md](CHANGELOG.md)

## Контакты / репозиторий

- Remote: `git@github.com:FerrPOINT/task-tracker.git`
- Ветка: `main`
