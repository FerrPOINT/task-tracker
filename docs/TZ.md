# Техническое задание: Task Tracker (Jira-like)

## 1. Цель

Разработать self-hosted таск-трекер с функционалом, сопоставимым с open-source Jira. Система поддерживает проекты, задачи, workflow, kanban/scrum-доски, фильтры, поиск, роли и уведомления.

## 2. Стек (актуальные версии)

- **Backend**: Rust 1.88, Axum 0.8.9, Tokio 1.52.3, SQLx 0.9.0, PostgreSQL 17
- **Frontend**: React 19.1, Vite 6.2, TypeScript 5.9.3, Tailwind 4.1, shadcn/ui
- **Тестирование**: Vitest 4.1.10, Playwright 1.51.1, cargo test, testcontainers 0.27.3

## 3. Функциональные требования

### 3.1 Аутентификация и пользователи
- Регистрация / логин / логаут
- JWT access + httpOnly refresh cookie
- Профиль пользователя, аватар, timezone, язык
- Системные роли: admin, user

### 3.2 Проекты
- CRUD проектов
- Ключ проекта (PROJECT), название, описание, тип (kanban/scrum)
- Архивация / удаление
- Роли внутри проекта: owner, manager, developer, viewer

### 3.3 Типы задач
- Встроенные: Task, Story, Bug, Epic, Sub-task
- Настраиваемые типы с иконками и цветами

### 3.4 Workflow
- Статусы: To Do, In Progress, Done + настраиваемые
- Переходы между статусами с разрешениями
- Workflow может быть глобальным или привязанным к проекту

### 3.5 Задачи (issues)
- Создание, редактирование, удаление
- Поля: summary, description, status, assignee, reporter, priority, labels, due date, estimate, time spent
- Комментарии с rich text
- Вложения (файлы, изображения)
- Связи: blocks/is blocked by, duplicates, relates to
- История изменений (audit log)

### 3.6 Kanban / Scrum доски
- Колонки = статусы
- Drag-and-drop между колонками
- Swimlanes (по assignee, epic, priority)
- WIP limits
- Спринты: start/end, velocity, burndown

### 3.7 Фильтры и поиск
- Сохраняемые фильтры
- JQL-подобный поиск: `project = PROJ AND status = "In Progress" AND assignee = currentUser()`
- Автокомплит
- Полнотекстовый поиск по summary/description/comments

### 3.8 Эпики
- Группировка задач в эпик
- Прогресс эпика

### 3.9 Уведомления
- In-app notifications
- Email notifications (через очередь)
- Правила уведомлений: assign, mention, status change, comment

### 3.10 Разрешения
- Глобальные и проектные permissions
- Примеры: create issue, delete issue, manage project, admin

### 3.11 Dashboard
- Мои задачи
- Задачи, где я assignee / reporter / watcher
- Избранные фильтры

## 4. API endpoints

- `POST   /api/v1/auth/register`
- `POST   /api/v1/auth/login`
- `POST   /api/v1/auth/refresh`
- `POST   /api/v1/auth/logout`
- `GET    /api/v1/me`
- `GET    /api/v1/users`
- `GET/POST/PUT/DELETE /api/v1/projects`
- `GET/POST/PUT/DELETE /api/v1/projects/{id}/members`
- `GET/POST/PUT/DELETE /api/v1/projects/{id}/issue-types`
- `GET/POST/PUT/DELETE /api/v1/projects/{id}/workflows`
- `GET/POST/PUT/DELETE /api/v1/issues`
- `GET    /api/v1/issues/search`
- `POST   /api/v1/issues/{id}/comments`
- `POST   /api/v1/issues/{id}/attachments`
- `GET/POST/PUT/DELETE /api/v1/boards`
- `GET/POST/PUT/DELETE /api/v1/filters`
- `GET/POST/PUT/DELETE /api/v1/sprints`
- `GET/POST/PUT/DELETE /api/v1/epics`
- `GET    /api/v1/notifications`
- `GET    /api/v1/health`, `/ready`, `/metrics`

## 5. Non-functional requirements

- P95 API latency < 200 ms at 100 RPS
- Kanban board with 1000 issues loads < 1 s
- Full-text search < 300 ms on 100k issues
- 99.9% uptime target (single-instance)
- Dark theme by default
- Russian + English i18n

## 6. UI/UX

- Dark theme default
- Responsive: mobile (375px), Full HD (1920px), 2K (2560px)
- Drag-and-drop kanban
- Command palette (Cmd+K)
- Keyboard shortcuts
- Live updates via WebSocket

## 7. Этапы разработки

1. **Каркас**: Rust workspace, Axum, SQLx, Docker Compose, React + Vite
2. **Auth + users**: регистрация, логин, JWT, тесты, скриншоты
3. **Projects + issue types + workflow**
4. **Issues CRUD + comments + attachments**
5. **Kanban board + drag-and-drop**
6. **Filters + JQL search**
7. **Sprints + epics**
8. **Notifications + email queue**
9. **Admin panel + audit log**
10. **Performance optimization + load testing**

## 8. Интеграции

- SMTP для email
- WebDAV/S3 для вложений
- Meilisearch/OpenSearch как опция для поиска
- OpenAPI UI (Scalar)

## 9. Развёртывание

- Docker Compose (app + postgres + redis)
- Порт внутри контейнера: 19876
- Хост mapping: 19875:19876
- Environment prefix: `TASKTRACKER_`
