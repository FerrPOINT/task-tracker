# Техническое задание: Task Tracker

## 1. Цель

Self-hosted таск-трекер для команд, упрощённый аналог Jira. MVP должен покрывать управление проектами, задачами, kanban-доской, фильтрами и уведомлениями.

## 2. Область применения

- Внутренние команды.
- Личные проекты.
- Самостоятельный хостинг без зависимости от облачных сервисов.

## 3. Функциональные требования

### 3.1 Пользователи

- Регистрация по email + пароль.
- Вход / выход.
- Профиль: имя, аватар, email, язык, тёмная/светлая тема.
- Роли в системе: `admin`, `user`.
- В MVP без командных ролей: любой пользователь видит все проекты, но редактирует только свои или назначенные.

### 3.2 Проекты

- Создание проекта: имя, ключ (например `PROJ`), описание, владелец.
- Список проектов.
- Настройки проекта: название, описание, статусы задач, типы задач.
- Удаление проекта только владельцем или админом.

### 3.3 Задачи (issues)

- Создание задачи: проект, тип, название, описание (markdown), статус, приоритет, исполнитель.
- Редактирование задачи.
- Просмотр задачи с историей изменений.
- Удаление.
- Типы задач: `task`, `bug`, `story`, `epic` (MVP — только `task` и `bug`).
- Приоритеты: `lowest`, `low`, `medium`, `high`, `highest`.
- Статусы: настраиваемые в рамках проекта, привязаны к категориям `todo`, `in_progress`, `done`.
- Назначение исполнителя.
- Метки (labels).
- Вложения (файлы).
- Комментарии.
- Связи: `blocks`, `is blocked by`, `duplicates`, `relates to`.

### 3.4 Kanban-доска

- Доска автоматически строится по статусам проекта.
- Колонки = статусы.
- Перетаскивание задачи между колонками меняет статус.
- Порядок задач внутри колонки сохраняется.
- Фильтр по исполнителю, метке, приоритету, текстовый поиск.

### 3.5 Фильтры и поиск

- Сохранённые фильтры пользователя.
- Полнотекстовый поиск по названию и описанию.
- Фильтры: проект, статус, исполнитель, приоритет, метка, создатель, дата.

### 3.6 Уведомления

- In-app уведомления.
- События: назначили задачу, прокомментировали, изменили статус.
- WebSocket push в реальном времени.
- Email-уведомления — опционально, отключаемые.

### 3.7 Администрирование

- Панель админа: список пользователей, проектов.
- Возможность заблокировать пользователя.
- Системные настройки: SMTP, регистрация открыта/закрыта.

## 4. Нефункциональные требования

### 4.1 Производительность

- Время ответа API < 200 мс для 95-го перцентиля на типичных запросах.
- Kanban-доска проекта до 500 задач должна загружаться < 1 сек.

### 4.2 Масштабируемость

- Горизонтальное масштабирование backend за счёт stateless-сервиса.
- WebSocket — sticky sessions или отдельный pub/sub на Redis.

### 4.3 Безопасность

- Пароли хешируются argon2id.
- JWT с коротким access-токеном и refresh-токеном.
- Защита от CSRF на state-changing запросах.
- Rate limit на login/register.
- Загрузка файлов в изолированную директорию / S3 с проверкой расширений.

### 4.4 Надёжность

- Базовые тесты: unit для сервисов, integration для репозиториев, e2e для critical path.
- Миграции базы версионируются и откатываются.
- Логирование в JSON, structured logging.

### 4.5 UX

- Тёмная тема по умолчанию.
- Russian и English языки.
- Адаптивная вёрстка: mobile 375×667, desktop 1920×1080, 2K.
- Горячие клавиши для создания задачи и поиска.

## 5. API (REST + WebSocket)

### 5.1 Эндпоинты (основные)

#### Auth

- `POST /api/v1/auth/register`
- `POST /api/v1/auth/login`
- `POST /api/v1/auth/refresh`
- `POST /api/v1/auth/logout`
- `GET /api/v1/auth/me`

#### Projects

- `GET /api/v1/projects`
- `POST /api/v1/projects`
- `GET /api/v1/projects/:id`
- `PUT /api/v1/projects/:id`
- `DELETE /api/v1/projects/:id`

#### Issues

- `GET /api/v1/projects/:project_id/issues`
- `POST /api/v1/projects/:project_id/issues`
- `GET /api/v1/issues/:id`
- `PUT /api/v1/issues/:id`
- `DELETE /api/v1/issues/:id`
- `POST /api/v1/issues/:id/move` — смена статуса с позицией

#### Board

- `GET /api/v1/projects/:project_id/board` — структура доски + задачи

#### Comments

- `GET /api/v1/issues/:issue_id/comments`
- `POST /api/v1/issues/:issue_id/comments`
- `PUT /api/v1/comments/:id`
- `DELETE /api/v1/comments/:id`

#### Attachments

- `POST /api/v1/issues/:issue_id/attachments`
- `DELETE /api/v1/attachments/:id`

#### Filters

- `GET /api/v1/filters`
- `POST /api/v1/filters`
- `DELETE /api/v1/filters/:id`

#### Notifications

- `GET /api/v1/notifications`
- `PUT /api/v1/notifications/:id/read`
- `PUT /api/v1/notifications/read-all`

#### Admin

- `GET /api/v1/admin/users`
- `PUT /api/v1/admin/users/:id/status`
- `GET /api/v1/admin/projects`

### 5.2 WebSocket

- `GET /ws?token=...`
- Сообщения: JSON `{ "type": "issue.updated", "payload": {...} }`.

### 5.3 OpenAPI

- Спецификация генерируется из кода.
- UI: Scalar по адресу `/api/docs`.

## 6. Экраны frontend

- `/login` — вход
- `/register` — регистрация
- `/projects` — список проектов
- `/projects/:id` — детали проекта + вкладки задач/доска/настройки
- `/projects/:id/board` — kanban
- `/issues/:id` — карточка задачи
- `/filters` — сохранённые фильтры
- `/notifications` — уведомления
- `/admin` — панель администратора
- `/profile` — профиль пользователя

## 7. Данные и хранение

- PostgreSQL 16+ — основное хранилище.
- Файлы — локальная FS или S3-совместимое хранилище.
- Кеш сессий — Redis (опционально).

## 8. Деплой

- Docker Compose для локального запуска.
- Backend + frontend nginx для production.
- Порт по умолчанию: 19876.

## 9. Этапы разработки

### Этап 1. Каркас

- Rust backend: Axum, SQLx, PostgreSQL, миграции, базовый error handling.
- React frontend: Vite, Router, TanStack Query, Zustand, Tailwind, shadcn/ui.
- Docker Compose.

### Этап 2. Auth и пользователи

- Регистрация, логин, JWT, профиль.
- Frontend: формы, валидация Zod.

### Этап 3. Проекты

- CRUD проектов, настройки, ключи.

### Этап 4. Задачи

- CRUD задач, статусы, приоритеты, исполнители, метки.

### Этап 5. Kanban

- Доска, DnD, фильтры, realtime обновления.

### Этап 6. Комментарии и вложения

- Комментарии, markdown, загрузка файлов.

### Этап 7. Фильтры и поиск

- Полнотекстовый поиск, сохранённые фильтры.

### Этап 8. Уведомления и realtime

- WebSocket hub, in-app уведомления.

### Этап 9. Admin и polish

- Панель админа, тёмная тема, i18n, e2e-тесты.

## 10. Критерии приёмки

- `docker compose up` поднимает приложение.
- Пользователь может зарегистрироваться, создать проект, добавить задачу, переместить её на доске.
- UI адаптирован под 375, 1920, 2560.
- API покрыт OpenAPI, frontend покрыт типами.
- Critical path покрыт e2e-тестами.

## 11. Что сознательно вне MVP

- Teams, группы, сложные роли.
- CalDAV.
- Импортёры из сторонних систем.
- OAuth / OpenID / LDAP.
- Time tracking.
- Sprints, epics, story points.
- Webhooks.
- Плагинная система.
- Desktop / mobile приложения.
