# Архитектура Task Tracker

## 1. Контекст

Self-hosted таск-трекер (Jira-like). MVP покрывает проекты, канбан-доску, бэклог, поиск, дашборд, создание задач и JWT-аутентификацию.

Связь frontend ↔ backend реализована через OpenAPI-first: `openapi/openapi.json` генерируется из Rust-кода, TypeScript-клиент `frontend/src/api/generated.ts` обновляется командой `pnpm generate-api`, запросы идут через `openapi-fetch`, состояния кешируются через `@tanstack/react-query`.

## 2. Технологический стек

### Backend

| Компонент | Библиотека | Версия |
|---|---|---|
| Язык | Rust | 1.97.1 |
| Web framework | axum | 0.8.3 |
| Async runtime | tokio | 1.44 |
| DB ORM | sea-orm | 1.1 |
| Raw SQL | sqlx | 0.8 |
| Migrations | sea-orm-migration | 1.1 |
| Config | config | 0.15 |
| Auth | jsonwebtoken + argon2 | 9.3 / 0.5 |
| Validation | validator | 0.19 |
| HTTP middleware | tower-http | 0.6 |
| OpenAPI | utoipa + utoipa-axum + utoipa-swagger-ui | 5.0 / 0.2 / 9.0 |
| IDs | uuid | 1.16 |
| Time | chrono | 0.4 |
| Optional cache | moka + redis | 0.12 / 0.29 |
| CLI | clap | 4.5 |
| Testing | tokio-test + reqwest | 0.4 / 0.12 |

### Frontend

| Компонент | Библиотека | Версия |
|---|---|---|
| Framework | react + react-dom | 19.1.0 |
| Build | vite | 6.2.0 |
| Styling | tailwindcss + @tailwindcss/vite | 4.1.0 |
| Components | shadcn/ui | — |
| Router | react-router | 8.1.0 |
| Server state | @tanstack/react-query | 5.74.4 |
| Client state | zustand | 5.0.3 |
| Forms | react-hook-form + zod | 7.55.0 / 3.25.60 |
| i18n | i18next + react-i18next | 25.1.0 / 15.5.0 |
| Unit tests | vitest + @testing-library/react | 4.1.10 / 16.x |
| E2E tests | @playwright/test | 1.61.1 |
| Types | typescript | 5.9.3 |

### Infrastructure

- PostgreSQL 17
- Docker + Docker Compose
- Backend порт: `3456`
- Frontend dev порт: `5173`
- Env prefix: `TASKTRACKER_`

## 3. Структура монорепозитория

```
task-tracker/
├── backend/
│   ├── Cargo.toml          # workspace
│   ├── api/                # axum routes + DTO
│   ├── app/                # сервисы / use cases
│   ├── domain/             # entities + repository traits
│   ├── infra/              # postgres repos + event bus
│   ├── shared/             # config, errors, id utils
│   ├── server/             # entrypoint
│   ├── cli/                # утилиты командной строки
│   ├── migration/          # sea-orm migrations
│   └── scripts/
│       └── run-e2e-tests.sh # coverage gate
├── frontend/
│   ├── src/
│   │   ├── api/            # openapi-fetch client + ручные API
│   │   ├── app/            # router, providers
│   │   ├── entities/       # dto/types
│   │   ├── features/       # feature slices
│   │   ├── pages/          # страницы
│   │   ├── shared/         # ui-kit, lib, i18n
│   │   └── widgets/        # app-shell
│   ├── e2e/                # Playwright specs
│   └── src/**/*.test.tsx   # Vitest unit tests
├── docker-compose.yml
├── .env.example
├── justfile                # unified dev commands
├── lefthook.yml            # git hooks
└── docs/
    ├── AGENTS.md
    ├── ARCHITECTURE.md
    ├── API.md
    ├── API_EDGE_CASES.md
    ├── API_STANDARDS.md
    ├── API_VERSIONING.md
    ├── SECURITY.md
    ├── BACKUP_RESTORE.md
    ├── CACHING.md
    ├── CI_CD.md
    ├── CLI.md
    ├── CODE_REVIEW.md
    ├── CODE_STYLE.md
    ├── DATABASE_INDEXES.md
    ├── DATABASE_STANDARDS.md
    ├── DATA_MODEL.md
    ├── BACKUP_RESTORE.md
    ├── DEPLOYMENT.md
    ├── DESIGN_TOKENS.md
    ├── DOMAIN_MODEL.md
    ├── ERROR_HANDLING.md
    ├── EVENTS.md
    ├── FAQ.md
    ├── FEATURE_FLAGS.md
    ├── FRONTEND_ARCHITECTURE.md
    ├── FRONTEND_STANDARDS.md
    ├── GLOSSARY.md
    ├── I18N.md
    ├── JIRA_GAP_DETAILS.md
    ├── JIRA_UI_CAPTURE.md
    ├── API.md
    ├── LIBRARIES.md
    ├── LOAD_BALANCING.md
    ├── LOCAL_SETUP.md
    ├── LOGGING_STANDARDS.md
    ├── MIGRATIONS.md
    ├── MONITORING.md
    ├── NOTIFICATIONS.md
    ├── ONBOARDING.md
    ├── OPS_RUNBOOK.md
    ├── PAGINATION.md
    ├── PERFORMANCE.md
    ├── PROJECT_ADMIN.md
    ├── REACT_STYLING.md
    ├── RELEASE.md
    ├── REPORTS.md
    ├── RESILIENCE.md
    ├── ROADMAP.md
    ├── ROUTING.md
    ├── RUNTIME.md
    ├── SECURITY.md
    ├── SECURITY.md
    ├── STORAGE.md
    ├── SYSTEM_ADMIN.md
    ├── TESTING.md
    ├── TROUBLESHOOTING.md
    ├── TZ.md
    ├── UI_LIBRARIES.md
    ├── UI_UX.md
    ├── USER_STORIES.md
    ├── UX_PRODUCT.md
    ├── EVENTS.md
    ├── WORKFLOW.md
    └── adr/0001-rust-axum.md ... adr/0010-apalis.md
```

## 4. Backend: слоистая архитектура

### 4.1 Presentation layer (`api/`)

Тонкий HTTP-адаптер. Отвечает за:
- извлечение path/query/body/auth state
- route-уровневую валидацию (`ProjectKey::is_valid`, UUID parse)
- вызов сервисных функций из `app/`
- маппинг `AppError` → HTTP статус через `IntoResponse`

Все защищённые маршруты проходят через JWT-middleware (`api/src/middleware/auth.rs`).

### 4.2 Application layer (`app/`)

Сервисы содержат бизнес-операции:
- `auth.rs` — регистрация/логин, хеширование, JWT
- `services.rs` — CRUD проектов, задач, дашборд, поиск, board move

Общая логика маппинга и счётчиков вынесена в `app/src/services/helpers.rs`.

### 4.3 Domain layer (`domain/`)

- Entities: `Issue`, `Project`, `User`, `Board`, `Sprint`, `Worklog`
- Repository traits: async, без `delete()` (soft-архивирование не реализовано в MVP)
- In-memory stubs для тестов: `domain/src/stubs/memory.rs`

### 4.4 Infrastructure layer (`infra/`)

- `repos.rs` — SeaORM Postgres-реализации repository traits
- `event_bus.rs` — in-memory event bus
- `entities/` — SeaORM models

## 5. Конфигурация

Конфиг загружается через `config` crate с префиксом `TASKTRACKER_`.

```rust
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub auth: AuthConfig,
}
```

Основные env vars:
- `TASKTRACKER_DATABASE_URL`
- `TASKTRACKER_SERVER_PORT`
- `TASKTRACKER_JWT_SECRET`
- `TASKTRACKER_DATABASE_PASSWORD`

Fallback: если `auth.jwt_secret` не задан, используется `TASKTRACKER_JWT_SECRET`.

## 6. Middleware stack

```rust
Router::new()
    .merge(api_routes())
    .layer(TraceLayer::new_for_http())
    .layer(CorsLayer::permissive())
    .layer(CompressionLayer::new())
```

CORS настроен для локальной разработки (`localhost:5173`, `localhost:4173`).

## 7. Security

- **AuthN**: JWT access token (Bearer), без refresh в MVP
- **Hashing**: argon2id
- **Input validation**: `validator` derive на Request DTO + route-уровневые проверки
- **CORS**: whitelist для dev

## 8. Frontend архитектура

- **Pages** — экраны: login, register, dashboard, projects, project-board, project-backlog, search, issue-create, issue-detail
- **Features** — time-tracking и будущие бизнес-модули
- **Shared** — ui-kit, i18n, auth store, theme, API hooks
- **App** — роутер (`react-router`), провайдеры QueryClient + ThemeProvider
- **Widgets** — `AppShell` с sidebar + header, адаптивный под mobile

## 9. API, документация и тестирование

- OpenAPI-схема — `openapi/openapi.json`
- Детали REST API — `docs/API.md`
- UI/UX — `docs/UI_UX.md`
- Дата-модель — `docs/DATA_MODEL.md`
- Тестирование — `docs/TESTING.md`

## 10. Dev workflow

Управляется через `justfile`:

```bash
just setup       # установка зависимостей
just dev         # backend + frontend
just gate        # fmt + clippy + typecheck + tests
just build       # production build
just e2e         # Playwright tests
```

Git hooks через `lefthook`:
- `pre-commit`: rust fmt check, clippy, frontend typecheck + test + lint
- `pre-push`: backend tests, frontend build, e2e smoke
- `commit-msg`: conventional commits

## 11. Deployment

Подробности — `docs/DEPLOYMENT.md`.

## References

- `README.md`
- `docs/AGENTS.md`
- `docs/DEPLOYMENT.md`
- `docs/TESTING.md`
