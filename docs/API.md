# API v1 Specification — Task Tracker

## Overview

REST API первой версии Task Tracker. Все endpoint возвращают JSON и используют единую модель пагинации, ошибок и webhook-событий. WebSocket live-updates описаны в разделе 7.

## Базовая информация

- Base URL: `https://{host}:19876/api/v1`
- Content-Type: `application/json`
- Auth: JWT access в `Authorization: Bearer {token}`, refresh в httpOnly cookie.
- Версионирование: path-based `/api/v1`.
- Пагинация: `?page=0&size=20&sort=createdAt,desc`
- Фильтр поиска задач: `?jql=...`

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

### POST /auth/forgot-password

**Body:** `{ "email": "jdoe@example.com" }`

### POST /auth/reset-password

**Body:** `{ "token": "...", "password": "NewP@ss" }`

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

### PUT /users/me

**Body:** partial update displayName, timezone, locale, theme, avatar.

### PUT /users/me/password

**Body:** `{ "currentPassword": "...", "newPassword": "..." }`

### GET /users

Query: `?q=john&page=0&size=20`

### GET /users/{id}

### POST /users/{id}/avatar

multipart/form-data

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

### GET /projects/{id}

### PUT /projects/{id}

### DELETE /projects/{id}

Soft delete / archive.

### GET /projects/{id}/members

### POST /projects/{id}/members

**Body:** `{ "userId": "uuid", "roleName": "developer" }`

### PUT /projects/{id}/members/{userId}

**Body:** `{ "roleName": "manager" }`

### DELETE /projects/{id}/members/{userId}

### GET /projects/{id}/settings

### PUT /projects/{id}/settings

---

## Issue Types

### GET /issue-types

### POST /issue-types

**Body:**
```json
{
  "name": "Hotfix",
  "description": "Production hotfix",
  "iconUrl": "...",
  "color": "#ff0000",
  "isSubtask": false,
  "hierarchyLevel": 1
}
```

### PUT /issue-types/{id}

### DELETE /issue-types/{id}

---

## Workflows

### GET /workflows

### POST /workflows

**Body:**
```json
{
  "name": "Approval Workflow",
  "description": "...",
  "statuses": ["uuid-todo", "uuid-in-progress", "uuid-done"],
  "transitions": [
    {
      "name": "Start Progress",
      "fromStatusId": "uuid-todo",
      "toStatusId": "uuid-in-progress",
      "conditions": [],
      "validators": [],
      "postFunctions": [
        { "type": "set_field", "config": { "field": "assignee", "value": "current_user" } }
      ]
    }
  ]
}
```

### GET /workflows/{id}

### PUT /workflows/{id}

### DELETE /workflows/{id}

### GET /workflows/{id}/transitions

### POST /workflows/{id}/transitions

---

## Schemes

### Workflow Schemes

- `GET /workflow-schemes`
- `POST /workflow-schemes`
- `GET /workflow-schemes/{id}`
- `PUT /workflow-schemes/{id}`
- `DELETE /workflow-schemes/{id}`

### Issue Type Schemes

- `GET /issue-type-schemes`
- `POST /issue-type-schemes`
- `GET /issue-type-schemes/{id}`
- `PUT /issue-type-schemes/{id}`
- `DELETE /issue-type-schemes/{id}`

### Permission Schemes

- `GET /permission-schemes`
- `POST /permission-schemes`
- `GET /permission-schemes/{id}`
- `PUT /permission-schemes/{id}`
- `DELETE /permission-schemes/{id}`

### Notification Schemes

- `GET /notification-schemes`
- `POST /notification-schemes`
- `GET /notification-schemes/{id}`
- `PUT /notification-schemes/{id}`
- `DELETE /notification-schemes/{id}`

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

### POST /issues/{id}/assign

**Body:** `{ "assigneeId": "uuid" }`

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

Toggle watch.

### POST /issues/{id}/vote

Toggle vote.

### GET /issues/{id}/activity

### POST /issues/{id}/clone

### POST /issues/{id}/move

**Body:** `{ "targetProjectId": "uuid" }`

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

### POST /issues/{id}/comments

**Body:**
```json
{
  "body": { "type": "doc", "content": [...] },
  "mentions": ["uuid-user"]
}
```

### PUT /issues/{id}/comments/{commentId}

### DELETE /issues/{id}/comments/{commentId}

---

## Attachments

### GET /issues/{id}/attachments

### POST /issues/{id}/attachments

multipart/form-data

### GET /attachments/{id}

Download/stream.

### DELETE /attachments/{id}

---

## Worklogs

### GET /issues/{id}/worklogs

### POST /issues/{id}/worklogs

**Body:**
```json
{
  "timeSpentSeconds": 3600,
  "remainingEstimateSeconds": 7200,
  "startedAt": "2026-01-15T10:00:00Z",
  "description": "Implemented login"
}
```

### PUT /issues/{id}/worklogs/{worklogId}

### DELETE /issues/{id}/worklogs/{worklogId}

### GET /worklogs/reports

Query: `?projectId=uuid&userId=uuid&from=...&to=...`

---

## Issue Links

### GET /issues/{id}/links

### POST /issues/{id}/links

**Body:**
```json
{
  "targetIssueId": "uuid",
  "linkTypeId": "uuid"
}
```

### DELETE /issue-links/{id}

---

## Boards

### GET /boards

### POST /boards

**Body:**
```json
{
  "projectId": "uuid",
  "name": "Development Board",
  "type": "kanban",
  "filterQuery": "project = TT AND status != Done",
  "swimlaneField": "epic",
  "columns": [
    { "name": "To Do", "statusIds": ["uuid-todo"], "wipLimit": 10 },
    { "name": "In Progress", "statusIds": ["uuid-in-progress"], "wipLimit": 5 },
    { "name": "Done", "statusIds": ["uuid-done"], "wipLimit": null }
  ]
}
```

### GET /boards/{id}

### PUT /boards/{id}

### DELETE /boards/{id}

### GET /boards/{id}/issues

Возвращает задачи, сгруппированные по колонкам.

### PUT /boards/{id}/columns/reorder

---

## Sprints

### GET /sprints

Query: `?projectId=uuid&state=active`

### POST /sprints

**Body:**
```json
{
  "projectId": "uuid",
  "name": "Sprint 1",
  "goal": "Complete MVP",
  "startDate": "2026-01-01",
  "endDate": "2026-01-14"
}
```

### GET /sprints/{id}

### PUT /sprints/{id}

### DELETE /sprints/{id}

### POST /sprints/{id}/start

### POST /sprints/{id}/close

**Body:**
```json
{
  "moveIncompleteToBacklog": true,
  "nextSprintId": "uuid"
}
```

### POST /sprints/{id}/issues

**Body:**
```json
{
  "issueIds": ["uuid"],
  "action": "add" // or "remove"
}
```

### GET /sprints/{id}/burndown

---

## Versions

### GET /projects/{id}/versions

### POST /projects/{id}/versions

**Body:** `{ "name": "v1.0.0", "releaseDate": "2026-02-01" }`

### PUT /versions/{id}

### DELETE /versions/{id}

### POST /versions/{id}/release

### POST /versions/{id}/archive

---

## Components

### GET /projects/{id}/components

### POST /projects/{id}/components

**Body:** `{ "name": "Backend", "leadId": "uuid", "defaultAssigneeId": "uuid" }`

### PUT /components/{id}

### DELETE /components/{id}

---

## Custom Fields

### GET /custom-fields

### POST /custom-fields

**Body:**
```json
{
  "name": "Story Points",
  "fieldType": "number",
  "defaultValue": null,
  "contexts": [
    {
      "name": "Default",
      "projectIds": ["uuid"],
      "issueTypeIds": ["uuid-task", "uuid-story"]
    }
  ]
}
```

### PUT /custom-fields/{id}

### DELETE /custom-fields/{id}

### GET /custom-fields/{id}/options

### POST /custom-fields/{id}/options

---

## Filters (JQL)

### GET /filters

### POST /filters

**Body:**
```json
{
  "name": "My open tasks",
  "jql": "assignee = currentUser() AND status != Done",
  "isPublic": false
}
```

### GET /filters/{id}

### PUT /filters/{id}

### DELETE /filters/{id}

### POST /filters/{id}/execute

Query: `?page=0&size=20`

---

## Dashboards

### GET /dashboards

### POST /dashboards

**Body:**
```json
{
  "name": "Team Dashboard",
  "layout": { "columns": 2, "rows": [] },
  "gadgets": [
    {
      "gadgetType": "filter_results",
      "title": "Open bugs",
      "position": { "x": 0, "y": 0, "w": 2, "h": 2 },
      "config": { "filterId": "uuid", "columns": ["key", "summary", "status", "assignee"] }
    }
  ]
}
```

### GET /dashboards/{id}

### PUT /dashboards/{id}

### DELETE /dashboards/{id}

---

## Notifications

### GET /notifications

Query: `?unreadOnly=true&page=0&size=20`

### PUT /notifications/{id}/read

### PUT /notifications/read-all

### GET /notifications/settings

### PUT /notifications/settings

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

### GET /reports/time-tracking

Query: `?projectId=uuid&from=...&to=...&groupBy=user`

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

## WebSocket

### Handshake

`GET /ws/v1/connect` — upgrade to WebSocket.

### Subscribe

После установления соединения клиент отправляет сообщение с подпиской:

```json
{
  "type": "subscribe",
  "topics": ["project:{project_id}", "board:{board_id}", "user:{user_id}:notifications", "issue:{issue_id}"]
}
```

Ответ подтверждения:

```json
{
  "type": "subscribed",
  "topics": ["project:{project_id}", "board:{board_id}", "user:{user_id}:notifications", "issue:{issue_id}"]
}
```

### Отписка

```json
{
  "type": "unsubscribe",
  "topics": ["board:{board_id}"]
}
```

### Heartbeat

```json
{
  "type": "ping",
  "timestamp": "2026-07-13T12:00:00Z"
}
```

Сервер отвечает:

```json
{
  "type": "pong",
  "timestamp": "2026-07-13T12:00:00Z"
}
```

### События от сервера

#### issue_created
```json
{
  "type": "issue_created",
  "topic": "project:{project_id}",
  "payload": {
    "id": "uuid",
    "key": "PROJ-123",
    "summary": "[REDACTED]",
    "projectId": "uuid",
    "issueType": { "id": "uuid", "name": "Task" },
    "status": { "id": "uuid", "name": "To Do" },
    "priority": { "id": "uuid", "name": "Medium" },
    "assigneeId": "uuid",
    "reporterId": "uuid",
    "createdAt": "2026-07-13T12:00:00Z"
  }
}
```

#### issue_updated
```json
{
  "type": "issue_updated",
  "topic": "issue:{issue_id}",
  "payload": {
    "id": "uuid",
    "key": "PROJ-123",
    "changedFields": ["status", "assignee"],
    "changelog": [
      { "field": "status", "from": "To Do", "to": "In Progress", "actorId": "uuid", "at": "2026-07-13T12:00:00Z" },
      { "field": "assignee", "from": "uuid", "to": "uuid", "actorId": "uuid", "at": "2026-07-13T12:00:01Z" }
    ]
  }
}
```

#### issue_deleted
```json
{
  "type": "issue_deleted",
  "topic": "project:{project_id}",
  "payload": { "id": "uuid", "key": "PROJ-123" }
}
```

#### issue_commented
```json
{
  "type": "issue_commented",
  "topic": "issue:{issue_id}",
  "payload": {
    "commentId": "uuid",
    "issueId": "uuid",
    "issueKey": "PROJ-123",
    "authorId": "uuid",
    "body": "[REDACTED]",
    "createdAt": "2026-07-13T12:00:00Z"
  }
}
```

#### board_updated
```json
{
  "type": "board_updated",
  "topic": "board:{board_id}",
  "payload": {
    "boardId": "uuid",
    "changed": ["columns", "wip_limits"],
    "byUserId": "uuid"
  }
}
```

#### sprint_started / sprint_completed
```json
{
  "type": "sprint_started",
  "topic": "project:{project_id}",
  "payload": {
    "sprintId": "uuid",
    "name": "Sprint 8",
    "startDate": "2026-07-13",
    "endDate": "2026-07-27"
  }
}
```

#### notification
```json
{
  "type": "notification",
  "topic": "user:{user_id}:notifications",
  "payload": {
    "notificationId": "uuid",
    "event": "issue_assigned",
    "title": "Задача PROJ-123 назначена вам",
    "body": "...",
    "link": "/issues/PROJ-123",
    "read": false,
    "createdAt": "2026-07-13T12:00:00Z"
  }
}
```

### Ошибки WebSocket

```json
{
  "type": "error",
  "code": "AUTH_REQUIRED",
  "message": "Access token missing or expired"
}
```

Коды ошибок:

| Код | Описание |
|-----|----------|
| `AUTH_REQUIRED` | Токен отсутствует/истёк |
| `TOPIC_NOT_FOUND` | Топик не существует или нет доступа |
| `RATE_LIMITED` | Слишком много сообщений |
| `INVALID_MESSAGE` | Невалидный формат сообщения |

### Client-Side Handling

- Подключение к `/ws/v1/connect` с access token в query `?token=...` или header `Authorization: Bearer`.
- Автоматическое переподключение с exponential backoff (max 30s).
- Повторная подписка на сохранённые топики после reconnect.
- `ping` каждые 30 секунд для keepalive.

---

## Import / Export

### POST /import/csv

multipart/form-data

### POST /import/json

### POST /export/csv

**Body:** `{ "jql": "project = TT" }`

### POST /export/json

---

## Trash

### GET /trash

Query: `?projectId=uuid&page=0&size=20`

### POST /trash/{issueId}/restore

### DELETE /trash/{issueId}

Hard delete.

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
## 11. References

- `docs/ARCHITECTURE.md` — общая архитектура backend/frontend.
- `docs/ERROR_HANDLING.md` — формат ошибок и retry-политика.
- `docs/SECURITY.md` — headers, CORS, CSRF, auth flow.
- `docs/API.md` — детали endpoints аутентификации.
- `docs/DATA_MODEL.md` — структура базы данных.
- `docs/JQL.md` — синтаксис поиска.
- `docs/WORKFLOW.md` — workflow engine.
- `docs/NOTIFICATIONS.md` — события и шаблоны уведомлений.
- `docs/SECURITY.md` — headers, CORS, CSRF.
