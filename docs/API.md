# API v1 Specification — Task Tracker

## Overview

REST API первой версии Task Tracker. Все endpoint возвращают JSON и используют единую модель пагинации, ошибок и webhook-событий. Real-time обновления через SSE описаны в разделе [Real-time (SSE)](#real-time-sse).

> **Single source of truth:** актуальная OpenAPI-схема лежит в [`openapi/openapi.json`](../openapi/openapi.json). Backend генерирует её из `utoipa`-аннотаций Rust-хендлеров, а фронт получает из неё TypeScript-клиент. Ручная документация ниже — для контекста, но при расхождении приоритет у `openapi/openapi.json`.

## Базовая информация

- Base URL: `https://{host}:3456/api/v1`
- Content-Type: `application/json`
- Auth: JWT access в `Authorization: Bearer *** refresh в httpOnly cookie.
- Версионирование: path-based `/api/v1`.
- Пагинация: `?page=0&size=20&sort=createdAt,desc`
- Фильтр поиска задач: `?jql=...`

## OpenAPI generation

```bash
cd backend
cargo run --bin openapi-gen -- ../openapi/openapi.json
cd ../frontend
pnpm generate:api   # writes src/api/generated.ts from openapi/openapi.json
```

---

## Реализованные эндпоинты (v1, автоген из openapi.json)

Ниже — все 70 путей и 99 операций, фактически реализованных в бэкенде (источник: `openapi/openapi.json`, сгенерирован из `utoipa`-аннотаций). Остальные разделы этого документа описывают целевую полную спецификацию (фазы 5+).

### Health

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/health` | Health-check (plain text) |

### Auth

| Метод | Путь | Назначение |
|---|---|---|
| POST | `/auth/login` | Вход, выдача access/refresh |
| POST | `/auth/logout` | Выход, отзыв refresh |
| POST | `/auth/refresh` | Обновление access-токена |
| POST | `/auth/register` | Регистрация |
| GET | `/auth/me` | Текущий аутентифицированный пользователь |

### Users

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/users` |  |
| GET | `/users/me` | Текущий пользователь |

### Projects (CRUD)

| Метод | Путь | Назначение |
|---|---|---|
| GET, POST | `/projects` | Список проектов / создание |
| GET, DELETE, PATCH | `/projects/{project_key}` | Проект по ключу / обновление / удаление |

### Projects — board/backlog/sprints/labels (по ключу)

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/projects/{project_key}/backlog` | Бэклог проекта |
| GET | `/projects/{project_key}/board` | Доска проекта |
| POST | `/projects/{project_key}/board/move` | Перемещение задачи по колонкам |
| GET, POST | `/projects/{project_key}/labels` | Метки проекта / создание |
| GET, POST | `/projects/{project_key}/sprints` | Спринты проекта / создание |
| GET, PATCH | `/projects/{project_key}/sprints/{sprint_id}` | Спринт: получение / PATCH / удаление |
| POST | `/projects/{project_key}/sprints/{sprint_id}/close` | Закрытие спринта |
| POST | `/projects/{project_key}/sprints/{sprint_id}/issues` | Перенос задач в спринт |
| POST | `/projects/{project_key}/sprints/{sprint_id}/remove-issue` | Убрать задачу из спринта |

#### Backlog pagination

`GET /projects/{project_key}/backlog?offset=0&limit=100` возвращает детерминированно отсортированное окно (`created_at DESC, id DESC`). `limit` — 1..200 (default 100). Ответ содержит `backlog_total`, `backlog_offset`, `backlog_limit`, `backlog_issues` и `sprint_issues`; используйте метаданные для пагинации, не полагайтесь на старый hard-cap 100.

#### Project owner

`ProjectResponse` включает `owner_id` и `owner_name`; клиент показывает `owner_name`, а `owner_id` оставляет для машинных операций.
| POST | `/projects/{project_key}/sprints/{sprint_id}/start` | Старт спринта |

### Project members (по UUID)

| Метод | Путь | Назначение |
|---|---|---|
| GET, POST | `/projects/{project_key}/members` | Участники проекта / добавление (upsert роли) |
| DELETE | `/projects/{project_key}/members/{user_id}` | Удаление участника |

### Issues

| Метод | Путь | Назначение |
|---|---|---|
| GET, POST | `/issues` | Поиск задач / создание |
| GET, DELETE, PATCH | `/issues/{id}` |  |

### Comments

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/issues/{issue_id}/comments` | Список комментариев задачи |
| POST | `/issues/{issue_id}/comments` | Добавление комментария |
| DELETE, PATCH | `/comments/{id}` | Правка / удаление комментария |

### Attachments

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/issues/{issue_id}/attachments` | Список вложений задачи |
| POST | `/issues/{issue_id}/attachments` | Загрузка вложения (multipart) |
| DELETE | `/attachments/{id}` | Метаданные / удаление вложения |
| GET | `/attachments/{id}/download` | Скачивание файла вложения |

### Labels

| Метод | Путь | Назначение |
|---|---|---|
| PUT, DELETE | `/labels/{id}` | Метка: обновление / удаление |

### Issue Labels

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/issues/{issue_id}/labels` | Список меток задачи |
| POST | `/issues/{issue_id}/labels` | Привязка метки к задаче |
| DELETE | `/issues/{issue_id}/labels/{label_id}` | Отвязка метки от задачи |

### Issue Links

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/issues/{issue_id}/links` | Список связей задачи |
| POST | `/issues/{issue_id}/links` | Создание связи между задачами |
| DELETE | `/issue-links/{id}` | Удаление связи |

### Worklogs

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/issues/{issue_id}/worklogs` | Список записей о затраченном времени |
| POST | `/issues/{issue_id}/worklogs` | Добавление записи о затраченном времени |
| DELETE, PATCH | `/worklogs/{id}` | Правка / удаление записи |

### Issue Watchers

| Метод | Путь | Назначение |
|---|---|---|
| POST | `/issues/{issue_id}/watch` | Подписка на задачу |
| DELETE | `/issues/{issue_id}/watch` | Отписка от задачи |
| GET | `/issues/{issue_id}/watchers` | Список наблюдателей |

### Issue Votes

| Метод | Путь | Назначение |
|---|---|---|
| POST | `/issues/{issue_id}/vote` | Голосование за задачу |
| DELETE | `/issues/{issue_id}/vote` | Снятие голоса |
| GET | `/issues/{issue_id}/votes` | Список проголосовавших |

### Custom Fields

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/projects/{project_key}/custom-fields` | Список кастомных полей проекта |
| POST | `/projects/{project_key}/custom-fields` | Создание кастомного поля |
| PUT | `/custom-fields/{id}` | Обновление кастомного поля |
| DELETE | `/custom-fields/{id}` | Удаление кастомного поля |
| GET | `/issues/{issue_id}/custom-fields` | Значения кастомных полей задачи |
| PUT | `/issues/{issue_id}/custom-fields/{field_id}/value` | Установка значения кастомного поля |

### Components

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/projects/{project_key}/components` | Список компонентов проекта |
| POST | `/projects/{project_key}/components` | Создание компонента |
| PUT | `/projects/{project_key}/components/{component_id}` | Обновление компонента |
| DELETE | `/projects/{project_key}/components/{component_id}` | Удаление компонента |

### Versions

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/projects/{project_key}/versions` | Список версий проекта |
| POST | `/projects/{project_key}/versions` | Создание версии |
| PUT | `/projects/{project_key}/versions/{version_id}` | Обновление версии |
| DELETE | `/projects/{project_key}/versions/{version_id}` | Удаление версии |

### Trash (Soft-delete)

| Метод | Путь | Назначение |
|---|---|---|
| DELETE | `/issues/{id}` | Soft-delete задачи (перемещение в корзину) |
| POST | `/issues/{id}/restore` | Восстановление задачи из корзины |
| DELETE | `/issues/{id}/trash` | Безвозвратное удаление задачи |
| GET | `/projects/{key}/trash` | Список удалённых задач проекта |

### Workflow — Statuses

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/statuses` | Статусы workflow |

### Workflow — Transitions

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/transitions` | Разрешённые переходы workflow |

### Workflow — Issue Types

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/issue-types` | Типы задач |

### Search

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/search` | Глобальный поиск с фильтрами |

### Dashboard

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/dashboard` | Дашборд текущего пользователя |

### Reports

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/reports/velocity` | Отчёт по скорости спринтов |
| GET | `/reports/burndown` | Отчёт по сгоранию задач |
| GET | `/reports/cumulative-flow` | Отчёт по кумулятивному потоку |
| GET | `/reports/control-chart` | Контрольная диаграмма cycle time |

### Real-time (SSE)

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/events` | SSE-поток событий реального времени |

---

## Общие модели

### PaginationResponse<T>

```json
{
  "data": [],
  "page": 0,
  "size": 20,
  "total": 100,
  "totalPages": 5
}
```

### ErrorResponse

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Request validation failed",
    "details": [
      { "field": "summary", "message": "required" }
    ]
  }
}
```

---

## Auth

### POST /auth/register

**Body:**
```json
{
  "username": "jdoe",
  "email": "jdoe@example.com",
  "password": "Str0ngP@ss",
  "displayName": "John Doe"
}
```

**Response 201:**
```json
{
  "id": "uuid",
  "username": "jdoe",
  "email": "jdoe@example.com",
  "displayName": "John Doe",
  "accessToken": "jwt",
  "expiresIn": 900
}
```

Refresh token — httpOnly cookie.

### POST /auth/login

**Body:**
```json
{
  "login": "jdoe", // username or email
  "password": "Str0ngP@ss"
}
```

### POST /auth/refresh

Refresh из `httpOnly` cookie. Возвращает новый access token и обновляет refresh cookie.

**Response 200:**
```json
{
  "accessToken": "jwt",
  "expiresIn": 900,
  "tokenType": "Bearer"
}
```

### POST /auth/logout

Инвалидирует refresh token и очищает cookie.

**Response 204:** No content.

### GET /auth/me

Возвращает текущего аутентифицированного пользователя.

**Response 200:** `UserResponse`

```json
{
  "id": "uuid",
  "username": "jdoe",
  "email": "jdoe@example.com",
  "display_name": "John Doe"
}
```

## Auth Flow

```
Client                          Server
  |                               |
  |--- POST /auth/login --------->|
  |                               | argon2id verify
  |<-- accessToken + Set-Cookie --|
  |                               |
  |--- GET /api/v1/... Bearer --->|
  |<-- 401 expired                |
  |                               |
  |--- POST /auth/refresh Cookie->|
  |<-- new accessToken + cookie --|
```

- Access token TTL: 15 минут.
- Refresh token TTL: 7 дней.
- Refresh cookie: `httpOnly`, `Secure`, `SameSite=Lax`, path `/api/v1/auth`.
- Access token хранится в memory; не в localStorage.

---

## Users

### GET /users/me

```json
{
  "id": "uuid",
  "username": "jdoe",
  "email": "jdoe@example.com",
  "displayName": "John Doe",
  "avatarUrl": null,
  "timezone": "Europe/Moscow",
  "locale": "ru",
  "theme": "dark",
  "isAdmin": false,
  "createdAt": "2026-01-01T00:00:00Z"
}
```

### GET /users

Query: `?q=john&page=0&size=20`

---

## Projects

### GET /projects

Query: `?archived=false&page=0&size=20`

### POST /projects

**Body:**
```json
{
  "key": "TT",
  "name": "Task Tracker",
  "description": "Our internal tracker",
  "projectType": "scrum",
  "leadId": "uuid",
  "defaultAssigneeType": "project_lead"
}
```

### GET /projects/{project_key}

### DELETE /projects/{project_key}

Soft delete / archive.

### GET /projects/{project_key}/members

### POST /projects/{project_key}/members

**Body:** `{ "userId": "uuid", "roleName": "developer" }`

### DELETE /projects/{project_key}/members/{userId}

---

## Issues

### GET /issues

Query parameters:
- `jql` — JQL-строка
- `projectId` — UUID
- `statusId` — UUID
- `assigneeId` — UUID
- `sprintId` — UUID
- `epicId` — UUID
- `page`, `size`, `sort`

**Response:** `PaginationResponse<IssueSummary>`

```json
{
  "id": "uuid",
  "key": "TT-42",
  "self": "https://tasktracker.example.com:3456/api/v1/issues/uuid",
  "projectId": "uuid",
  "projectKey": "TT",
  "issueType": { "id": "uuid", "name": "Task", "iconUrl": "...", "color": "..." },
  "status": { "id": "uuid", "name": "In Progress", "category": "in_progress", "color": "..." },
  "summary": "Implement auth",
  "priority": "high",
  "assignee": { "id": "uuid", "displayName": "...", "avatarUrl": "..." },
  "reporter": { "id": "uuid", "displayName": "..." },
  "labels": ["backend"],
  "dueDate": "2026-02-01",
  "rank": "m/aaa",
  "createdAt": "2026-01-01T00:00:00Z"
}
```

### POST /issues

**Body:**
```json
{
  "projectId": "uuid",
  "issueTypeId": "uuid",
  "summary": "Implement auth",
  "description": { "type": "doc", "content": [...] },
  "priority": "high",
  "assigneeId": "uuid",
  "labels": ["backend"],
  "components": ["uuid"],
  "fixVersionIds": ["uuid"],
  "parentId": "uuid",
  "epicId": "uuid",
  "dueDate": "2026-02-01",
  "originalEstimateSeconds": 3600,
  "customFieldValues": [
    { "customFieldId": "uuid", "valueJsonb": "story points" }
  ]
}
```

### GET /issues/{id}

**Response:** `IssueDetail` с полной историей, связями, кастомными полями.

### PUT /issues/{id}

**Body:** partial update разрешённых полей.

### DELETE /issues/{id}

Soft delete → trash.

### POST /issues/{id}/transition

**Body:**
```json
{
  "transitionId": "uuid",
  "comment": "Moving to review",
  "fields": { "resolution": "Fixed" }
}
```

### POST /issues/{id}/watch

Подписка текущего пользователя на задачу. Если в теле передан `user_id`, подписывается указанный пользователь (требует прав).

**Body (optional):**
```json
{
  "user_id": "uuid"
}
```

**Response 204:** No content.

### DELETE /issues/{id}/watch

Отписка текущего пользователя от задачи.

**Response 204:** No content.

### GET /issues/{id}/watchers

Список наблюдателей задачи.

**Response 200:**
```json
{
  "watchers": [
    {
      "user_id": "uuid",
      "username": "jdoe",
      "display_name": "John Doe"
    }
  ]
}
```

### POST /issues/{id}/vote

Голосование текущего пользователя за задачу.

**Response 201:**
```json
{
  "user_id": "uuid",
  "username": "jdoe",
  "display_name": "John Doe",
  "voted_at": "2026-01-15T10:00:00Z"
}
```

### DELETE /issues/{id}/vote

Снятие голоса текущего пользователя.

**Response 204:** No content.

### GET /issues/{id}/votes

Список проголосовавших пользователей.

**Response 200:**
```json
{
  "votes": [
    {
      "user_id": "uuid",
      "username": "jdoe",
      "display_name": "John Doe",
      "voted_at": "2026-01-15T10:00:00Z"
    }
  ],
  "count": 5
}
```

---

## Issue Labels

### GET /issues/{issue_id}/labels

Список меток, привязанных к задаче.

**Response 200:** `LabelListResponse`

```json
{
  "labels": [
    {
      "id": "uuid",
      "project_id": "uuid",
      "name": "backend",
      "color": "#1b67f2"
    }
  ]
}
```

### POST /issues/{issue_id}/labels

Привязка существующей метки к задаче.

**Body:** `AttachLabelRequest`

```json
{
  "label_id": "uuid"
}
```

**Response 204:** No content.

### DELETE /issues/{issue_id}/labels/{label_id}

Отвязка метки от задачи.

**Response 204:** No content.

---

## Issue Expandable Fields

`GET /api/v1/issues/{id}?expand=changelog,renderedFields,operations,editmeta`

| Expand | Included Data |
|--------|---------------|
| `renderedFields` | HTML/ADF rendered description and comments |
| `operations` | Allowed workflow transitions |
| `editmeta` | Metadata of editable fields per issue type |
| `changelog` | Full history of field changes |
| `versionedRepresentations` | Versioned content snapshots |

## Comments

### GET /issues/{id}/comments

Список комментариев задачи.

**Response 200:** `CommentListResponse`

```json
{
  "comments": [
    {
      "id": "uuid",
      "issue_id": "uuid",
      "author_id": "uuid",
      "author_name": "John Doe",
      "body": "Looks good",
      "created_at": "2026-01-15T10:00:00Z",
      "updated_at": "2026-01-15T10:00:00Z"
    }
  ]
}
```

### POST /issues/{id}/comments

**Body:** `CreateCommentRequest`

```json
{
  "body": "Comment text"
}
```

**Response 201:** `CommentResponse`

```json
{
  "id": "uuid",
  "issue_id": "uuid",
  "author_id": "uuid",
  "author_name": "John Doe",
  "body": "Comment text",
  "created_at": "2026-01-15T10:00:00Z",
  "updated_at": "2026-01-15T10:00:00Z"
}
```

### PUT /issues/{id}/comments/{commentId}

### DELETE /issues/{id}/comments/{commentId}

---

## Attachments

### GET /issues/{id}/attachments

Список вложений задачи.

**Response 200:** `AttachmentListResponse`

```json
{
  "attachments": [
    {
      "id": "uuid",
      "issue_id": "uuid",
      "author_id": "uuid",
      "file_name": "screenshot.png",
      "content_type": "image/png",
      "size_bytes": 102400,
      "created_at": "2026-01-15T10:00:00Z"
    }
  ]
}
```

### POST /issues/{id}/attachments

Multipart-форма с полем `file`.

**Response 201:** `AttachmentResponse`

```json
{
  "id": "uuid",
  "issue_id": "uuid",
  "author_id": "uuid",
  "file_name": "screenshot.png",
  "content_type": "image/png",
  "size_bytes": 102400,
  "created_at": "2026-01-15T10:00:00Z"
}
```

### GET /attachments/{id}

Download/stream.

### DELETE /attachments/{id}

---

## Worklogs

### GET /issues/{id}/worklogs

Список записей о затраченном времени.

**Response 200:** `WorklogListResponse`

```json
{
  "worklogs": [
    {
      "id": "uuid",
      "issue_id": "uuid",
      "author_id": "uuid",
      "author_name": "Ivan",
      "started_at": "2026-01-15T10:00:00Z",
      "duration_seconds": 3600,
      "description": "Implemented login",
      "created_at": "2026-01-15T10:00:00Z",
      "updated_at": "2026-01-15T10:00:00Z"
    }
  ]
}
```

### POST /issues/{id}/worklogs

**Body:** `CreateWorklogRequest`

```json
{
  "started_at": "2026-01-15T10:00:00Z",
  "duration_seconds": 3600,
  "description": "Implemented login"
}
```

**Response 201:** `WorklogResponse`

```json
{
  "id": "uuid",
  "issue_id": "uuid",
  "author_id": "uuid",
  "author_name": "Ivan",
  "started_at": "2026-01-15T10:00:00Z",
  "duration_seconds": 3600,
  "description": "Implemented login",
  "created_at": "2026-01-15T10:00:00Z",
  "updated_at": "2026-01-15T10:00:00Z"
}
```

### PUT /issues/{id}/worklogs/{worklogId}

**Body:** то же, что и POST.

**Response 200:** `WorklogResponse`

### DELETE /issues/{id}/worklogs/{worklogId}

**Response 204**

**Права:** создание/редактирование/удаление worklog доступно пользователям с правом `Work On Issues` для проекта. Просмотр — с правом `View Project`.

---

## Issue Links

### GET /issues/{id}/links

Список связей задачи.

**Response 200:** `IssueLinkListResponse`

```json
{
  "links": [
    {
      "id": "uuid",
      "source_id": "uuid",
      "source_key": "TT-42",
      "target_id": "uuid",
      "target_key": "TT-43",
      "link_type": "blocks"
    }
  ]
}
```

### POST /issues/{id}/links

**Body:** `CreateLinkRequest`

```json
{
  "target_key": "TT-43",
  "link_type": "blocks"
}
```

**Response 201:** `IssueLinkResponse`

```json
{
  "id": "uuid",
  "source_id": "uuid",
  "source_key": "TT-42",
  "target_id": "uuid",
  "target_key": "TT-43",
  "link_type": "blocks"
}
```

### DELETE /issue-links/{id}

---

## Versions

### GET /projects/{project_key}/versions

Список версий проекта.

**Response 200:**
```json
{
  "versions": [
    {
      "id": "uuid",
      "project_id": "uuid",
      "name": "v1.0.0",
      "description": "Initial release",
      "released": false,
      "release_date": "2026-02-01T00:00:00Z",
      "created_at": "2026-01-01T00:00:00Z"
    }
  ]
}
```

### POST /projects/{project_key}/versions

**Body:**
```json
{
  "name": "v1.0.0",
  "description": "Initial release",
  "released": false,
  "release_date": "2026-02-01T00:00:00Z"
}
```

**Response 201:** `VersionResponse`

```json
{
  "id": "uuid",
  "project_id": "uuid",
  "name": "v1.0.0",
  "description": "Initial release",
  "released": false,
  "release_date": "2026-02-01T00:00:00Z",
  "created_at": "2026-01-01T00:00:00Z"
}
```

### PUT /projects/{project_key}/versions/{version_id}

**Body:** то же, что и POST.

**Response 200:** `VersionResponse`

### DELETE /projects/{project_key}/versions/{version_id}

**Response 204**

---

## Components

### GET /projects/{project_key}/components

Список компонентов проекта.

**Response 200:**
```json
{
  "components": [
    {
      "id": "uuid",
      "project_id": "uuid",
      "name": "Backend",
      "description": "Backend services",
      "created_at": "2026-01-01T00:00:00Z"
    }
  ]
}
```

### POST /projects/{project_key}/components

**Body:**
```json
{
  "name": "Backend",
  "description": "Backend services"
}
```

**Response 201:** `ComponentResponse`

```json
{
  "id": "uuid",
  "project_id": "uuid",
  "name": "Backend",
  "description": "Backend services",
  "created_at": "2026-01-01T00:00:00Z"
}
```

### PUT /projects/{project_key}/components/{component_id}

**Body:** то же, что и POST.

**Response 200:** `ComponentResponse`

### DELETE /projects/{project_key}/components/{component_id}

**Response 204**

---

## Custom Fields

### GET /projects/{project_key}/custom-fields

Список кастомных полей проекта.

**Response 200:**
```json
{
  "fields": [
    {
      "id": "uuid",
      "project_id": "uuid",
      "name": "Story Points",
      "field_type": "number",
      "options": [],
      "is_required": false,
      "created_at": "2026-01-01T00:00:00Z"
    }
  ]
}
```

### POST /projects/{project_key}/custom-fields

**Body:**
```json
{
  "name": "Story Points",
  "field_type": "number",
  "options": [],
  "is_required": false
}
```

**Response 201:** `CustomFieldResponse`

```json
{
  "id": "uuid",
  "project_id": "uuid",
  "name": "Story Points",
  "field_type": "number",
  "options": [],
  "is_required": false,
  "created_at": "2026-01-01T00:00:00Z"
}
```

### PUT /custom-fields/{id}

**Body:**
```json
{
  "name": "Story Points",
  "field_type": "number",
  "options": [],
  "is_required": true
}
```

**Response 200:** `CustomFieldResponse`

### DELETE /custom-fields/{id}

**Response 204**

### GET /issues/{issue_id}/custom-fields

Значения кастомных полей задачи.

**Response 200:**
```json
{
  "values": [
    {
      "field_id": "uuid",
      "value": 5
    }
  ]
}
```

### PUT /issues/{issue_id}/custom-fields/{field_id}/value

Установка значения кастомного поля для задачи. `value` — произвольный JSON.

**Body:**
```json
{
  "value": 5
}
```

**Response 204:** No content.

---

## Notifications

All notification endpoints require authentication.

### GET /notifications

Returns up to 10 unread notifications for the current user, newest first.

**Response:** `{ "notifications": [...], "unread_count": 2 }`

### PATCH /notifications/{id}/read

Marks one unread notification as read. The notification must belong to the current user. Returns `204`; malformed IDs return `400`, unavailable/foreign IDs return `404`.

### POST /notifications/read-all

Marks every unread notification for the current user as read. Returns `204`.

### GET /notification-settings

Returns saved preferences or defaults without creating a row: `email_frequency: "immediate"`, empty `disabled_event_types`, and `notify_own_changes: false`.

### PATCH /notification-settings

**Body:**
```json
{
  "email_frequency": "immediate",
  "disabled_event_types": [],
  "notify_own_changes": false
}
```

Allowed email frequencies: `immediate`, `hourly`, `daily`, `never`.

---

## Reports

### GET /reports/velocity

Query: `?projectId=uuid&count=6`

**Response:**
```json
{
  "sprints": [
    { "name": "Sprint 1", "committed": 20, "completed": 18 }
  ]
}
```

### GET /reports/burndown

Query: `?sprintId=uuid&unit=story_points`

### GET /reports/cumulative-flow

Query: `?projectId=uuid&from=...&to=...`

### GET /reports/control-chart

Query: `?projectId=uuid`

**Response 200:** `ControlChartResponse`

```json
{
  "points": [
    {
      "issue_key": "TT-42",
      "cycle_time_days": 3.5
    }
  ]
}
```

---

## Admin

### GET /admin/users

### POST /admin/users

### PUT /admin/users/{id}/status

### GET /admin/audit-log

Query: `?actorId=uuid&entityType=issue&from=...&to=...`

### GET /admin/system-settings

### PUT /admin/system-settings

---

## Real-time (SSE)

### GET /events

Server-Sent Events (SSE) — поток событий реального времени для инвалидации клиентского кэша (TanStack Query). В отличие от WebSocket, SSE — однонаправленный поток (server → client) поверх HTTP, без upgrade-хендшейка.

**Content-Type:** `text/event-stream`

**Auth:** JWT access token в `Authorization: Bearer ...`

**Подключение:**

```
GET /api/v1/events
Accept: text/event-stream
Authorization: Bearer <access_token>
```

**Формат сообщений:**

Каждое SSE-событие имеет поле `event: tracker` и `data` с JSON-представлением `DomainEvent`:

```
event: tracker
data: {"type":"Created","issue_id":"uuid","reporter_id":"uuid"}

event: tracker
data: {"type":"StatusChanged","issue_id":"uuid","from":"uuid","to":"uuid"}
```

### Типы событий

Сервер публикует события из `DomainEvent` (enum с тегом `type`):

| Event Type | When | Payload Fields |
|------------|------|----------------|
| `Created` (IssueEvent) | Создана задача | `issue_id`, `reporter_id` |
| `StatusChanged` (IssueEvent) | Переход workflow | `issue_id`, `from`, `to` |
| `Assigned` (IssueEvent) | Назначение assignee | `issue_id`, `assignee_id` |
| `CommentAdded` (IssueEvent) | Новый комментарий | `issue_id`, `comment_id`, `author_id` |
| `Created` (ProjectEvent) | Создан проект | `project_id`, `owner_id` |

### Client-Side Handling

- Клиент подключается к `GET /api/v1/events` с access token в заголовке `Authorization`.
- При получении события клиент инвалидирует соответствующие TanStack Query и рефетчит затронутые данные.
- Keep-alive: сервер отправляет SSE ping-сообщения по умолчанию (Axum `KeepAlive::default()`).
- При разрыве соединения клиент автоматически переподключается (браузерный `EventSource` API).
- Lagged subscribers (при переполнении broadcast-канала) тихо пропускают пропущенные сообщения и рефетчат данные при следующем событии.

### Пример (JavaScript)

```javascript
const es = new EventSource('/api/v1/events', {
  withCredentials: true, // для httpOnly refresh cookie
});

es.addEventListener('tracker', (e) => {
  const event = JSON.parse(e.data);
  // Инвалидация TanStack Query
  queryClient.invalidateQueries({ queryKey: ['issues'] });
  if (event.type === 'StatusChanged') {
    queryClient.invalidateQueries({ queryKey: ['board'] });
  }
});

es.onerror = () => {
  // EventSource автоматически переподключается
};
```

---

## Trash (Soft-delete)

### DELETE /issues/{id}

Soft-delete задачи — перемещение в корзину. Задача не удаляется физически и может быть восстановлена.

**Response 204**

### POST /issues/{id}/restore

Восстановление задачи из корзины.

**Response 200:** `IssueResponse`

### DELETE /issues/{id}/trash

Безвозвратное (физическое) удаление задачи из корзины.

**Response 204**

### GET /projects/{key}/trash

Список удалённых задач проекта (находящихся в корзине).

**Response 200:** `IssueListResponse`

```json
{
  "issues": [
    {
      "id": "uuid",
      "key": "TT-42",
      "summary": "Implement auth",
      ...
    }
  ]
}
```

---

## Status Codes

| Код | Когда |
|-----|-------|
| 200 | OK |
| 201 | Created |
| 204 | No Content (delete) |
| 400 | Bad Request / validation |
| 401 | Unauthorized |
| 403 | Forbidden (permission) |
| 404 | Not found |
| 409 | Conflict (duplicate key, concurrent update) |
| 422 | Business rule violation (workflow) |
| 429 | Rate limit |
| 500 | Internal error |
## References

- `docs/ARCHITECTURE.md` — общая архитектура backend/frontend.
- `docs/ERROR_HANDLING.md` — формат ошибок и retry-политика.
- `docs/SECURITY.md` — headers, CORS, CSRF, auth flow.
- `docs/API_VERSIONING.md` — политика версионирования и deprecation.
- `docs/API_EDGE_CASES.md` — граничные случаи и поведение в конфликтах.
- `docs/DATA_MODEL.md` — структура базы данных.
- `docs/WORKFLOW.md` — workflow engine.
- `docs/NOTIFICATIONS.md` — события и шаблоны уведомлений.
- `docs/PAGINATION.md` — пагинация, bulk operations, rate limiting headers.
