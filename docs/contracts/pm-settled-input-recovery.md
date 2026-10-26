# Settled PM Input Expiry Recovery V1

Status: implemented source candidate; Cargo/PG/OpenAPI/live qualification pending.
This is not a claim that the installed producer supports recovery yet.

The shared contract is maintained in Fleet Control:
[version1 source](https://github.com/FerrPOINT/fleet-control/blob/42015825/docs/contracts/PM_SETTLED_INPUT_RECOVERY_V1.md).
Fleet owns the durable no-successor barrier; Tracker owns issuance and readback.
Neither service changes Hermes or repeats the original business message.

## Producer

The existing resume POST accepts optional recovery metadata: kind
settled_input_expiry, original key/hash, latest committed predecessor key/hash
and barrier_id. Without this member, request serialization/hash are unchanged.
Responses keep ExecutionLeaseReceipt/ExecutionLeaseOperation and
dispatch_allowed=false; recovery itself does not dispatch a native run.

The service reads the existing authenticated Fleet configuration-reader boundary:
GET /internal/runtime/v1/pm/runs/{previous_run_id}/settled-input-recovery with the
exact barrier_id query. The closed witness contains contract_version1,
session_run_id, barrier_id, execution identity, request_sha256 and the full command.
Current owner/project/assignment/fence, original consumed input/checkpoint and
terminal predecessor must match. The callback is not a client-provided boolean.

After callback read, issuance rechecks task/lease state under locks. Only the
exact current expired lease is advanced by one version using the database clock,
with unchanged holder, lease ID, claimed_at and TTL30. A newer completion/delivery
owner, stale version, fork or changed input rejects recovery. The original input
and its receipt are never updated or inserted again.

## Storage And Replay

Additive migration0103 reuses sdlc_pm_lease_operations.recovery. No new table or
old migration edit is required. It protects both insertion directions of the
logical resume/lease-operation key namespace; ambiguous results conflict rather
than being selected by query order.

Exact-key GET reads original and recovery ledgers. Same key/payload returns its
immutable receipt, not a renewal. If a recovery ACK is already expired, Fleet
retains it without a permit and keeps the barrier closed. A next attempt may
follow only that latest committed predecessor, with another exact expired CAS.
The default three-attempt limit is durable. Unknown acceptance is never a new attempt.

## Optional Trusted Manual Budget (Source Only)

Реализация Tracker `e7c0f57c` принимает optional `manual_budget` из ответа
существующего authenticated Fleet callback. Поле отсутствует при `None`, поэтому
legacy wire/hash не меняются. Публичный resume POST, его ключ, body и hash не
получают новых полей; клиент не может прислать разрешающий budget вместо callback.
Не добавляются endpoint, CLI, MCP, Auth role, таблица или изменения Hermes.

Закрытый payload содержит ровно девять полей:
`contract_version`, `root_operation_id`, `control_operation_id`,
`retry_operation_key`, `retry_request_sha256`, `original_resume_request_sha256`,
`prefix_count`, `prefix_sha256`, `allowance`. Требуются version1, canonical nonnil
UUID, canonical retry key, lowercase SHA256, `prefix_count >= 3` и `allowance = 3`.
`original_resume_request_sha256` должен совпадать с исходным hash в recovery
command. Fleet root/control IDs не приравниваются к Tracker execution/run IDs.

Fleet проверяет immutable committed prefix и текущего pending retry parent
перед выдачей callback proof. `prefix_sha256` запечатывает полные Fleet commands,
включая kind/request/attempted/result: это НЕ hash проекции Tracker. Tracker
проверяет свою цепочку оригинала/predecessor и число `n` уже committed recoveries
для того же original request. Без budget разрешено только `n < 3`; с валидным
budget требуется `prefix_count <= n < prefix_count + 3`. Grant с prefix3 допускает
позиции3,4,5 (четвёртую-шестую recovery), но не позицию6. Grant сам по себе не
доказывает ACK и не предоставляет lease или business permission.

Preflight за default cap только читает custody, чтобы обратиться к trusted
callback; это не выдача разрешения. Commit заново проверяет task/owner/project,
assignment/fence, exact command/barrier и expired lease CAS под существующими
locks, затем сохраняет budget внутри `sdlc_pm_lease_operations.recovery.witness`.
Forward104 payload `backend/migration/src/pm_manual_recovery_budget.sql` меняет
только cap и optional closed witness shape старого SQL gate, сохраняя остальные
guards. Main `fb5ef353` регистрирует wrapper
`m20261026_0000104_pm_manual_recovery_budget`; applied103 не редактируется.

Fresh issuance требует свежего callback. Exact-key historical GET/replay
проверяет сохранённый witness и его исходную позицию, не требует снова живого
Fleet parent после его completion и не обновляет срок lease. Исторический ACK
не становится текущей authority. Source contract не снимает unknown у уже
attempted четвёртой recovery и не разрешает reset latch, повтор POST или новый key.

Для этого расширения пройдены только source format/review; compile, PostgreSQL,
новый image и live counterpart НЕ проверены. Установленный Tracker533 сохраняет
cap3 и остаётся producer blocker для четвёртой recovery. Старые qualification
receipts не являются проверкой optional budget/104.

## Verification And Rollout

Added tests cover old-wire compatibility, exact witness binding, invalid anchors,
expired CAS, lost ACK/readback, bounded chain, concurrency, revocation and both
key-collision directions. Reader tests verify the exact URL and existing token,
and reject redirects, denied responses, oversized bodies and caller booleans.
Test presence is not evidence that these tests have passed.

The bounded local candidate starts from the installed7d6 producer. Current main
also contains a separate native-spaces/auth/SDK rollout; those changes are not
silently deployed as part of first-PM repair. Migration0103 reserves ordering
after main's0100-0102 without changing their files. Main compatibility and local
runtime acceptance are separate gates.

Install the qualified Tracker producer first, then the qualified matching Fleet
consumer. Builds use independent local QA resources and exact source/image
receipts. Completion requires the same PM chat, owner-confirmed requirements
and Backlog/Ready without an Analyst intent; no SQL authority edit substitutes
for that acceptance.
