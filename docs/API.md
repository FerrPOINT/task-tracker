# API v1 Specification — Task Tracker

## Prepared Analysis reservation

Tracker-only strict API: POST/GET
`/api/v1/issues/{id}/sdlc/analysis-reservation`, POST `/heartbeat`, GET
`/operations/{idempotency_key}`. Frozen routing обязателен; machine mutation
требует отдельный configured scheduler subject, exact `task-tracker:write` и
persisted project ACL. GET: human/project ACL или exact `task-tracker:read`.
Receipt awaiting_admission/dispatch_allowed=false; GET явно показывает expired
и reconciliation_needed. PM unknown, stale CAS/fence и capacity conflict fail-closed.
Replay не продлевает lease. Release/ACK/run endpoints отсутствуют.
Полные DTO и hash semantics: [SDLC_RESERVATION_V1](SDLC_RESERVATION_V1.md).

## SDLC Clarification API

Маршрутизация SDLC проекта реализована как prerequisite под управлением владельца:
policy POST/current/version/operation readback в
`/api/v1/projects/{project_id}/sdlc/routing-policy` и неизменяемый
`/api/v1/issues/{id}/sdlc/routing-snapshot`. Exact confirmation может принять
`expected_routing_policy_version` и атомарно зафиксировать текущую policy.
Изменение доступно только владельцу проекта с Central human session;
service-read/project ACL разрешает наблюдение, не admission. Семь конкретных
agent/config/package/Workflow refs остаются **declared**, native-ready/dispatch false.
Legacy данные не включаются автоматически и не переписываются. Текущий UI не передаёт
opt-in версию. [Точный wire, CAS и ACL](SDLC_ROUTING_V1.md).

Exact owner confirmation now publishes Backlog and queues Analysis/Ready in the
same transaction. The existing confirmation response remains `stage: Backlog`;
current context reports `stage: Analysis`, `waiting_reason: queued_for_analysis`
and no new confirmation permission. New PM commands cannot change this revision.
Same-key/payload replay retains the original receipt; stale revision, changed
payload or a second confirmation key is 409. Reserved PM admission remains gated.

`GET /api/v1/issues/{id}/sdlc/analysis-intent` returns the strict frozen
`AnalysisIntent` (Rust DTO in `backend/domain/src/sdlc.rs`): binding IDs, intent and
confirmation UUIDs, exact revision/hash, `stage: Analysis`, `status: Ready`,
`role: Analyst`, `workflow: hermes-sdlc:analyst`, `mode: analysis`, `scope: business`, cycle 0,
attempt 0, creation time and stable `operation_key: analysis:<confirmation UUID>`.
Central service-read and fresh explicit project access are required; PM grants
cannot read this queue resource. Absent Analysis is 404, retained inconsistency
409. It does not claim capacity, allocate an agent or permit dispatch.
The same transaction emits `analysis.intent_created` after
`requirements.confirmed`; `event_id` equals `intent_id`. Both legacy outbox and
`metadata_v1` carry the new content-free frozen intent. Update consumer event
unions before pickup. Further contracts/gaps: [lifecycle](SDLC_LIFECYCLE_V1.md).

The owner issue-detail SDLC tab now uses these real endpoints and the generated
`Analysis` / `analysis.intent_created` unions. Metadata polling is an explicit
bounded page read, not a scheduler or inbox ACK: `projection=metadata_v1`, decimal
`after`, `limit=100`, `max_bytes=65536` are query parameters. Their OpenAPI query
annotation is corrected and covered by the API schema test. Exact confirmation
is followed by fresh context/intent readback; the Backlog receipt alone never
renders Analysis, assignment or success. [UI wire and freshness contract](SDLC_UI_V1.md).

Analysis claim/heartbeat/assignment ACK endpoints are not implemented. The existing
execution-lease API is PM-specific and does not grant Analyst admission. Missing
Tracker integration of Fleet's new read-only configuration observation, native
admission/acceptance/verified-stop lookup and pre-decomposition Workflow assignment
mapping remain fail-closed; intent readback is not their substitute.

`POST /api/v1/issues/{id}/sdlc/binding` retains the strict two-field `BindCommand`
and owner-session/project ACL boundary, but is explicitly root-only:
`root_task_id == id`. Any other root claim is 422 without writes, including a
same-project issue. No child/decomposition/terminal authority is granted by this
command or generic issue links. Retained non-root aggregates fail closed with 409.
Queued Analysis aggregate updates are DB-fenced until real guarded admission;
historical confirmation replay and readback remain unchanged. See the bounded
[pre-decomposition guard](SDLC_LIFECYCLE_V1.md#pre-decomposition-guard-b-sdlc-01).

`GET /api/v1/sdlc/project-access` returns
`{contract_version:1,tracker_instance_id,project_ids:[UUID]}` with unique IDs sorted
by UUID. Verified Central Auth service read access and an active local shadow
identity matched by central subject are required. Only explicit project ownership
or membership grants access; global admin, public access and the legacy central
bypass do not. The indexed query uses one database snapshot and has no cache, so
membership revocation changes the next response. Missing/inactive identity is 403,
unverified credentials 401, unavailable Central Auth or unconfigured SDLC 503;
other database errors retain the existing server-error contract. An authorized
identity with no projects receives an empty list. Query/body fields cannot expand
this scope.

`GET /api/v1/sdlc/project-directory` is the strict Fleet project selector source:

```typescript
type ProjectDirectory = {
  contract_version: 1;
  tracker_instance_id: string;
  projects: { id: string; key: string; name: string }[];
  next_cursor: string | null;
};
```

All fields, including null `next_cursor`, are required. IDs/cursors are canonical
lowercase hyphenated nonnil UUIDs; no descriptions, counts, roles or other project
metadata are returned. Query accepts only optional `after` (exclusive UUID) and
`limit` (integer 1..100, default 50). Unknown/duplicate query fields are 400;
invalid cursor/bounds are 422. Ordering is ascending project UUID. The repository
reads limit+1 authorized rows in the same SQL snapshot as active central-subject
and explicit owner/member checks, deduplicates owner+member overlap, and truncates
to limit. `next_cursor` is the last returned ID only when a lookahead row exists;
otherwise it is null, including on empty pages. A cursor is not a project lookup
or authorization receipt. Every page/retry rechecks ACL; pages do not share a
retained snapshot. Revocation can remove entries on continuation/replay. Existing
project-access wire and ordinary legacy project listing are unchanged. The same
401/403/503 policy above applies; corrupt projected source is a safe 409, never a
skipped row. Selection does not authorize Draft creation, reservation or admission.

Opt-in endpoints under `/api/v1/issues/{id}/sdlc`, exact Fleet DTOs, machine
grants, provisioning and delivery contract: [CHAT_CLARIFICATION_CONTRACT.md](CHAT_CLARIFICATION_CONTRACT.md).
SDLC requires Central Auth and strict project membership independently of the
legacy Tracker central-auth project bypass. Answer/confirm are owner-session
commands. Questions/revisions are assigned PM commands. Rust types live in
`backend/domain/src/sdlc.rs`; all routes are included in generated OpenAPI.

An assignment-scoped PM PAT cannot use the ordinary legacy API, global SDLC
project-access/directory, owner answers/confirmation, verifier evidence or
assignment/binding operations, even if it also has service read/write scopes.
These requests are 403. Its single canonical grant permits only bound-task
context/input, questions, requirements/revisions/diff, events and lease reads,
question/revision publication, question cancellation and lease claim/heartbeat.
Current assignment subject/grant and explicit project access are rechecked in
the repository; corrupt current assignment ledger yields 409, not historical
access. Authentication failure remains 401 and Central outage 503. No new API
token format, scope, public path or migration is introduced by confinement.

Human saga source: `POST /api/v1/projects/{project_id}/sdlc/drafts` takes only
`{title,description,idempotency_key}` and returns typed `CreatedDraft` (201 new,
200 exact replay, changed payload 409). Browser Central Auth, explicit project
write access and exact central owner apply even on replay. Issue, private binding,
creation ledger and outbox commit together; no PM/run is started. Exact wire and
limits are in the contract above. Ordinary issue POST is not a saga substitute.

`GET /api/v1/projects/{project_id}/sdlc/drafts/operations/{idempotency_key}`
reads unknown creation acceptance before POST replay. Percent-encode the exact
UTF-8 key as one path segment (including `/` as `%2F`). Only a human session
with fresh project access can read its own project/author/key namespace. 200 is
the existing unchanged seven-field `CreatedDraft`, without title/description;
404 means no such command (or project). A retained command with missing/invalid
original entity, binding or input is 409, never false absence. Current task stage
does not rewrite the original creation result. Changed POST payload remains 409.

`POST /api/v1/issues/{id}/sdlc/pm-draft-assignment` accepts strict
`{expected_owner_version,expected_assignment_version,requested_agent_id,idempotency_key}`.
Initial values are `0,null`; selector is a canonical non-nil UUID, not Fleet
verification. Exact owner session/project ACL is required. 201/200 return strict
`PmDraftReservation`: version 1, variant `pm_draft_reserved`, immutable binding,
owner_cas, unchanged five-field assignment, execution `{ordinal,key}` (positive
i64 decimal string, `SDLC-<ordinal>`), input `{snapshot_ref,sha256}`,
`assignment_operation_key`, `admission_state:reserved`, `dispatch_allowed:false`.
`GET` on that path optionally accepts `idempotency_key` and returns
`{contract_version,binding,owner_version,current,operation}` with nullable current
reservation and separate author-scoped historical operation. Historical replay
never installs current authority. All reserved PM writes/legacy assignments are
blocked until later verified admission. Source DTOs: `backend/domain/src/sdlc_pm_draft.rs`.
No admission, actual Fleet agent/config/chat/workspace or dispatch is claimed.

Machine-only `POST/GET /api/v1/issues/{id}/sdlc/pm-draft-execution-lease`
and `POST .../heartbeat` implement a separate persisted ownership lease.
Claim uses `{expected_owner_version,fence,idempotency_key}`; heartbeat adds
`lease_id,expected_lease_version`. TTL is 30s, suggested renewal interval 10s;
PostgreSQL clock after locks and lease-version CAS determine validity. Exact
idempotent replay never renews twice. GET distinguishes live/expired current from
historical operation receipt; expiry is retained, not absence or automatic
reacquire. Same existing exact PM grant and fresh machine/project authorization
apply to every read/replay/renewal. No new scope, admission or run-side effect
authority is introduced; reservation/owner cursor/business gate stay unchanged.
Strict DTOs: `backend/domain/src/sdlc_execution_lease.rs`; complete wire and
generation/recovery restrictions are in the clarification contract above.

`GET /api/v1/issues/{id}/sdlc/pm-draft-input` returns
`{contract_version:1,tracker_instance_id,project_id,task_id,root_task_id,owner_subject,
input:{snapshot_ref,title,description,sha256}}`. It reads the immutable original
creation ledger, with exactly the current context resource ACL, not owner-only
read access. Content hash is SHA-256 of canonical exact UTF-8 title/description
JSON, without idempotency key or normalization. Missing historical input is 409;
mutable issue text is never backfilled. All refs are server-derived; CreatedDraft
wire and business confirmation permissions are unchanged. Source DTOs are
`PmDraftInputResponse`/`PmDraftInput` in `backend/domain/src/sdlc.rs`; the schema is
generated in `openapi/openapi.json`. This is input readback, not PM admission,
assignment CAS, dispatch or runtime delivery.

`GET /api/v1/issues/{id}/sdlc/events?projection=metadata_v1&limit=100&max_bytes=262144`
returns strict metadata references, canonical metadata digest, decimal-string
after/next_after cursors and has_more. Defaults/limits and all ten resource
variants are in the clarification contract. The entire serialized response is
bounded (1024..1048576 bytes), preserving a contiguous task-event prefix and
blocking rather than skipping unsupported/corrupt/oversized events. Typed static
errors distinguish 422 metadata_budget_too_small from 409
metadata_event_unrepresentable/metadata_source_invalid. Legacy default wire,
query behavior, scopes and original content hashes remain unchanged. DTOs live
in `backend/domain/src/sdlc_metadata.rs`; OpenAPI includes both success variants.

## Overview

REST API первой версии Task Tracker. Все endpoint возвращают JSON и используют единую модель пагинации, ошибок и webhook-событий. Real-time обновления через SSE описаны в разделе [Real-time (SSE)](#real-time-sse).

> **Single source of truth:** актуальная OpenAPI-схема лежит в [`openapi/openapi.json`](../openapi/openapi.json). Backend генерирует её из `utoipa`-аннотаций Rust-хендлеров, а фронт получает из неё TypeScript-клиент. Ручная документация ниже — для контекста, но при расхождении приоритет у `openapi/openapi.json`.

## Базовая информация

- Base URL: `https://{host}:3456/api/v1`
- Content-Type: `application/json`
- Auth: Central Auth access token в `Authorization: Bearer <token>`; браузерная
  refresh/session cookie принадлежит Central Auth и остаётся `httpOnly`.
- Версионирование: path-based `/api/v1`.
- Пагинация: `?page=0&size=20&sort=createdAt,desc`
- Фильтр поиска задач: `?jql=...`

## OpenAPI generation

```bash
cd backend
cargo run -p api --bin gen-openapi > ../openapi/openapi.json
cd ../frontend
pnpm generate:api   # writes src/api/generated.ts from openapi/openapi.json
```

---

## Реализованные эндпоинты (v1, автоген из openapi.json)

Ниже — все 70 путей и 99 операций, фактически реализованных в бэкенде (источник: `openapi/openapi.json`, сгенерирован из `utoipa`-аннотаций). Остальные разделы этого документа описывают целевую полную спецификацию (фазы 5+).

### Health

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/health` | Base catalog compatibility alias; same liveness response as `/api/v1/health`. |
| GET | `/api/v1/health` | Current public liveness endpoint; plain `ok`, no dependency-readiness claim. |

### Auth

При заданном `TT_AUTH__CENTRAL_JWKS_URI` браузерный вход выполняется напрямую
через Central Auth Authorization Code + PKCE. Перечисленные ниже локальные
password/register/refresh endpoints являются legacy-контрактом и в центральном
режиме не монтируются; локального fallback при ошибке Central Auth нет.

| Метод | Путь | Назначение |
|---|---|---|
| POST | `/auth/login` | Вход, выдача access и refresh-cookie |
| POST | `/auth/logout` | Выход, отзыв refresh и очистка cookie |
| POST | `/auth/refresh` | Обновление access-токена по refresh-cookie |
| POST | `/auth/register` | Регистрация |
| GET | `/auth/me` | Текущий аутентифицированный пользователь |

### Users

| Метод | Путь | Назначение |
|---|---|---|
| GET | `/users` | Активные пользователи центрального каталога; локальный профиль создаётся по `sub` при необходимости |
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

В платформенном режиме Central Auth эти legacy-записи не ограничивают доступ к
проекту и не определяют список исполнителей: назначение использует активный
центральный каталог. Dashboard не предлагает управление локальными memberships.

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

`POST /projects/{project_key}/labels` и `PUT /labels/{id}` принимают непустой
`name` и `color` в формате `#RRGGBB`. Невалидные значения возвращают `400`.

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

### Export

| Метод | Путь | Назначение |
|---|---|---|
| POST | `/export/csv` | Скачать все доступные активные задачи проекта в CSV |
| POST | `/export/json` | Скачать все доступные активные задачи проекта в JSON |

Оба пути принимают `{ "project_key": "TT" }`, требуют обычный project-access и
возвращают attachment с детерминированным порядком задач (`created_at DESC`).

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

Runtime contract (single source of truth — `backend/shared/src/error.rs`):

```json
{ "error": "not found: issue 0192…", "detail": null, "instance": "/api/v1/issues/…" }
```

- `error` — короткое человекочитаемое сообщение; для 500 всегда `internal server error` (детали только в логах, не в теле ответа).
- `detail` / `instance` — RFC 7807-совместимые поля, зарезервированы и сейчас всегда `null`/путь запроса.
- Структурированные коды ошибок (`code` + `details[]`) в рантайме НЕ отдаются; SDK должен ориентироваться на HTTP status (400/401/403/404/409/429/500) и текст `error`.

---

## Auth

### POST /auth/register

**Body:**
```json
{
  "username": "jdoe",
  "email": "jdoe@example.com",
  "password": "Str0ngP@ss",
  "name": "John Doe"
}
```

**Response 201:**
```json
{
  "access_token": "jwt",
  "token_type": "Bearer",
  "user_id": "uuid",
  "email": "jdoe@example.com",
  "expires_in": 900
}
```

Refresh token не возвращается в JSON, а выставляется через `Set-Cookie` как `httpOnly` cookie.

### POST /auth/login

**Body:**
```json
{
  "email": "jdoe@example.com",
  "password": "Str0ngP@ss"
}
```

**Response 200:** `AuthResponse`, refresh token выставляется через `Set-Cookie`.

### POST /auth/refresh

Refresh берётся из `httpOnly` cookie. Для non-browser клиентов допускается optional body fallback:

```json
{
  "refresh_token": "refresh-token"
}
```

Возвращает новый access token и обновляет refresh cookie. Refresh token не возвращается в JSON.

Клиенты без cookie-jar (CLI) могут передать текущий refresh-токен в теле: `{"refresh_token": "…"}`. Cookie имеет приоритет.

**Response 200:**
```json
{
  "access_token": "jwt",
  "token_type": "Bearer",
  "user_id": "uuid",
  "email": "jdoe@example.com",
  "expires_in": 900
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
  |<-- access_token + Set-Cookie --|
  |                               |
  |--- GET /api/v1/... Bearer --->|
  |<-- 401 expired                |
  |                               |
  |--- POST /auth/refresh Cookie->|
  |<-- new access_token + cookie --|
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

**Response:** `IssueResponse`, включая `original_estimate_seconds`, `remaining_estimate_seconds` и агрегированное `time_spent_seconds`.

### PATCH /issues/{id}

**Body:** partial update разрешённых полей. Nullable поля (`description`, `sprint_id`, `component_id`, `affected_version_id`, `fix_version_id`) очищаются при явном `null` и остаются без изменений при отсутствии ключа.

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

### PATCH /comments/{id}

Updates the body of a comment. The authenticated user must be the comment author.

### DELETE /comments/{id}

Deletes a comment. The authenticated user must be the comment author or a project editor.

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

Multipart-форма с полем `file`. Максимальный размер тела запроса берётся из `TASKTRACKER_STORAGE__MAX_UPLOAD_BYTES` (по умолчанию 25 MiB).
Backend нормализует имя файла, отбрасывает path/control-символы и принимает только whitelist безопасных content-type: `text/plain`, `text/markdown`, `text/csv`, `application/json`, `application/pdf`, `application/zip`, `application/gzip`, `image/png`, `image/jpeg`, `image/gif`, `image/webp`, а также распространённые Office/OpenDocument форматы.

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

### GET /attachments/{id}/download

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

### PATCH /worklogs/{worklogId}

**Body:** `UpdateWorklogRequest` — частичное обновление `started_at`, `duration_seconds`, `description`. Отсутствующий `description` сохраняет текущее значение, явный `null` очищает описание.

**Response 200:** `WorklogResponse`

### DELETE /worklogs/{worklogId}

**Response 204**

Создание, редактирование и удаление worklog пересчитывают агрегаты задачи: `time_spent_seconds` и `remaining_estimate_seconds`.

**Права:** создание/редактирование/удаление worklog доступно пользователям с правом `Work On Issues` для проекта. Просмотр — с правом `View Project`.

---

## Issue Links

### GET /issues/{id}/links

Список связей задачи.
Связи с задачами, которые находятся в корзине, скрываются из обычного списка.

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

**Response 409:** связь уже существует. Для `relates` пара задач считается недирекционной, поэтому обратная связь того же типа тоже конфликтует.

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

By default returns up to 10 unread notifications for the current user, newest first.

Query:
- `include_read` — when `true`, the returned page includes read and unread notifications; default `false`.
- `limit` — page size, `1..50`, default `10`.
- `offset` — page offset, default `0`.

`unread_count` is always the total number of unread notifications for the current user, not the length of the returned page.

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

Эти локальные маршруты удалены. Список, создание и отключение пользователей
выполняются через Admin Panel и Central Auth. Старые клиенты получают `404`;
локальное создание учёток после миграции недоступно.
Это намеренное несовместимое изменение при переходе на единый каталог.

### GET /admin/audit-log

Query: `?actorId=uuid&entityType=issue&from=...&to=...`

### GET /admin/system-settings

### PUT /admin/system-settings

---

## Real-time (SSE)

### GET /events

Server-Sent Events (SSE) — поток событий реального времени для инвалидации клиентского кэша (TanStack Query). В отличие от WebSocket, SSE — однонаправленный поток (server → client) поверх HTTP, без upgrade-хендшейка.

**Content-Type:** `text/event-stream`

**Auth:** access token в `Authorization: Bearer ...`. Query-параметр
`?access_token=` не принимается: секрет не должен попадать в URL, историю или логи.
Browser-клиент использует потоковый `fetch` с заголовком Authorization.

**Подключение:**

```
GET /api/v1/events
Accept: text/event-stream
Authorization: Bearer <access_token>
```

**Формат сообщений:**

Каждое SSE-событие имеет поле `event: tracker` и `data` с JSON-представлением
`TrackerEventPayload`. Поле `type` - runtime discriminator; `event_type` в
этом payload не используется.

```
event: tracker
data: {"type":"issue_created","issue_id":"uuid","project_key":"TT"}

event: tracker
data: {"type":"sprint_changed","project_key":"TT"}
```

### Типы событий

Сервер публикует coarse-grained invalidation events из `TrackerEvent`:

| Event Type | When | Payload Fields |
|------------|------|----------------|
| `issue_created` | Создана задача | `issue_id`, `project_key` |
| `issue_updated` | Изменена задача или связанные данные | `issue_id`, `project_key` |
| `issue_deleted` | Задача отправлена в корзину | `issue_id`, `project_key` |
| `issue_moved` | Задача перемещена по workflow/board | `issue_id`, `project_key` |
| `issue_commented` | Добавлен/изменен/удален комментарий | `issue_id`, `project_key` |
| `worklog_logged` | Изменен worklog задачи | `issue_id`, `project_key` |
| `sprint_changed` | Изменился sprint lifecycle или состав sprint/backlog | `project_key` |
| `notification_created` | Создано уведомление для текущего пользователя | `recipient_id` |

### Client-Side Handling

- Browser и non-browser клиенты используют `Authorization: Bearer ...`; browser helper `connectAuthenticatedEventStream` обрабатывает потоковый `fetch` и переподключение.
- При получении события клиент инвалидирует соответствующие TanStack Query и рефетчит затронутые данные.
- Keep-alive: сервер отправляет SSE ping-сообщения по умолчанию (Axum `KeepAlive::default()`).
- При разрыве соединения helper автоматически переподключается.
- Lagged subscribers (при переполнении broadcast-канала) тихо пропускают пропущенные сообщения и рефетчат данные при следующем событии.

### Пример (JavaScript)

```javascript
import { connectAuthenticatedEventStream } from '@sdlc/ui/lib';

const disconnect = connectAuthenticatedEventStream({
  url: '/api/v1/events',
  token: accessToken,
  eventTypes: ['tracker'],
  onEvent: (_type, event) => {
    if (['issue_created', 'issue_updated', 'issue_deleted', 'issue_moved'].includes(event.type)) {
      queryClient.invalidateQueries({ queryKey: ['projects'] });
      queryClient.invalidateQueries({ queryKey: ['dashboard'] });
      queryClient.invalidateQueries({ queryKey: ['search'] });
      queryClient.invalidateQueries({ queryKey: ['project', event.project_key] });
      queryClient.invalidateQueries({ queryKey: ['backlog', event.project_key] });
      queryClient.invalidateQueries({ queryKey: ['issue', event.issue_id] });
    }
  },
});

// Вызвать disconnect() при закрытии страницы или компонента.
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

## Идентификаторы задач в рабочих CLI-сценариях

GET/PATCH/DELETE `/api/v1/issues/{id}` и POST `/api/v1/issues/{id}/transition`, `/restore` принимают UUID или ключ `PROJ-1`. Разрешение выполняется application service с проверкой доступа к проекту, окончательная проверка права изменения сохраняется в соответствующем сервисе. Обычное чтение не разрешает удалённую задачу; restore использует отдельное чтение, включая soft deleted, и требует право редактирования проекта. Purge остаётся прежним UUID endpoint и не включён в новый CLI-пакет.

Дочерние endpoints (comments, attachments, links, fields, worklogs) сохраняют UUID-контракт; CLI сначала читает задачу по UUID/key и передаёт полученный UUID. Серверные фильтры и страницы используются непосредственно, без клиентской фильтрации списка задач.

## Общая база

Подключение версий, границы контрактов и проверки описаны в [BASE_INTEGRATION](BASE_INTEGRATION.md).
