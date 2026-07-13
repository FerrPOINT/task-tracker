# Архитектура Task Tracker (Jira-like)

## 1. Контекст

Self-hosted таск-трекер, полноценный аналог open-source Jira. Покрывает полный жизненный цикл задач: проекты, типы задач, workflow, kanban/scrum-доски, фильтры, поиск, комментарии, вложения, уведомления, роли/разрешения, спринты, эпики, метрики.

## 2. Структура репозитория (monorepo)

```
task-tracker/
├── backend/                 # Rust REST API + WS
│   ├── crates/
│   │   ├── task-tracker-api/        # HTTP/WS сервер (bin)
│   │   ├── task-tracker-core/       # Модели, ошибки, доменная логика
│   │   ├── task-tracker-db/         # Репозитории, SQLx, миграции
│   │   ├── task-tracker-services/   # Бизнес-сервисы
│   │   ├── task-tracker-auth/       # JWT, argon2, сессии
│   │   ├── task-tracker-search/     # Полнотекстовый поиск, фильтры
│   │   ├── task-tracker-realtime/   # WebSocket hub, SSE fallback
│   │   └── task-tracker-notify/     # Уведомления: in-app, email, push
│   ├── migrations/
│   ├── tests/
│   └── Cargo.toml
├── frontend/                # React 19 + Vite 6
│   ├── src/
│   │   ├── api/
│   │   ├── features/        # auth, projects, issues, board, filters, admin
│   │   ├── components/
│   │   ├── hooks/
│   │   ├── stores/
│   │   ├── routes/
│   │   ├── i18n/
│   │   ├── types/
│   │   └── utils/
│   ├── tests/
│   │   ├── unit/
│   │   ├── integration/
│   │   └── e2e/
│   └── package.json
├── cli/                     # CLI-утилита (стек позже)
├── docs/
│   ├── ARCHITECTURE.md
│   ├── TZ.md
│   ├── PERFORMANCE.md
│   └── TESTING.md
├── docker-compose.yml
├── Dockerfile.backend
├── Dockerfile.frontend
└── README.md
```

## 3. Стек (актуальные версии на 2025–2026)

| Слой | Технология | Версия |
|------|------------|--------|
| Язык backend | Rust | 1.85+ |
| Web-фреймворк | Axum | 0.8+ |
| Runtime | Tokio | 1.43+ |
| База данных | PostgreSQL | 17 |
| Database access | SQLx | 0.8.3+ |
| Миграции | sqlxmigrate / refinery | latest |
| Auth | argon2 + jsonwebtoken | latest |
| Валидация | garde | 0.22+ |
| Сериализация | serde + serde_json | 1.0.219+ |
| Логирование | tracing + tracing-subscriber | 0.1.44+ |
| HTTP client | reqwest | 0.12.15+ |
| OpenAPI | utoipa | 5.0+ |
| Testing | cargo test + testcontainers | latest |
| Frontend | React | 19.1+ |
| Bundler | Vite | 6.2+ |
| TypeScript | TypeScript | 5.9+ |
| Styling | Tailwind CSS | 4.1+ |
| UI kit | shadcn/ui (React 19 + Tailwind v4) | latest |
| Forms | React Hook Form | 7.66+ |
| Валидация форм | Zod | 4.1+ |
| Server state | TanStack Query | 5.93+ |
| Client state | Zustand | 5.0+ |
| Router | React Router | 7.5+ |
| DnD | @dnd-kit/core | 6.3+ |
| i18n | i18next | 25.0+ |
| Markdown | react-markdown | 10.0+ |
| Unit tests | Vitest | 3.2+ |
| E2E / screenshots | Playwright | 1.52+ |
| Component tests | @testing-library/react | 16.3+ |

## 4. Backend архитектура (Rust)

### 4.1 Workspace crates

Каждый crate отвечает за один слой. Зависимости направлены внутрь:

```
api → services → db → core
       ↓           ↓
      auth    search/realtime/notify
```

- `task-tracker-core` — доменные типы, `Error` enum, `Result<T>`, ID-типы, константы.
- `task-tracker-db` — пулы SQLx, репозитории, сырые SQL-запросы, миграции.
- `task-tracker-services` — бизнес-логика, транзакции, политики доступа.
- `task-tracker-api` — Axum-роуты, middleware, WebSocket, OpenAPI.
- `task-tracker-auth` — регистрация, логин, JWT, argon2, refresh-токены.
- `task-tracker-search` — JQL-подобный поиск, фильтры, saved filters, full-text.
- `task-tracker-realtime` — WebSocket hub, каналы, broadcast.
- `task-tracker-notify` — in-app, email SMTP, push (опционально).

### 4.2 Структура crate `task-tracker-api`

```
src/
  bin/
    main.rs              # Точка входа
  config/
    mod.rs               # ENV-конфиг ( envy + serde )
  routes/
    mod.rs               # Монтирование роутов
    v1/
      auth.rs
      users.rs
      projects.rs
      issues.rs
      boards.rs
      filters.rs
      comments.rs
      attachments.rs
      admin.rs
      search.rs
      notifications.rs
  handlers/              # Тонкие обёртки над сервисами
  middleware/
    auth.rs              # JWT extractor
    cors.rs
    rate_limit.rs        # tower-governor
    request_id.rs
    trace.rs
  websocket/
    hub.rs
    protocol.rs
  errors/
    mod.rs               # Единый Error → JSON
  openapi.rs             # utoipa::OpenApi
```

### 4.3 Структура crate `task-tracker-services`

```
src/
  auth_service.rs
  user_service.rs
  project_service.rs
  issue_service.rs
  board_service.rs
  filter_service.rs
  search_service.rs
  comment_service.rs
  attachment_service.rs
  notification_service.rs
  admin_service.rs
  workflow_service.rs
  role_service.rs
  permission_service.rs
```

Каждый сервис:

```rust
pub struct IssueService {
    db: Arc<DbPool>,
    search: Arc<dyn SearchRepository>,
    notifier: Arc<dyn Notifier>,
    realtime: Arc<dyn RealtimeBus>,
}

impl IssueService {
    pub fn new(...)-> Self { ... }
    pub async fn create(&self, ctx: Ctx, cmd: CreateIssue) -> Result<Issue> { ... }
}
```

### 4.4 Структура crate `task-tracker-db`

```
src/
  pool.rs
  migrations/
  repositories/
    user_repo.rs
    project_repo.rs
    issue_repo.rs
    board_repo.rs
    comment_repo.rs
    filter_repo.rs
    attachment_repo.rs
    notification_repo.rs
    role_repo.rs
    search_repo.rs
  models/                # SQLx FromRow-структуры
  tx.rs                  # Транзакции
```

Принцип: **без ORM**. SQL-запросы в `.sql` файлах или inline с `sqlx::query_as!`. Индексы и ограничения описаны в миграциях.

### 4.5 Auth

- Регистрация: email + пароль.
- Пароль: argon2id (memory=19 MiB, iterations=2, parallelism=1).
- Access token: JWT, TTL 15 мин.
- Refresh token: UUIDv4, хранится в PostgreSQL `refresh_tokens`, передаётся в `httpOnly` cookie.
- Middleware извлекает access token из `Authorization: Bearer <token>` или cookie.
- CSRF: SameSite=Lax для cookie + CORS origin whitelist.

### 4.6 Роли и разрешения

- Роли: `system_admin`, `project_admin`, `project_lead`, `developer`, `viewer`, `guest`.
- Разрешения гранулярные: `issue:create`, `issue:update`, `issue:delete`, `project:settings`, `board:admin`, `filter:manage`, `user:admin`.
- Проверка в сервисном слое через `PermissionService`.

### 4.7 Workflow

- Каждый проект имеет набор статусов.
- Статус привязан к категории: `todo`, `in_progress`, `done`.
- Workflow-переходы задают разрешённые переходы между статусами и проверки.
- Перемещение задачи на доске = `POST /issues/:id/transitions`.

### 4.8 Realtime

- WebSocket endpoint `/ws`.
- Аутентификация: access token в query-параметре `?token=`.
- Каналы: `user:{id}`, `project:{id}`, `issue:{id}`, `board:{project_id}`.
- Fallback: Server-Sent Events `/sse`.
- События: `IssueCreated`, `IssueUpdated`, `IssueMoved`, `CommentAdded`, `Notification`, `BoardRefresh`.

### 4.9 Уведомления

- In-app: таблица `notifications`, пуш через WS.
- Email: SMTP через `lettre`.
- Push: Web Push (опционально).
- Шаблоны: Handlebars / Tera.

## 5. Frontend архитектура (React 19)

### 5.1 Структура `frontend/src`

```
src/
  api/                   # Клиент axios/fetch, TanStack Query hooks
    client.ts
    hooks/
      useAuth.ts
      useProjects.ts
      useIssues.ts
      useBoard.ts
      useFilters.ts
      useNotifications.ts
  features/              # Feature-based модули
    auth/
    projects/
    issues/
    board/
    filters/
    search/
    comments/
    attachments/
    admin/
    notifications/
    settings/
  components/              # Переиспользуемые UI-компоненты
    ui/                    # shadcn/ui база
    layout/
    data-table/
    board/
    forms/
  hooks/                   # Общие hooks
  stores/                  # Zustand
    authStore.ts
    uiStore.ts
    boardStore.ts
  routes/                  # React Router 7
  i18n/                    # ru, en
  types/                   # Сгенерированные из OpenAPI
  utils/
  main.tsx
  App.tsx
```

### 5.2 Принципы

- **Feature-based**: каждая фича содержит свои компоненты, API-hooks, формы, типы.
- **Server state** — TanStack Query: кеш, invalidation, optimistic updates.
- **Client state** — Zustand: auth, UI, board DnD-черновик.
- **Forms** — React Hook Form + Zod (`@hookform/resolvers/zod`).
- **Валидация API-ответов** — Zod-схемы или openapi-typescript.
- **Routing** — React Router 7 в data-mode.
- **Темы** — тёмная по умолчанию, CSS variables через Tailwind v4.

### 5.3 Доска

- `@dnd-kit/core` + `@dnd-kit/sortable`.
- Колонки = статусы проекта.
- Перетаскивание вызывает `POST /issues/:id/transitions`.
- Optimistic update в TanStack Query.
- Realtime: при получении `BoardRefresh` — refetch.

### 5.4 Поиск и фильтры

- Полнотекстовый поиск по `title`, `description`, `comments`.
- JQL-подобный язык: `project = PROJ AND status in (Open, "In Progress") AND assignee = currentUser()`.
- Сохранённые фильтры.
- Результаты в таблице и на доске.

## 6. База данных

### 6.1 Таблицы

- `users`, `user_profiles`, `user_settings`
- `roles`, `permissions`, `role_permissions`, `project_roles`, `project_members`
- `projects`, `project_keys`
- `issue_types`, `issue_types_in_projects`
- `issue_statuses`, `status_categories`, `workflow_transitions`
- `issues`, `issue_history`, `issue_links`, `issue_relations`
- `comments`
- `attachments`
- `labels`, `issue_labels`
- `components`, `issue_components`
- `versions`, `issue_fix_versions`, `issue_affected_versions`
- `boards`, `board_columns`, `board_column_issues`
- `sprints`, `sprint_issues`
- `filters`, `filter_queries`
- `notifications`
- `email_queue`
- `audit_log`
- `refresh_tokens`
- `api_keys`
- `webhooks` (опционально)

### 6.2 Индексы

- B-tree: `issues.project_id`, `issues.status_id`, `issues.assignee_id`, `issues.created_at`, `issues.updated_at`.
- GIN: `issues.search_vector` (tsvector), `issues.custom_fields` (JSONB).
- Partial index: активные спринты, непрочитанные уведомления.
- Unique: `project_keys.key`, `issues.project_id + project_issue_number`.

### 6.3 Миграции

- `refinery` или `sqlx migrate`.
- Версионирование, откат через down-скрипты.
- Seed-данные: статусы по умолчанию, роли, системный админ.

## 7. API

### 7.1 REST

- Версионирование: `/api/v1/...`.
- Content-Type: `application/json`.
- Ошибки:
  ```json
  {
    "error": {
      "code": "VALIDATION_ERROR",
      "message": "...",
      "details": { "field": ["..."] }
    }
  }
  ```
- Пагинация: cursor-based для лент, offset-based для таблиц.

### 7.2 Основные группы эндпоинтов

- `/api/v1/auth/*`
- `/api/v1/users/*`
- `/api/v1/projects/*`
- `/api/v1/projects/:id/issues/*`
- `/api/v1/issues/:id/*`
- `/api/v1/projects/:id/boards/*`
- `/api/v1/projects/:id/workflow/*`
- `/api/v1/filters/*`
- `/api/v1/search`
- `/api/v1/notifications/*`
- `/api/v1/admin/*`
- `/api/v1/attachments/*`

### 7.3 OpenAPI + документация

- `utoipa` генерирует спецификацию.
- UI: Scalar по `/api/docs`.
- Генерация frontend-типов: `openapi-typescript`.

## 8. Тестирование

Подробнее в `TESTING.md`. Кратко:

- **Backend unit**: сервисы через mock-репозитории.
- **Backend integration**: testcontainers PostgreSQL, реальные репозитории.
- **Backend e2e**: reqwest + запущенное приложение.
- **Frontend unit**: Vitest + @testing-library/react.
- **Frontend integration**: MSW + TanStack Query.
- **E2E / screenshots**: Playwright, mobile + desktop, visual regression.

## 9. Инфраструктура

### 9.1 Docker Compose

```yaml
services:
  postgres:
    image: postgres:17-alpine
  backend:
    build:
      context: ./backend
      dockerfile: ../Dockerfile.backend
    ports:
      - "19876:19876"
  frontend:
    build:
      context: ./frontend
      dockerfile: ../Dockerfile.frontend
    ports:
      - "19877:80"
```

### 9.2 ENV

Префикс `TASKTRACKER_`:

- `TASKTRACKER_DATABASE_URL`
- `TASKTRACKER_HTTP_PORT=19876`
- `TASKTRACKER_JWT_SECRET`
- `TASKTRACKER_JWT_ACCESS_TTL=900`
- `TASKTRACKER_JWT_REFRESH_TTL=604800`
- `TASKTRACKER_LOG_LEVEL=info`
- `TASKTRACKER_REDIS_URL` (опционально)
- `TASKTRACKER_SMTP_HOST`, `TASKTRACKER_SMTP_FROM`

## 10. Безопасность

- Argon2id, JWT, httpOnly refresh cookie.
- Rate limiting на auth (10 req/min на IP).
- CORS whitelist.
- SQL-injection исключена SQLx.
- XSS: React экранирует по умолчанию; markdown через DOMPurify.
- CSRF: SameSite=Lax + state-changing POST через JWT.
- Загрузка файлов: MIME-check, size limit, quarantine-директория.

## 11. Производительность

Подробнее в `PERFORMANCE.md`.

## 12. Эволюция

1. **MVP-каркас**: auth, проекты, задачи, статусы, kanban, комментарии.
2. **Core Jira**: workflow, роли, фильтры, поиск, уведомления, вложения.
3. **Agile**: спринты, эпики, story points, burndown.
4. **Enterprise**: LDAP/OAuth, SLA, автоматизация rules, webhooks, plugins.

## 13. Референс функционала (open-source Jira)

- Atlassian Jira Data Center / Server.
- OpenProject (структура проектов, роли, agile-доски).
- YouTrack (поиск, команды).
- Redmine (workflow, роли).

