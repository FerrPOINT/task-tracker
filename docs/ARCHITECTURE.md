# Архитектура Task Tracker

## Контекст

Self-hosted таск-трекер, упрощённый аналог Jira. MVP-фокус: проекты, задачи, статусы, kanban-доска, фильтры, комментарии, вложения, уведомления.

## Структура репозитория

```
.
├── backend/          # Rust REST API + WebSocket
├── frontend/         # React + Vite + TypeScript
├── cli/              # CLI-утилита (стек позже)
├── docs/             # Архитектура, ТЗ, API
├── docker-compose.yml
└── README.md
```

## Стек

| Слой | Технология |
|------|------------|
| Backend | Rust, Axum или Actix-web, SQLx, PostgreSQL |
| Frontend | React 19, Vite 6, TypeScript 5.9, Tailwind 4, TanStack Query |
| CLI | TBD (Rust clap / Go cobra / Python click) |
| База данных | PostgreSQL 16+ |
| Realtime | WebSocket (native) или SSE |
| Контейнеры | Docker + Docker Compose |

## Backend (Rust)

### Слои

```
bin/               # Точка входа
config/            # ENV-конфиг, валидация
db/                # Подключение к PostgreSQL, миграции
models/            # SQLx-структуры и доменные типы
repositories/      # Доступ к данным (чистые SQLx-запросы)
services/          # Бизнес-логика
handlers/          # HTTP-хендлеры (Axum/Actix)
routes/            # Маршрутизация
middleware/        # Auth, CORS, логирование, rate limit
errors/            # Единый error type
websocket/         # WS hub
notifications/     # Email/push/in-app уведомления
```

### Ключевые принципы

- Без ORM. SQLx + миграции `migrations/*.sql`.
- Dependency injection через конструкторы сервисов.
- Единый формат ошибок: `{ "error": { "code": "", "message": "" } }`.
- Асинхронность: `tokio`, `async-trait`.
- Пароли: `argon2`.
- JWT для сессий, refresh-токены в cookie `httpOnly`.
- ID: UUIDv7.

### API

- REST + JSON.
- Версионирование: `/api/v1/...`.
- WebSocket: `/ws` для realtime-уведомлений и обновлений доски.
- Документация: OpenAPI 3.1, рендер через Scalar или Redoc.

## Frontend (React)

### Слои

```
src/
  api/              # Клиенты axios/fetch, TanStack Query hooks
  components/       # UI-компоненты (shadcn/ui база)
  features/         # Feature-based модули: auth, projects, issues, board, filters
  hooks/            # Общие hooks
  stores/           # Zustand: auth, ui state
  routes/           # React Router 7
  i18n/             # ru, en
  types/            # Генерация из OpenAPI или ручные
  utils/            # helpers
```

### Ключевые принципы

- Feature-based структура.
- Server state — TanStack Query.
- Client state — Zustand.
- Формы — React Hook Form + Zod.
- DnD kanban — @dnd-kit/core.
- Тёмная тема по умолчанию.
- Мобильная адаптация обязательна.

## База данных

### Таблицы (MVP)

- `users`
- `workspaces` / `projects`
- `issue_types`
- `issues`
- `issue_statuses`
- `status_categories` (todo, in_progress, done)
- `issue_links` / `issue_relations`
- `comments`
- `attachments`
- `boards` (kanban) + `board_columns`
- `filters` / `saved_filters`
- `notifications`
- `audit_log`
- `labels` + `issue_labels`
- `assignees` (many-to-many users ↔ issues)
- `watchers`

### Индексы

- `issues.project_id`, `issues.status_id`, `issues.assignee_id`, `issues.created_at`.
- GIN на JSONB-полях кастомных полей.
- Full-text search по `issues.title`, `issues.description` через `tsvector`.

## Realtime

- WebSocket-соединение аутентифицируется по JWT из query-param или cookie.
- События: `issue.created`, `issue.updated`, `issue.moved`, `comment.added`, `notification`.
- Frontend подписывается на канал проекта.

## CLI

Пока не определён стек. Возможности:

- `task-tracker issue list --project X --status Y`
- `task-tracker issue create --title ... --project ...`
- `task-tracker migrate up`
- `task-tracker user create`

## Инфраструктура

### Docker Compose

- PostgreSQL 16.
- Backend (port 19876).
- Frontend dev-сервер (port 19877) — только для разработки.
- Прод: фронтенд билдится в статику и отдаётся backend/nginx.

### ENV

Префикс `TASKTRACKER_`:

- `TASKTRACKER_DATABASE_URL`
- `TASKTRACKER_HTTP_PORT`
- `TASKTRACKER_JWT_SECRET`
- `TASKTRACKER_LOG_LEVEL`
- `TASKTRACKER_REDIS_URL` (опционально, для сессий/очередей)

## Безопасность

- CORS ограничен origin.
- Rate limiting на auth-эндпоинтах.
- Загрузка файлов: проверка MIME, ограничение размера, хранение в S3 или локальной FS.
- SQL-инъекции исключены SQLx (compile-time checked queries).

## Эволюция

1. MVP: проекты, задачи, статусы, kanban, фильтры, комментарии, вложения, уведомления.
2. Расширение: спринты, эпики, workflow-переходы, разрешения, роли, webhooks.
3. Масштаб: горизонтальный шардинг проектов, очереди задач, интеграции.
