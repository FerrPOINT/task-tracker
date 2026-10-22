# Архитектура Task Tracker

Initial PM bootstrap retry reads a composite witness from the EXISTING Fleet
reader outside SQL, then rechecks task/lease/authorization under locks. Fleet,
not Tracker, reads Workflow. Original-key recovery is ledger-first; no public
boolean, fake checkpoint, new execution/owner or direct Workflow credential.
[Source-only boundary](contracts/pm-bootstrap-technical-retry.md).

Bootstrap callback candidate cf5a57e0 uses a request-local180s deadline rather
than the ordinary10s configuration-reader budget. The native/Workflow witness
must complete before the existing atomic lease transaction; no transaction spans
HTTP. Ordinary configuration and unsubmitted reads retain10s/connect3s, with
unchanged credentials/64KiB/JSON/redirect restrictions. Producer schema, exact-key
replay, fencing, lease TTL and history are unchanged. Source checks and live PM
acceptance remain separate gates.

Project counters читают сохранённые `statuses.category` без учёта ASCII-регистра:
Debug-формат записи `Todo`/`InProgress`/`Done`, wire-формат и прежнее
`in_progress` дают соответствующую категорию как в status mapping, так и в batch
aggregation. SQL-группы обоих написаний суммируются, неизвестная категория в
batch остаётся ошибкой. Это исправление read projection без переписывания
статусов, issue/history, миграций или HTTP/DTO-контракта.

Привязка исходников сохраняет существующие controller/service/repository и
транзакцию подтверждения требований. Владелец проекта утверждает проверенный
источник, Draft выбирает immutable revision, а confirmation фиксирует точный
binding. Reader Forge использует существующую зарегистрированную границу;
метаданные каталога и hash receipt не предоставляют Git-write authority.
Explicit enrollment прежнего Draft не меняет PM lease, задачу или чат.
[Контракт источника](SDLC_TASK_SOURCE_BINDING.md).

Current critical PM review source uses the existing command/lease/resume/completion
transactions, not a second scheduler. A journal-validated current review may read
its exact frozen approved baseline while PM edits a newer revision; technical
`exact_revision` checks stay strict. Owner confirmation alone remains Draft and
creates no Analysis intent. Only actual trusted PM completion atomically resolves
the blocker and publishes an append-only routing generation and Analysis intent.
No current-project-policy substitution, historical receipt overwrite or legacy
Analysis fallback is introduced. Tracker0fb passes the bounded compile,
PostgreSQL, repeated-cycle and populated-history checks; this is not accepted
stand installation or native/live SDLC acceptance.
[Qualified scope](CRITICAL_BLOCKER_VERIFICATION.md#latest-qualified-source-15-october).
[Persistence boundary](contracts/critical-pm-lifecycle-persistence.md).

General Delivery Reserve owns execution display-key allocation using PM's existing
identity sequence. All new-profile roles, including Analyst, receive immutable
`execution:{ordinal,key}` in assignment/hash and lifecycle readback. Allocation is
after replay/CAS/capacity checks and before the one transactional persistence/outbox
boundary; failed transactions may consume unused ordinals. Fleet never synthesizes
an execution key from task/chat identity. Missing execution on a historical receipt
is a technical dispatch blocker, not authority to invent one. Legacy Analysis's
separate sequence and Workflow namespace remain unchanged; no global historical
display-key uniqueness, stopped-writer cutover or high-water repair is attempted.
Stable canonical execution identity is Tracker instance plus execution UUID.

New single-task enrollment is explicit at Draft creation, not inferred from a
project flag or an existing PM reservation. The immutable `flow:"single-task-v1"`
marker stays in canonical TaskState's retained JSON; Context/CreatedDraft do not
grow new fields. Creation checks current declared v2 policy under the existing
project lock only after original replay. Actual PM readiness belongs to Fleet's
existing Draft SDK path, outside Tracker's transaction; Tracker never forces
configuration or treats policy declarations as actual readiness.

Owner confirmation freezes current project routing under the same project lock
as consent/persistence. Optional caller version is an extra CAS, not the opt-in
switch. The original confirmation command hash is unchanged when it is omitted;
the selected policy is retained in the immutable snapshot, never retroactively
substituted on replay. SQL protects marker shape/origin/immutability and requires
the exact v2 snapshot before a marked task leaves Draft. Existing initial delivery
projection and new assignment inserts reject absent marker without rewriting
historical executions or interfering with their terminal/readback paths.

Новая opt-in вертикаль одной задачи сохраняет цепочку controller/service/repository.
Agent proposal не равен stage completion: Fleet после actual terminal передаёт
guarded Complete, а Tracker владеет переходом и capacity. Второй планировщик
не добавляется. [Single-task integration](SINGLE_TASK_DELIVERY_INTEGRATION.md).

Single-task producer source retains one task UUID through all roles, with
scheduler-only terminal acceptance/reconciliation and ordinary assignment-scoped
agent proposals. SDLC project access always requires active local ownership or
explicit membership; global human hints do not expand the SDLC directory. New
queue/reserve is project-opt-in after original-operation replay. Tracker adds no
runtime scheduler or Hermes proof protocol; parent integrates existing routes.
[Integration boundary and trusted PM resume](TRACKER_SINGLE_TASK_HANDOFF.md).

PM resume reuses the current execution/assignment/checkpoint and lease generation,
with a new runtime run owned by Fleet, not a new Tracker scheduler. Only the configured
reservation scheduler's separate write-only child credential can consume a real
owner answer/feedback/confirmation and CAS-renew that retained lease after expiry.
Before command submission, Fleet reads actual Workflow checkpoint and old runtime
terminal through its existing credentialed readback/callback path. Tracker trusts
that configured producer boundary (as Lifecycle Complete), checks local identity,
ACL, current requirements and immutable input/dedup, and never treats an artifact
reference/hash alone as independently verified terminal proof. No new outbound
Fleet client/token or agent terminal/admission protocol is introduced. Ordinary
claim/heartbeat, historical replay semantics and PM business-write freeze remain.

Fleet continuation reads actual immutable owner feedback by message UUID using
the existing PM holder's read-only assignment capability and fresh project ACL.
It never impersonates the owner or looks up a human idempotency key. Tracker
rechecks the saved owner/identities/hash against the current assignment fence;
history is independent of lease expiry and current requirements revision. This
read has no dispatch, lease, outbox or input-consumption effect and needs no new
ledger/schema. Human feedback writes/operation reads keep their separate boundary.
Inline-создание проекта записывает подтверждённый ответ в query cache первой
страницы каталога и инвалидирует чтение. Форма задачи не держит отдельную
постоянную копию проекта: свежий серверный каталог обновляет имя и доступность.
Namespace остаётся авторитетным для явно привязанного проектного контекста.

Reserved PM producer commands use Tracker reservation/lease recheck and atomic
command/history write. No Fleet admission HTTP call is made on the producer
path. [PM producer lease](PM_NATIVE_ADMISSION.md).

[Reader проектов для Forge](NAMESPACE_PROJECT_READER.md) возвращает только подтверждённые Namespace-проекты через отдельную machine-only границу. Reader не создаёт human identity и не пересылает пользовательский PAT.

[Namespace context](NAMESPACE.md) разрешает Project по confirmed local binding.
Counter и lifecycle guards принадлежат Tracker; каталог людей следует общей
trusted policy, а machine scopes и owner-only SDLC confirmations сохраняются.

Prepared Analysis configuration preflight uses `api -> SdlcService ->
FleetConfigurationReader` and the existing authorized reservation repository.
The reader uses operator credentials only, bounded HTTP and Fleet's real owner
DTO. Two repository reads bracket HTTP, so no transaction/ACL lock is held while
waiting on Fleet, and expiry or authorization change rejects the result.
No durable admission, dispatch or caller-provided proof is introduced.
[Contract](SDLC_CONFIGURATION_PREFLIGHT_V1.md).

## Tracker-only Analysis preparation

Слои `api/routes/sdlc_reservation -> SdlcService -> SdlcRepository -> infra/sdlc_reservation`.
Tracker — единственный assignment producer; новый background scheduler, remote
dispatch или shared schema не вводятся. Одна PG-транзакция фиксирует exact
intent/snapshot, root/agent/pool capacity, immutable identity/hash/fence, lease,
operation receipt и typed durable outbox. Lease head отдельно от immutable
assignment. Readback expiry и original replay не меняют head. Unknown удерживает
capacity до будущего trusted stop counterpart; любой прежний PM unknown блокирует
reserve. Native admission/Workflow acceptance остаются fail-closed.
[Owner контракт](SDLC_RESERVATION_V1.md).

## SDLC Clarification Slice

`api/routes/sdlc_routing -> SdlcService -> SdlcRepository -> infra/sdlc_routing`
использует тот же строгий Central layer и pending 000034. Изменение policy сразу
блокирует проект FOR UPDATE после проверки активного пользователя, без повышения
shared project lock. Изменение принадлежит существующему владельцу проекта, а не
пользователю с admin/member признаком. Публикация уже удерживает shared project
authorization lock, поэтому обновление policy сериализуется с выбором exact-head
snapshot до commit. Неизменяемые policy revisions/receipts и полные task snapshots
отделяют текущие defaults проекта от исторической маршрутизации задачи.
Remote calls и второй scheduler не добавлены. Входные refs являются объявлениями
владельца, а не Fleet observation/native admission evidence. Без opt-in legacy
confirmation hashes сохраняют побайтовую совместимость.
[Полномочия, конкурентность и следующие интерфейсы](SDLC_ROUTING_V1.md).

`api/routes/sdlc -> app::sdlc::SdlcService -> domain::sdlc::SdlcRepository ->
infra::sdlc::PostgresSdlcRepository` is wired through `AppContext.sdlc` only
when a stable instance ID is configured. Its separate Central Auth route layer
never uses local JWT fallback or legacy project bypass. Repository authorization
resolves active users by central subject and locks project/member authorization
rows together with issue/task state. Global admin roles confer no SDLC access.
Requirements hash covers the full typed document with canonical JSON. Questions
invalidate readiness until PM publishes a new revision; independent verifier
evidence plus exact owner-session consent atomically publish Backlog/Ready,
without queuing Analysis. The existing terminal/Workflow PM completion ledger
remains separate and mandatory for accepted PM completion.
The issue gate is also enforced at PostgreSQL so legacy mutations cannot bypass
it. Durable pull outbox separates Tracker persistence from Workflow resume.
Fleet gateway, inbox/projection, Workflow fencing and the real verifier remain
external integration responsibilities. See [contract](CHAT_CLARIFICATION_CONTRACT.md).

PM assignment grants have a separate request boundary before handlers. The legacy
router rejects them before linking a Central identity or converting it to local
UserClaims. The strict router preserves the Principal, accepts one canonical
task grant, and enumerates allowed GET/POST resources for that task using the
original full URI. Owner answers/confirmation, global selectors, verifier and
assignment/binding operations are not PM capabilities. Allowed reads reuse the
issue/project/aggregate lock order, then verify the current assignment subject,
exact grant and latest append-only assignment payload/version. The request path
alone is not authority. Generic human/service PAT policy is unchanged; this is
not global retirement of the ordinary central-mode bypass.

Human Draft creation uses a project/central-subject/key advisory transaction
lock with canonical JSON namespace encoding. It authorizes after the lock,
rechecks immutable ownership on replay, and commits issue/binding/ledger/outbox
atomically. Hash collisions can only serialize unrelated requests; exact ledger
keys/payload hashes still determine identity. Number allocation matches ordinary
issue MAX-suffix allocation; only PostgreSQL unique violations on `issues_key_key`
restart the entire transaction (five attempts). No independent number sequence
or ordinary non-idempotent issue POST is used for the Fleet creation saga.

Creation also freezes the exact original title/description and server snapshot
UUID in its append-only ledger, with a separate canonical UTF-8 content hash.
PM draft input readback follows service -> repository and reuses the same
`load`/authorization/lock order as context before selecting the indexed ledger.
It verifies snapshot hash/binding and never reconstructs missing input from
mutable issue fields. Existing CreatedDraft and owner confirmation wire remain
unchanged. Workflow project mapping/admission remain separate integration work.
Initial owner-CAS reservation now allocates Tracker-owned assignment/execution
UUIDs and a durable SDLC ordinal in one transaction, but explicitly cannot
dispatch. Selector identity is not actual Fleet verification. Reserved enrollment
blocks all PM business writes and legacy allocator bypass; exact historical
replay is read-only. The creation-operation GET shares creation's advisory lock,
fresh ACL and exact author namespace, validates retained entity/binding/input and
returns only the original CreatedDraft. See the bounded contract for full future
admission, replacement quiescence and resume responsibilities. Opt-in metadata outbox
projects immutable events and historical references without emitting content.
The separate execution ownership lease reuses strict Central auth and the exact
persisted PM grant, project/aggregate locks and immutable reservation/input
checks. It locks one lease cursor, then reads PostgreSQL clock_timestamp; an
append-only operation receipt and renewal cursor commit in one transaction.
Historical replay is never renewal/current authority. No lease deletion,
replacement/reacquire, admission or runtime side effect is enabled; reservation
JSON, owner cursor and reserved application/DB business gates remain unchanged.
The metadata serialized whole envelope is byte-bounded; legacy outbox remains unchanged.
Fleet must pin the projection before consumption and persist its own inbox/cursor.

Read-only project scope uses the same strict route layer but one indexed SQL
snapshot instead of per-project authorization requests. The repository resolves
the active central-subject identity and unions explicit ownership/membership,
returning sorted unique IDs for the configured Tracker instance. The membership
index is part of pending 000034. No cache is used; this read scope does not replace
transactional authorization rechecks on SDLC write routes.
The additive `project-directory` route follows the same controller/service/repo
path and strict auth. Its one statement combines active subject, deduplicated
owner/member IDs and UUID keyset limit+1 read. Only project ID/key/name are
projected; the bounded page emits the last returned ID iff lookahead exists,
otherwise required null. No counts, separate ACL read, retained page snapshot or
scope cache is introduced. Every continuation rechecks access; selector results
do not replace write/admission checks. Rust DTOs/route annotations own the schema.

Target autonomous SDLC: [Task lifecycle v1](SDLC_LIFECYCLE_V1.md). Owner — Tracker;
документ отделяет реализованные срезы от дальнейших target capabilities.
Описание target capabilities само по себе не активирует новые API или миграции.

Before Architect admission exists, the root-only identity guard is enforced by
`TaskState::require_root`, repository load/binding and pending 000034. All existing
PM business commands and reads reject non-root aggregates. The binding endpoint
cannot provision independent PM flows for claimed delivery children. A queued
Analysis row is frozen by a separate DB trigger; rollback to Backlog cannot bypass
the generic issue status gate. Fresh ACL, existing issue/aggregate lock order,
outbox and historical replay remain unchanged. This is a fail-closed lifecycle
guard, not a decomposition graph, accepted terminal protocol or root barrier.

Exact initial confirmation preserves its Backlog receipt and publishes the current
aggregate as Backlog/Ready. The existing task lock and replay ledger serialize
confirmation; confirmed requirements reject new PM business commands. Routing and
source snapshots still freeze in the consent transaction, together with issue
projection/history, receipt and `requirements.confirmed`. There is no automatic
Analysis intent or `analysis.intent_created`; an outbox failure rolls back consent.
The separate PM completion keeps its exact terminal/Workflow refs and lease/fence
gates. Neither completion nor replay queues Analysis. Historical queued Analysis
readback/admission and critical-review generation handling are unchanged; their
tests seed deliberate historical Analysis fixtures, not a new promotion API.

## 1. Контекст

Self-hosted таск-трекер (Jira-like). MVP покрывает проекты, канбан-доску, бэклог, поиск, дашборд, создание задач и JWT-аутентификацию.

Связь frontend ↔ backend реализована через OpenAPI-first: `openapi/openapi.json` генерируется из Rust-кода, TypeScript-клиент `frontend/src/api/generated.ts` обновляется командой `pnpm generate:api`, запросы идут через `openapi-fetch`, состояния кешируются через `@tanstack/react-query`.

## 2. Технологический стек

### Backend

| Компонент | Библиотека | Версия |
|---|---|---|
| Язык | Rust | 1.88.0 |
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
    ├── ADR.md
    ├── AGENTS.md
    ├── API.md
    ├── API_EDGE_CASES.md
    ├── API_STANDARDS.md
    ├── API_VERSIONING.md
    ├── ARCHITECTURE.md
    ├── BACKUP_RESTORE.md
    ├── CACHING.md
    ├── CI_CD.md
    ├── CLI.md
    ├── CODE_REVIEW.md
    ├── CODE_STYLE.md
    ├── DATABASE_INDEXES.md
    ├── DATABASE_STANDARDS.md
    ├── DATA_MODEL.md
    ├── DEPLOYMENT.md
    ├── DOMAIN_MODEL.md
    ├── ERROR_HANDLING.md
    ├── EVENTS.md
    ├── FRONTEND_ARCHITECTURE.md
    ├── FRONTEND_STANDARDS.md
    ├── I18N.md
    ├── LIBRARIES.md
    ├── LOCAL_SETUP.md
    ├── LOGGING_STANDARDS.md
    ├── MIGRATIONS.md
    ├── MONITORING.md
    ├── NOTIFICATIONS.md
    ├── OPS_RUNBOOK.md
    ├── PAGINATION.md
    ├── PERFORMANCE.md
    ├── PROJECT_ADMIN.md
    ├── RELEASE.md
    ├── REPORTS.md
    ├── RESILIENCE.md
    ├── REVIEW.md
    ├── ROADMAP.md
    ├── ROUTING.md
    ├── RUNTIME.md
    ├── SECURITY.md
    ├── STORAGE.md
    ├── SYSTEM_ADMIN.md
    ├── TESTING.md
    ├── TROUBLESHOOTING.md
    ├── TZ.md
    ├── UI_UX.md
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
- `entities/` — SeaORM models
- `storage.rs` — file attachment storage (local filesystem / S3-compatible)

## 5. Конфигурация

Конфиг загружается через `config` crate с префиксом `TASKTRACKER_`.

```rust
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub storage: StorageConfig,
    pub email: EmailConfig,
}
```

Основные env vars:
- `TASKTRACKER_DATABASE__URL`
- `TASKTRACKER_SERVER__PORT`
- `TASKTRACKER_JWT_SECRET`
- `TASKTRACKER_DATABASE_PASSWORD`
- `TASKTRACKER_AUTH__REFRESH_COOKIE_SECURE`
- `TASKTRACKER_STORAGE__MAX_UPLOAD_BYTES`

Fallback: если `auth.jwt_secret` не задан, используется `TASKTRACKER_JWT_SECRET`.

## 6. Middleware stack

```rust
Router::new()
    .merge(api_routes())
    .layer(TraceLayer::new_for_http())
    .layer(CorsLayer::new())
    .layer(CompressionLayer::new())
```

CORS: `TASKTRACKER_SERVER__CORS_ALLOWED_ORIGINS`. По умолчанию используется явный whitelist localhost-источников (`http://localhost:19877,http://localhost:5173`); wildcard `*` включается только при явном указании `*`.",

## 7. Security

- **AuthN**: JWT access token (`Authorization: Bearer`), refresh token в `httpOnly` cookie на `/api/v1/auth`
- **Hashing**: argon2id
- **Input validation**: `validator` derive на Request DTO + route-уровневые проверки
- **CORS**: wildcard без credentials либо explicit whitelist с `Access-Control-Allow-Credentials`

## 8. Frontend архитектура

- **Pages** — экраны: login, register, dashboard, projects, project-board, project-backlog, search, issue-create, issue-detail
- **Features** — time-tracking, owner SDLC intent/readback и будущие бизнес-модули
- **Shared** — ui-kit, i18n, auth store, theme, API hooks
- **App** — роутер (`react-router`), провайдеры QueryClient + ThemeProvider
- **Widgets** — `AppShell` с sidebar + header, адаптивный под mobile

`issue-detail?tab=sdlc -> features/sdlc -> api/sdlc` читает strict SDLC API через
существующий Central bearer client. Generated DTOs и metadata union остаются
единственным wire type source. Snapshot проверяет binding/revision/hash/routing,
затем UI показывает frozen intent как queued awaiting admission. Query cache
namespace меняется при смене credential без bearer в query keys; 401/403 скрывают
cached owner data, stale/readback error блокирует consent. Mutation использует
exact revision/hash и stable per-pin operation key, не optimistic transition.
Отдельная owner-only query читает существующую published project routing policy.
Unchecked publication opt-in сохраняет legacy omission; явный выбор фиксирует
version/hash отдельно от head и передаёт exact version в generated ConfirmCommand.
Stale policy/source errors блокируют routed confirmation без замены выбора;
unknown POST блокирует retry до fresh context readback. Нет редактора policy.
Нет Fleet/config/dispatch вызовов, новых runtime producers или live fixtures.
Подробнее: [B-SDLC-05 UI contract](SDLC_UI_V1.md).

Удаление использует общий `ConfirmDialog` из Base. Экран передаёт явную
ссылку на кнопку меню задачи; для worklog возврат focus после успеха направлен
на вкладку журнала, которая остаётся после удаления строки. При отмене focus
возвращается на кнопку исходной записи. Диалог остаётся смонтированным даже
при пустом списке до завершения Promise удаления и обновления query cache.
Ошибка относится только к текущему подтверждению и сбрасывается при закрытии
или выборе другой записи. Форма создания/редактирования worklog блокирует
закрытие и повторную отправку во время pending. Парсер длительности и правила
журнала принадлежат Tracker; контракт Base, API и схема БД не меняются.

## 9. API, документация и тестирование

Форма создания задачи сохраняет исходную команду и Idempotency-Key в session
journal до успешного ответа. Только первый структурированный отказ валидации
(`400`/`422`, `VALIDATION_ERROR`) освобождает команду, чтобы исправленные поля
отправлялись с новым ключом. После неопределённого исхода или восстановления
journal последующий отказ не доказывает отсутствие прежней записи: повтор
сохраняет исходный payload/key. Автоматический retry POST отключён. HTTP-контракт
и схема БД не меняются.

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

## 12. Общая Шапка Платформы

`AppShell` использует `PlatformHeader` из `@pdlc/ui`: общий компонент владеет
геометрией, порядком leading/services/context/actions и единственным переключателем
сервисов. Снимок file-зависимости и lock обновляются вместе с внедрением Base UI;
наличие экспортов в sibling source не заменяет проверку установленного пакета.

Проектный picker виден от 1024 px. На меньших экранах список проектов доступен
через основную навигацию. Поиск остаётся в навигации, без повторной ссылки в шапке.
Создание задачи сохраняет `project_key`: от 768 px это действие шапки, ниже —
явная команда мобильного drawer. Оно не заменяется созданием проекта. Темы,
уведомления и аккаунт остаются доступными во всех размерах. Слева — общий
платформенный знак; имя текущего приложения показывает переключатель сервисов,
без второго текстового дубля. Auth lifecycle и API этой адаптацией не меняются.

## References

- `README.md`
- `AGENTS.md`
- `docs/DEPLOYMENT.md`
- `docs/TESTING.md`

## Общая база

Подключение версий, границы контрактов и проверки описаны в [BASE_INTEGRATION](BASE_INTEGRATION.md).

## Центральный профиль

`GET /users` является read-only projection центрального каталога. Пакетный
repository SELECT получает существующие привязки по `central_sub`, включая
inactive записи, без вызова authentication/materialization primitive.
Исторические случайные UUID сохраняются. Для новых subjects reference равна
UUID v5 namespace URL от `urn:sdlc:task-tracker:central-user:<trimmed sub>`;
имя и namespace являются стабильной частью внутреннего identity-контракта.
Email не участвует в вычислении или связывании identity. Та же reference
используется при первой материализации; локальные UUID не мигрируются.

Явные assignee/reporter и project member ссылки разрешаются в mutation handler по проверенному
центральному каталогу, до передачи команды сервису. Профиль создаётся при
первом входе либо явном назначении, а не при открытии списка. Все ссылки
проверяются до материализации; обычный service validation остаётся в сервисе.
При совместной работе старого/new backend resolver использует реальный UUID
уже созданной записи, не перепривязывает её к новой reference.
Коллизии reference/исторического UUID не допускают объединения identity;
projection с неоднозначными IDs возвращает временную ошибку, INSERT collision
не перезаписывает legacy пользователя. Проверка активности authenticating
principal и центральные проверки сессии/PAT не ослабляются.

JWT и личные токены получают имя из того же проверенного ответа Central Auth,
что и identity. Связывание выполняется только по `sub`, никогда по email.
Повторная проверка обновляет только имя и время изменения; локальный ID,
email, username, дата создания и исторические ссылки остаются прежними.
Неизменённый активный профиль читается без блокировки строки на запись.
Отключённый профиль отклоняется без изменения имени или восстановления доступа.
При отсутствии проверенного имени запрос отклоняется; настроенный центральный
вход не переходит к историческому локальному паролю при ошибке.
Действующая политика равных пользовательских прав не меняется.

## Совместимость project repository lineage

Migrator выбирает опубликованную либо сохранённую project-repositories lineage по точному применённому имени миграции. Оба пути используют штатный SeaORM status/up и отклоняют неизвестную историю. Исходная применённая 0096 и опубликованная 0097 сохранены без изменений. Legacy-путь добавляет отдельную таблицу task-source snapshots перед 0097; проектные связи в неё не копируются. `m20261018_0000098_project_repository_catalog` добавляет проектный каталог для published/fresh БД. API проектного каталога и панель проекта в карточке задачи читают project_repository_links; опубликованный task API и внутренний SDLC source binding сохраняют отдельный task snapshot. Проектный каталог не предоставляет task/Git authority.

## Граница Восстановления PM

Восстановление ещё не отправленного первоначального запуска расширяет существующие
SdlcService/repository и Fleet reader. Права планировщика проверяются до HTTP;
задача, владелец, ACL и lease повторно проверяются под блокировками после readback.
Сетевой ввод-вывод не выполняется внутри транзакции. Точный исторический повтор
не требует запроса Fleet. Только доверенный Fleet подтверждает свой durable
barrier: отсутствие dispatch journal или истёкший lease сами по себе недостаточны.
Fleet владеет атомарным submission barrier и защищённым свидетельством, Tracker —
проверенным продлением и неизменяемой историей. Новых планировщика, сервиса, задачи,
native terminal или подтверждения владельца нет.
