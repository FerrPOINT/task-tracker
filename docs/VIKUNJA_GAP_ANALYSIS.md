# Отчёт: сравнение Vikunja и task-tracker-next

## Контекст

- Vikunja склонирован в `/opt/dev/vikunja-reference` (40 MB, ~130 миграций).
- task-tracker-next находится в `/opt/dev/task-tracker-next` и на текущий момент содержит только документацию и пустые папки `backend/`, `frontend/`, `cli/`.

---

## 1. Что есть у Vikunja для полноценной работы проектов и задач

### Backend (Go)

| Модуль | Назначение | Файлы |
|--------|-----------|-------|
| **Project** | Проекты, вложенность, архивация, фон, права | `pkg/models/project.go`, `project_permissions.go`, `project_users.go`, `project_view.go`, `project_team.go` |
| **Task** | Задачи, title/due/repeat/priority/position/identifier/cover | `pkg/models/tasks.go`, `tasks_permissions.go` |
| **ProjectView** | Виды: List, Kanban, Gantt, Table + bucket config | `pkg/models/project_view.go` |
| **Bucket / Kanban** | Колонки доски, позиции задач, done-bucket | `pkg/models/kanban.go`, `kanban_task_bucket.go`, `kanban_permissions.go` |
| **TaskPosition** | Сортировка/ранжирование в рамках view | `pkg/models/task_position.go` |
| **TaskCollection + Filter** | Поиск, фильтры, сортировка, пагинация | `pkg/models/task_collection.go`, `task_collection_filter.go`, `task_search.go` |
| **Label + LabelTask** | Метки задач | `pkg/models/label.go`, `label_task.go` |
| **TaskRelation** | Связи задач (parent/sub-task, blocks, duplicates, etc.) | `pkg/models/task_relation.go` |
| **TaskComment** | Комментарии + mentions/quotes | `pkg/models/task_comments.go`, `mentions.go` |
| **TaskAttachment** | Вложения к задачам | `pkg/models/task_attachment.go` |
| **TaskAssignee** | Назначенные пользователи (множественные) | `pkg/models/task_assignees.go` |
| **TaskReminder** | Напоминания | `pkg/models/task_reminder.go` |
| **TimeEntry** | Учёт времени | `pkg/models/time_tracking.go` |
| **SavedFilter** | Сохранённые фильтры | `pkg/models/saved_filters.go` |
| **User + ProjectUser + Team** | Пользователи, члены проектов, команды | `pkg/models/users.go`, `project_users.go`, `teams.go` |
| **Permissions** | Read/Write/Admin на уровне проекта | `pkg/models/permissions.go`, `*_permissions.go` |
| **Notifications** | In-app + email, подписки | `pkg/models/notifications.go` |
| **Subscriptions** | Подписка на задачи/проекты | `pkg/models/subscription.go` |
| **LinkSharing** | Публичные ссылки | `pkg/models/link_sharing.go` |
| **API tokens + Bot users** | Токены, боты | `pkg/models/api_tokens.go`, `bot_users.go` |
| **Webhooks** | Исходящие webhook | `pkg/models/webhooks.go` |
| **Favorites** | Избранные проекты/задачи | `pkg/models/favorites.go` |
| **Reactions** | Реакции на задачи/комментарии | `pkg/models/reaction.go` |
| **Migration** | 130+ XORM-миграций | `pkg/migration/*.go` |
| **Routes** | v1 + v2 (Huma) API | `pkg/routes/api/v1/`, `pkg/routes/api/v2/` |
| **Config** | Полная конфигурация (config-raw.json) | `pkg/config/`, `config-raw.json` |
| **Files** | Хранение вложений | `pkg/files/` |
| **Events/Websocket** | Real-time обновления | `pkg/events/`, `pkg/websocket/` |

### Frontend (Vue.js)

| Модуль | Файлы |
|--------|-------|
| Список проектов | `frontend/src/views/project/ListProjects.vue`, `NewProject.vue` |
| Проект/доска | `frontend/src/views/project/ProjectView.vue`, `helpers/`, `settings/` |
| Задачи | `frontend/src/views/tasks/ShowTasks.vue`, `TaskDetailView.vue` |
| Kanban | `frontend/src/components/project/` |
| Фильтры | `frontend/src/views/filters/` |
| Time tracking | `frontend/src/views/time-tracking/`, `frontend/src/components/time-tracking/` |
| Admin | `frontend/src/views/admin/` |
| User/settings | `frontend/src/views/user/` |
| Services/API | `frontend/src/services/` |
| i18n | `frontend/src/i18n/lang/` |

### DevOps

| Компонент | Наличие |
|-----------|---------|
| Dockerfile | `Dockerfile` (multi-stage node + xgo + scratch) |
| Compose? | Есть `devenv.nix`, в основном README/документация предлагает бинарник; docker-compose официально нет в корне |
| CI/CD | `.github/workflows/` |
| Build tool | `magefile.go` |

---

## 2. Чего не хватает в task-tracker-next

### Критично для запуска и работы проектов/задач

| Чего нет | Почему критично | Что нужно сделать |
|----------|-----------------|-------------------|
| **Backend-код** | Нет ни одной строки Rust-кода | Создать workspace, сервер Axum, конфиг, миграции, репозитории, сервисы, контроллеры |
| **БД миграции** | Пустая папка `backend/` | Написать SQL/SeaORM миграции для `users`, `projects`, `issues`, `statuses`, `issue_types`, `workflows` и всей остальной модели |
| **Frontend-код** | Пустая папка `frontend/` | Инициализировать React + Vite, добавить страницы проектов, задач, kanban-доски |
| **Docker Compose** | Нет инфраструктуры | Создать `docker-compose.yml` с PostgreSQL 17.6, Redis 8.0, backend, frontend dev/prod |
| **Dockerfile** | Нет | Написать Dockerfile для backend (multi-stage cargo build) и отдельно/совместно для frontend |
| **Auth (регистрация/логин/JWT)** | Нет | Реализовать argon2id + JWT access + httpOnly refresh cookie |
| **CRUD проектов** | Нет | API + UI: create/list/get/update/delete/archive |
| **CRUD задач** | Нет | API + UI: create/list/get/update/delete, soft-delete/trash |
| **Kanban board** | Нет | API + UI: колонки, drag & drop, позиции, wip limit |
| **Workflow/status** | Нет | Хотя бы базовые статусы + переходы (To Do → In Progress → Done) |
| **Пользователи/права** | Нет | Регистрация, роли в проекте, проверка permissions |
| **Real-time (WebSocket)** | Нет | Обновления доски при изменении задач |
| **Полнотекстовый поиск** | Нет | PostgreSQL `tsvector` + JQL/simple search |
| **Вложения** | Нет | Upload/download файлов, preview |
| **Комментарии** | Нет | API + UI rich-text комментариев |
| **Уведомления** | Нет | In-app + email при assign/mention/status change |

### Важно, но не блокирует MVP запуск

| Чего нет | Уровень важности |
|----------|-----------------|
| Sprints / Scrum | High (Jira-like) |
| Epics / hierarchy | High |
| Custom fields | Medium |
| Automation rules | Medium |
| Dashboards/gadgets | Medium |
| Saved filters / JQL | High |
| Time tracking reports | Medium |
| Roadmap/Gantt | Medium |
| Import/export | Low |
| OIDC/LDAP/TOTP | Low (локальная auth MVP) |
| CalDAV | Low |
| Desktop/Electron | Low |
| Mobile app | Low |
| API tokens / bot users | Low |
| Link sharing | Low |
| Reactions | Low |

---

## 3. Рекомендуемый порядок для запуска минимальной рабочей версии

### Phase 1: Инфраструктура + каркас
1. `docker-compose.yml` (PostgreSQL + Redis + backend + frontend dev).
2. Rust workspace: `Cargo.toml`, `crates/{api,app,domain,infra,shared,server}`.
3. Frontend scaffold: React 19 + Vite 6 + Tailwind 4 + shadcn/ui.
4. `Dockerfile` для backend + frontend.

### Phase 2: Core domain
5. Миграции: `users`, `projects`, `issue_types`, `statuses`, `workflows`, `issues`, `issue_status_history`, `project_members`.
6. Auth: register/login/JWT/refresh/password hashing.
7. CRUD пользователей.
8. CRUD проектов + members + roles.

### Phase 3: Задачи
9. CRUD issues + issue keys (`PROJ-1`).
10. Soft delete + trash.
11. Assignee.
12. Labels.
13. Comments.
14. Attachments (filesystem/MinIO).

### Phase 4: Board
15. Kanban board + columns + bucket mapping.
16. Task position/rank.
17. Workflow transitions.
18. WebSocket live updates.

### Phase 5: Поиск и фильтры
19. Full-text search.
20. Saved filters.
21. Basic JQL.

### Phase 6: Уведомления и отчёты
22. Email + in-app notifications.
23. Time tracking.
24. Reports (velocity/burndown).

### Phase 7+ Расширения
25. Sprints, epics, custom fields, automation, dashboards, roadmap.

---

## 4. Быстрый факт: разница в моделях

Vikunja — более "todo-list" / Kanban-oriented:
- Task: `title`, `done`, `due_date`, `repeat_after`, `priority` (число), `position`, `percent_done`, `bucket_id`.
- Project: `title`, `identifier`, `hex_color`, `parent_project_id`, `views[]`, `position`.
- ProjectView: виды List/Kanban/Gantt/Table + фильтры.
- Bucket: колонки kanban.
- Права: Read/Write/Admin per project.

task-tracker-next (по документам) — более Jira-oriented:
- Issue: `summary`, `key`, `status_id` (workflow), `priority`, `assignee`, `reporter`, `labels`, `rank`.
- Project: `key` (uppercase), `project_type` (basic/kanban/scrum), `lead`, `scheme bindings`.
- Workflow schemes / issue type schemes / permission schemes / notification schemes.
- Sprints / epics / custom fields / JQL.

---

## 5. Вывод

**task-tracker-next сейчас не может запускаться и не имеет кода.** У Vikunja есть полный production-ready код для todo/kanban-функционала, но он на Go/Vue и сильно отличается от желаемого Rust/React Jira-like стека.

Для запуска и работы проектов/задач в task-tracker-next нужно с нуля реализовать backend/frontend/инфраструктуру. Стартовать стоит с Phase 1–4 (инфраструктура, auth, проекты, задачи, kanban) — это даст минимально рабочий продукт.
## References

- `docs/TZ.md`
- `docs/ARCHITECTURE.md`
