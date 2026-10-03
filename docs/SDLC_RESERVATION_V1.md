# Подготовка Analysis assignment v1

## Граница среза

Tracker — единственный владелец assignment identity, operation key/hash, fence и
capacity ledger. Этот API создаёт только prepared assignment для текущего Analysis
intent: `admission_state: awaiting_admission`, `dispatch_allowed: false`.
Stage остаётся Analysis, статус задачи — Ready/todo. Нет dispatch, InProgress,
session/run binding, release, ACK, stop proof или фонового второго scheduler.
Fresh Fleet native admission, Workflow acceptance и trusted verified-stop остаются
обязательными следующими counterpart interfaces, а не результатом reserve.

Нужен snapshot явной публикации из [SDLC_ROUTING_V1](SDLC_ROUTING_V1.md).
Текущий UI confirmation routing не включает. Legacy задачи без snapshot не
включаются автоматически. Любое прежнее неподтверждённое PM execution/assignment,
в том числе сохранённая история, даёт `409 pm_quiescence_unverified`.
Stage, confirmation и expiry не доказывают остановку PM. Пока trusted stop
counterpart отсутствует, этот API не разблокирует такие задачи.

## Authority

Deployment задаёт `TASKTRACKER_SDLC__RESERVATION_SCHEDULER_SUBJECT` отдельно от
PM orchestrator, verifier и владельца задачи. POST требует fresh Central machine
identity с точным единственным scope `[task-tracker:write]`. Этот субъект должен
иметь активную связанную запись пользователя и persisted project membership по
существующей политике Tracker. Global admin, local fallback, human session,
compound grants, agent prompt или owner-supplied subject полномочия не заменяют.
Пустая конфигурация scheduler даёт 503. PAT выпускает зарегистрированный Base
issuer; новые виды scopes не вводятся.

GET требует существующий Central human/project ACL либо machine с точным scope
`[task-tracker:read]` и тем же persisted project ACL. Для scheduler readback нужен
отдельный read PAT, а не добавление второго scope в mutation PAT. Права проверяются
заново и для replay/operation lookup; отзыв пользователя или membership блокирует
старый receipt. Условия не предоставляют Fleet/Hermes delegation или run rights.

## Strict API

Базовый путь: `/api/v1/issues/{id}/sdlc/analysis-reservation`.
Ответы не кешируются (`Cache-Control: no-store`).

| Метод | Вход / результат |
| --- | --- |
| POST | `intent_id`, `routing_snapshot_id`, `requirement_revision`, `content_hash`, `expected_reservation_version:0`, `idempotency_key`; 201 новый receipt, 200 первоначальный replay |
| GET | текущий receipt или null, observed time, lease state, reconciliation reason и retained capacity |
| POST `/heartbeat` | `assignment_id`, `execution_id`, `fencing_token`, `lease_id`, `expected_lease_version`, `idempotency_key`; 201 новая версия, 200 первоначальный replay |
| GET `/operations/{idempotency_key}` | original result и `request_sha256`; 404 при отсутствии операции |

Unknown поля запрещены. UUID канонические, ненулевые; версии положительные safe
JSON integers до 9007199254740991, initial reservation CAS только integer 0.
Agent, config, роль, workflow, TTL, флаги readiness/stop caller не передаёт.
Stale intent/snapshot/revision/hash, занятый root/agent/pool, другое содержимое
прежнего key и новый key уже reserved задачи дают 409 без writes.

## Immutable envelope

Receipt содержит `assignment`, отдельный `lease`, `technical_pool_slot`,
`ttl_seconds:30`, `heartbeat_interval_seconds:10`, `admission_state`,
`dispatch_allowed:false`, `capacity_held:true`.

Assignment v1 фиксирует:

- `tracker_instance_id`, `project_id`, `task_id`, `root_task_id`, `owner_subject`;
- `intent_id`, `confirmation_id`, `requirement_revision`, `content_hash`;
- `routing_snapshot_id`, `routing_policy_version`, `routing_hash`;
- backend UUID `assignment_id`, `execution_id`, `lease_id`, `reservation_version:1`;
- `workflow_task_ref: SDLC-<ordinal>` из отдельной backend DB sequence
  `sdlc_analysis_workflow_task_ordinal`; не chat UUID, display issue key,
  root/legacy PM ordinal или fencing token;
- `agent_id` и exact frozen `route: RoleRoute` из `policy.routes.analyst`;
- `stage:Analysis`, `role_key:analyst`, `workflow_key:hermes-sdlc:analyst`,
  `mode_key:analysis`, `scope:business`, `cycle_number:0`, `attempt_number:1`;
- backend monotonic `fencing_token`, `assignment_operation_key`, `created_at`;
- `assignment_hash`: SHA-256 существующей canonical JSON функции Tracker от всего
  assignment, исключая только поле `assignment_hash`.

Сам intent сохраняет pre-admission attempt 0. Assignment attempt 1 не свидетельствует
о выполнении. Workflow task ref неизменяем, входит в assignment hash и сохраняется
при heartbeat/replay. Ordinal выдаётся независимо от fence; rollback может оставлять
пропуски, но повторное использование ref запрещено unique constraint/NO CYCLE.
Legacy PM identities и их история не конвертируются.
Это будущий exact `RuntimeAssignmentRequest.task`, не принятый Workflow task/cursor.
Tracker не делает fallback lookup или dispatch consumer.

Session/run UUID не выдумываются; они принадлежат будущему Fleet
acceptance binding. Workflow predecomposition shape соответствует
`business-pre-decomposition`, но вызов Workflow/API adapter в этот срез не входит.
Physical Forge refs, decomposition revision и workspace generations отсутствуют.

Route является declared reference, не свежим proof. Fleet counterpart mapping:
revision — `effective_revision`, не desired; manifest — `package.manifestSha256`;
catalog — `workflow_binding.catalog_sha256`, не полный export hash. Namespace и
workflow IDs — канонические положительные signed-i64 decimal strings, не Base
namespace symbols. См. [точный DTO mapping](SDLC_ROUTING_V1.md).

Operation request hash вычисляется отдельно от immutable assignment hash:
canonical `{operation: reserve_analysis|heartbeat_analysis, payload: command}`.
Key scoped к task, сохраняется один original receipt. Replay не меняет current
head, timestamps, outbox, capacity, fence или assignment hash, даже после expiry.

## Lease и capacity

Lease head: `version`, `holder_subject`, `claimed_at`, `heartbeat_at`, `expires_at`.
Heartbeat требует exact immutable identity/fence и current lease version CAS.
Новая версия продвигается на единицу; expiry задаётся backend PG clock +30s.
Heartbeat после expiry запрещён. Рекомендованный интервал heartbeat — 10s.
Original receipt replay не означает, что его старая lease version сейчас активна.

GET не делает renewal или reconciliation writes. `lease_state`:
`unreserved|active|expired`. Expiry явно возвращает `reconciliation_needed:true`,
`reason:lease_expired_stop_unverified` и `capacity_held:true`; прежний receipt
остаётся неизменным. Неизвестный PM даёт reason `pm_quiescence_unverified`.
Без trusted stop не существует release/reacquire/replace endpoint.

Одна транзакция проверяет task/ACL/current publication, резервирует root1/agent1/
technical pool2, сохраняет assignment, lease, operation receipt и outbox
`analysis.assignment_reserved` (`event_id=assignment_id`). Все holds, включая
expired/unknown, занимают capacity. PM не занимает technical slot, но его
неподтверждённые root/agent bindings блокируют пересечение; новые PM assignment
writes также проверяют retained Analysis capacity. Старые PM rows не переписываются.
Событие сообщает о preparation, не является delivery/ACK command.
Typed `metadata_v1` проекция содержит только assignment/execution/intent/snapshot/
agent refs, `workflow_task_ref`, fence и hash; не публикует private package content.

Перед явным opt-in требуется compatible Fleet build с двумя strict read-only
типами `analysis.intent_created` и `analysis.assignment_reserved`. Исторический
consumer девяти PM types обязан fail-closed: unknown event нельзя пропускать или
продвигать cursor. Расширение union в metadata v1 не делает read-only проекцию
полным assignment API, runtime acceptance или native admission.
Reserved resource имеет ровно восемь полей: `assignment_id`, `execution_id`,
`workflow_task_ref`, `intent_id`, `routing_snapshot_id`, `agent_id`,
`fencing_token`, `assignment_hash`. Intent resource — полный прежний AnalysisIntent;
его stage/status Analysis/Ready, cycle/attempt 0, без lease/admission полей.

## SQL и проверка

Расширена только pending 000034: singleton capacity lock, ограниченная sequence
fence, append-only `sdlc_analysis_reservations`,
`sdlc_analysis_reservation_operations` и CAS head
`sdlc_analysis_reservation_leases`. Unique root/agent/pool slot удерживают capacity
без expiry predicates. FK/insert gates сверяют exact intent/snapshot/route;
deferred FK требуют lease и current operation receipt в той же транзакции.
Lease mutation gate запрещает delete, identity rewrite, version jump и expired
renewal. Исторические data не backfill-ятся и не enrolled автоматически.

Scoped проверки находятся в `app::sdlc_reservation`, `api::routes::sdlc_reservation`
и isolated PG/HTTP `support/analysis_reservation.rs`. Fixtures — только disposable
PG, без native admission/stop evidence. Полный build/CI/deploy/native acceptance
не являются частью проверки этого source-среза.

### Проверенный source-срез

Scoped app command/authority test, Rust API reservation schema test (включая
уникальность operation IDs), metadata schema test и один PostgreSQL/HTTP lifecycle
fixture прошли. Последний проверяет concurrent root/agent/pool reservation,
project ACL и отзыв membership, CAS/replay, rollback outbox, immutable routing/hash/
workflow task ref, реальный TTL expiry и durable readback после restart.
Deferred FK и SQL mutation gates проверены отрицательными транзакциями.

OpenAPI заново экспортирован штатным Rust `api --bin gen-openapi`, а не правкой
frozen JSON. Экспорт содержит Stage `Draft|Clarification|Backlog|Analysis` и
11 strict metadata event types. Generated TypeScript consistency и scoped
B05 typecheck прошли. Exhaustive B05 event-label map дополнен только нейтральной
подписью prepared/awaiting admission; отдельный history component test прошёл.
Новые policy UI, opt-in UI, admission или runtime-success состояния не добавлены.

Actual PG/HTTP metadata fixture сохраняет bounded ответ `metadata_v1` с
`analysis.intent_created` и `analysis.assignment_reserved`; routing payload,
package content и secrets в него не входят. Существующий metadata9 fixture
не менялся. Local evidence хранится вне tracked source.
Проверки выполнялись в отдельном temporary Compose project с task/purpose labels;
его контейнеры и сеть удалены в finally. Full build/CI, native admission и dispatch
не проверялись и не заявляются.
