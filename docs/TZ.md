# Техническое задание: Task Tracker (Jira-like)

## 1. Цель

Разработать self-hosted таск-трекер с функционалом, сопоставимым с open-source Jira (Data Center/Server). Система должна поддерживать проекты, типы задач, workflow, kanban/scrum-доски, фильтры, поиск, комментарии, вложения, уведомления, роли/разрешения, спринты, эпики и метрики.

## 2. Область применения

- Команды разработки.
- IT-службы поддержки (Service Desk light).
- Управление продуктами и проектами.
- Личное самоорганизация.

## 3. Общие требования

### 3.1 Стек

- Backend: Rust + Axum + Tokio + SQLx + PostgreSQL 17.
- Frontend: React 19 + Vite 6 + TypeScript 5.9 + Tailwind CSS 4 + shadcn/ui.
- CLI: TBD.
- Тестирование: Vitest (unit), Playwright (e2e + скриншоты), cargo test (Rust unit/integration).
- Деплой: Docker + Docker Compose.

### 3.2 Качество

- Покрытие тестами:
  - backend unit + integration: ≥ 80% логики сервисов и репозиториев.
  - frontend unit: ≥ 70% компонентов и hooks.
  - e2e: critical path (регистрация → проект → задача → доска → комментарий → поиск).
- После каждой доработки UI — скриншоты в трёх разрешениях: 375×667, 1920×1080, 2560×1440.
- Визуальная регрессия: Playwright + screenshot comparisons.
- Локализация: ru, en.
- Тёмная тема по умолчанию.

## 4. Функциональные требования

### 4.1 Пользователи и auth

- Регистрация по email + пароль.
- Подтверждение email (опционально, включаемое в админке).
- Вход/выход/смена пароля.
- Восстановление пароля через email.
- Профиль: имя, аватар, email, язык, тема, часовой пояс.
- Системные роли: `system_admin`, `user`.
- Сессии: JWT access + httpOnly refresh cookie.
- API-ключи для интеграций.

### 4.2 Проекты

- Создание проекта: имя, ключ (например `PROJ`), описание, лид, тип проекта (scrum/kanban/simple).
- Редактирование и архивирование проекта.
- Настройки:
  - Типы задач проекта.
  - Статусы и workflow.
  - Компоненты.
  - Версии (fix/affected).
  - Роли и члены проекта.
  - Права доступа.
- Удаление только архивированного проекта.
- Уникальный ключ проекта.

### 4.3 Задачи (issues)

- Поля задачи:
  - Проект, тип, номер (`PROJ-123`).
  - Заголовок, описание (markdown).
  - Статус, приоритет.
  - Исполнитель, репортёр.
  - Метки, компоненты, версии.
  - Эпик, спринт.
  - Story points, time estimate.
  - Даты: created, updated, due.
  - Кастомные поля (text, number, select, multi-select, date, user, checkbox).
- CRUD задач.
- История изменений (audit log по полям).
- Связи: `blocks`, `is blocked by`, `duplicates`, `relates to`, `parent of`, `child of`.
- Подзадачи.
- Клонирование задачи.
- Bulk actions: массовое изменение статуса/исполнителя/меток.

### 4.4 Типы задач

- Системные: `task`, `bug`, `story`, `epic`, `sub-task`.
- Пользовательские типы с иконкой и цветом.

### 4.5 Статусы и workflow

- Категории статусов: `todo`, `in_progress`, `done`.
- Workflow проекта: список статусов и разрешённых переходов.
- Условия перехода: разрешение, назначенный исполнитель, обязательные поля.
- Post-functions: назначить исполнителя, добавить комментарий, отправить уведомление.

### 4.6 Kanban-доска

- Доска по умолчанию из статусов проекта.
- Колонки = статусы.
- Swimlanes: по эпикам, по исполнителю, без swimlanes.
- Перетаскивание задач между колонками.
- Порядок задач в колонке.
- Быстрое создание задачи в колонке.
- Отображение assignee, labels, priority, due date.
- Фильтры по assignee, label, priority, epic, sprint, текст.
- WIP-лимиты на колонки (опционально).
- Realtime обновление.

### 4.7 Scrum-доска и спринты

- Создание спринта: имя, цель, даты.
- Backlog проекта.
- Перемещение задач из бэклога в спринт.
- Начало/завершение спринта.
- Sprint board.
- Burndown chart.
- Velocity.

### 4.8 Эпики

- Тип задачи `epic`.
- Связь `child of`/`parent of` со story/task.
- Прогресс эпика по дочерним задачам.

### 4.9 Комментарии

- Добавление, редактирование, удаление.
- Markdown + @mentions.
- История правок.
- Уведомления подписчикам.

### 4.10 Вложения

- Загрузка файлов к задаче или комментарию.
- Preview для изображений.
- Хранение: локальная FS или S3.
- Ограничение по размеру и MIME.

### 4.11 Поиск и фильтры

- Полнотекстовый поиск по заголовку, описанию, комментариям.
- JQL-подобный язык:
  - `project = PROJ`
  - `status in (Open, "In Progress")`
  - `assignee = currentUser()`
  - `created >= -7d`
  - `labels in (backend, rust)`
  - `sprint in (openSprints())`
- Сохранённые фильтры.
- Подписка на фильтр (уведомления).
- Экспорт результатов в CSV.

### 4.12 Уведомления

- In-app уведомления.
- Email-уведомления.
- События:
  - Назначили задачу.
  - Изменили статус.
  - Новый комментарий.
  - Упоминание `@user`.
  - Приближается due date.
  - Спринт начался/закончился.
- Настройки уведомлений пользователя.

### 4.13 Роли и разрешения

- Глобальные роли: `system_admin`, `user`, `guest`.
- Проектные роли: `project_admin`, `project_lead`, `developer`, `viewer`.
- Проектные схемы разрешений.
- Проверка на уровне API и сервисов.

### 4.14 Администрирование

- Управление пользователями: блокировка, сброс пароля, назначение ролей.
- Управление проектами.
- Глобальные настройки: регистрация, SMTP, внешний вид, retention.
- Audit log: кто и когда изменил критичные сущности.

### 4.15 Кастомные поля

- Типы: text, number, date, select, multi-select, checkbox, user, url.
- Контекст: глобальные / проектные.
- Валидация: обязательность, min/max.

### 4.16 Дашборды (v2)

- Гаджеты: мои задачи, задачи проекта, burndown, статусная диаграмма.
- Личный и проектный dashboard.

## 5. Нефункциональные требования

### 5.1 Производительность

- P95 ответа API: < 200 мс при нагрузке до 100 RPS на 2 CPU / 4 GB.
- Загрузка kanban-доски до 1000 задач: < 1 сек.
- Поиск по 100K задач: < 500 мс.
- Frontend First Contentful Paint: < 1.5 сек.
- WebSocket broadcast до 10K одновременных соединений.

### 5.2 Масштабируемость

- Stateless backend.
- Горизонтальное масштабирование backend за балансировщиком.
- WebSocket — sticky sessions или Redis pub/sub.
- PostgreSQL read-replicas для отчётов.

### 5.3 Надёжность

- Unit + integration тесты.
- E2E на critical path.
- Миграции с откатом.
- Graceful shutdown.
- Circuit breaker для внешних сервисов.

### 5.4 Безопасность

- Argon2id.
- JWT с коротким TTL.
- Rate limiting.
- CORS whitelist.
- Input validation (garde + Zod).
- XSS/CSRF защита.
- File upload sandbox.

### 5.5 UX

- Тёмная тема по умолчанию.
- Russian и English.
- Адаптив: mobile 375×667, desktop 1920×1080, 2K 2560×1440.
- Горячие клавиши: `c` — создать задачу, `/` — поиск, `Esc` — закрыть модалку.
- Offline-индикатор.
- Toast-уведомления.

## 6. API

### 6.1 Общие правила

- REST `/api/v1/...`.
- JSON.
- Ошибки:
  ```json
  {
    "error": {
      "code": "VALIDATION_ERROR",
      "message": "...",
      "details": {}
    }
  }
  ```
- Пагинация: `?limit=&cursor=` для лент, `?limit=&offset=` для таблиц.
- Сортировка: `?sort=created_at:desc`.

### 6.2 Эндпоинты

#### Auth

- `POST /api/v1/auth/register`
- `POST /api/v1/auth/login`
- `POST /api/v1/auth/refresh`
- `POST /api/v1/auth/logout`
- `POST /api/v1/auth/forgot-password`
- `POST /api/v1/auth/reset-password`
- `GET /api/v1/auth/me`

#### Users

- `GET /api/v1/users`
- `GET /api/v1/users/:id`
- `PUT /api/v1/users/:id`
- `PUT /api/v1/users/:id/avatar`

#### Projects

- `GET /api/v1/projects`
- `POST /api/v1/projects`
- `GET /api/v1/projects/:id`
- `PUT /api/v1/projects/:id`
- `DELETE /api/v1/projects/:id`
- `GET /api/v1/projects/:id/members`
- `POST /api/v1/projects/:id/members`
- `DELETE /api/v1/projects/:id/members/:user_id`
- `PUT /api/v1/projects/:id/members/:user_id/role`

#### Issues

- `GET /api/v1/projects/:project_id/issues`
- `POST /api/v1/projects/:project_id/issues`
- `GET /api/v1/issues/:id`
- `PUT /api/v1/issues/:id`
- `DELETE /api/v1/issues/:id`
- `POST /api/v1/issues/:id/clone`
- `POST /api/v1/issues/:id/transitions`
- `GET /api/v1/issues/:id/history`
- `POST /api/v1/issues/:id/watch`
- `POST /api/v1/issues/bulk-update`

#### Comments

- `GET /api/v1/issues/:issue_id/comments`
- `POST /api/v1/issues/:issue_id/comments`
- `PUT /api/v1/comments/:id`
- `DELETE /api/v1/comments/:id`

#### Attachments

- `POST /api/v1/issues/:issue_id/attachments`
- `GET /api/v1/attachments/:id`
- `DELETE /api/v1/attachments/:id`

#### Boards

- `GET /api/v1/projects/:project_id/boards`
- `POST /api/v1/projects/:project_id/boards`
- `GET /api/v1/boards/:id`
- `PUT /api/v1/boards/:id`
- `GET /api/v1/boards/:id/issues`
- `POST /api/v1/boards/:id/columns/:column_id/issues/:issue_id/move`

#### Sprints

- `GET /api/v1/projects/:project_id/sprints`
- `POST /api/v1/projects/:project_id/sprints`
- `POST /api/v1/sprints/:id/start`
- `POST /api/v1/sprints/:id/complete`
- `PUT /api/v1/sprints/:id/issues`

#### Filters

- `GET /api/v1/filters`
- `POST /api/v1/filters`
- `GET /api/v1/filters/:id`
- `PUT /api/v1/filters/:id`
- `DELETE /api/v1/filters/:id`
- `GET /api/v1/filters/:id/results`

#### Search

- `GET /api/v1/search?q=&jql=&...`

#### Notifications

- `GET /api/v1/notifications`
- `PUT /api/v1/notifications/:id/read`
- `PUT /api/v1/notifications/read-all`
- `PUT /api/v1/notifications/settings`

#### Admin

- `GET /api/v1/admin/users`
- `PUT /api/v1/admin/users/:id/status`
- `PUT /api/v1/admin/users/:id/role`
- `GET /api/v1/admin/projects`
- `GET /api/v1/admin/audit-log`

#### OpenAPI

- `GET /api/docs` — Scalar UI.
- `GET /api/openapi.json` — сырой JSON.

### 6.3 WebSocket

- `GET /ws?token=...`
- Каналы: `user:{id}`, `project:{id}`, `issue:{id}`, `board:{id}`, `sprint:{id}`.
- Сообщения:
  ```json
  { "type": "issue.created", "payload": { "id": "...", "project_id": "..." } }
  ```

## 7. Экраны frontend

- `/login`
- `/register`
- `/forgot-password`
- `/reset-password`
- `/projects` — список проектов
- `/projects/new`
- `/projects/:id` — обзор проекта
- `/projects/:id/issues` — список задач
- `/projects/:id/board` — kanban-доска
- `/projects/:id/backlog` — бэклог + спринты
- `/projects/:id/settings/*`
- `/issues/:id` — детали задачи
- `/filters`
- `/filters/:id`
- `/search?jql=...`
- `/notifications`
- `/profile`
- `/admin/*`
- `/dashboard` — личный дашборд

## 8. Данные и хранение

- PostgreSQL 17 — основное хранилище.
- Файлы — локальная FS `/data/attachments` или S3.
- Redis — опционально для сессий, rate limit, WS pub/sub.
- Elasticsearch / pg_search — опционально для полнотекста.

## 9. Этапы разработки

### Этап 1. Каркас (2 недели)

- Rust workspace, Axum, SQLx, PostgreSQL, миграции.
- React + Vite + Tailwind + shadcn/ui.
- Docker Compose.
- CI: cargo test, pnpm typecheck, lint.

### Этап 2. Auth и пользователи (1 неделя)

- Регистрация, логин, JWT, refresh, профиль.
- Frontend формы.

### Этап 3. Проекты (1 неделя)

- CRUD проектов, ключи, члены проекта.

### Этап 4. Задачи и workflow (2 недели)

- CRUD задач, типы, статусы, приоритеты.
- Workflow transitions.
- История изменений.

### Этап 5. Доска и DnD (2 недели)

- Kanban board, realtime.
- Фильтры.

### Этап 6. Комментарии и вложения (1 неделя)

### Этап 7. Поиск и фильтры (1 неделя)

### Этап 8. Роли и разрешения (1 неделя)

### Этап 9. Уведомления и realtime (1 неделя)

### Этап 10. Спринты и эпики (2 недели)

### Этап 11. Admin, dashboard, polish (2 недели)

### Этап 12. Тестирование, performance, документация (2 недели)

## 10. Критерии приёмки

- `docker compose up` поднимает всё.
- Регистрация → создание проекта → создание задачи → перемещение на доске → комментарий — проходит e2e.
- UI скриншоты по трём разрешениям.
- OpenAPI покрывает все публичные эндпоинты.
- Backend unit + integration tests зелёные.
- Lighthouse score ≥ 90 по Performance, Accessibility, Best Practices.

## 11. Что вне первой волны

- LDAP / OAuth / OpenID / SAML.
- CalDAV.
- Импортёры из Jira/Trello/Todoist.
- Desktop/mobile приложения.
- Marketplace / плагины.
- SLA / автоматизация rules / webhooks.
