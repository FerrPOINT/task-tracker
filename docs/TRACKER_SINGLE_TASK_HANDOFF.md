# Tracker single-task lifecycle: producer handoff

Baseline: `9eac790a332d9501511d8cac2a3ef0f3ac7ddc23`.
This is source implementation for parent integration, not live acceptance.
No Cargo, PostgreSQL, Docker, Fleet run or Workflow run was executed here.
No terminal receipt has been invented. New API registration is deliberately absent.

## Existing producers

`P = /api/v1/issues/{task_uuid}/sdlc`.

| Producer | Actual endpoint | Actual request / response |
| --- | --- | --- |
| Draft creation | POST `/api/v1/projects/{project}/sdlc/drafts` | `CreateDraftCommand` / `CreatedDraft` |
| Creation reconciliation | GET `/api/v1/projects/{project}/sdlc/drafts/operations/{key}` | Original `CreatedDraft` |
| PM assignment | POST / GET `P/pm-draft-assignment` | `ReservePmDraft` / `PmDraftReservation`, GET `PmDraftReadback` |
| PM lease | POST / GET `P/pm-draft-execution-lease` | `ClaimExecutionLease` / `ExecutionLeaseReceipt`, GET `ExecutionLeaseReadback` |
| PM heartbeat | POST `P/pm-draft-execution-lease/heartbeat` | `HeartbeatExecutionLease` / `ExecutionLeaseReceipt` |
| PM terminal | POST / GET `P/pm-draft-completion` | `CompletePmDraft` / `PmDraftCompletionReceipt`, GET `PmDraftCompletionReadback` |
| PM terminal operation | GET `P/pm-draft-completion/operations/{key}` | `PmDraftCompletionOperation` |
| Exact owner confirmation | POST `P/requirements/{revision}/confirm` | `ConfirmCommand` / `Confirmation` |
| Analysis intent | GET `P/analysis-intent` | `AnalysisIntent` |
| Analysis assignment | POST / GET `P/analysis-reservation` | `ReserveAnalysis` / `AnalysisReservationReceipt`, GET `AnalysisReservationReadback` |
| Analysis heartbeat | POST `P/analysis-reservation/heartbeat` | `HeartbeatAnalysis` / `AnalysisReservationReceipt` |
| Analysis reservation operation | GET `P/analysis-reservation/operations/{key}` | `AnalysisReservationOperation` |
| Analysis terminal | POST `P/analysis-result` | `SubmitAnalysisResult` / `AnalysisResultReceipt` |
| Analysis terminal operation | GET `P/analysis-result/operations/{key}` | `AnalysisResultOperation` |

Analyst has a real reservation/result producer, but its reservation is still
`awaiting_admission` with `dispatch_allowed=false`; it does not establish runtime
execution. Architect, Developer, Reviewer, Tester and DevOps have no registered
assignment/result API on the baseline. No role consumer may treat these as live.

Existing Analysis DTOs are in `backend/domain/src/sdlc_reservation.rs`.
`ReserveAnalysis` fields: `intent_id`, `routing_snapshot_id`,
`requirement_revision`, `content_hash`, `expected_reservation_version:0`,
`idempotency_key`. `HeartbeatAnalysis`: `assignment_id`, `execution_id`,
`fencing_token:i64`, `lease_id`, `expected_lease_version`, `idempotency_key`.
`SubmitAnalysisResult`: `contract_version:1`, `assignment_id`, `execution_id`,
`agent_id`, `intent_id`, `routing_snapshot_id`, `requirement_revision`,
`content_hash`, `fencing_token:i64`, `lease_id`, `expected_lease_version`,
`assignment_hash`, `terminal_receipt_ref`, `terminal_receipt_sha256`,
`architecture_input_sha256`, `summary`, `idempotency_key`.
Some older CLI forwarding fixtures are not valid server DTO examples.

## PM default and owner receipt

Project default source: GET `/api/v1/projects/{project}/sdlc/routing-policy`,
`RoutingPolicy.routes.project_manager`. Frozen published task routing: GET
`P/routing-snapshot`, `TaskRoutingSnapshot`.

New reservation code enforces a configured project's PM default under the
existing project lock, before allocating identity. Original replay precedes
this check: a changed default cannot rewrite an existing saga. Projects without
a published policy retain the explicit-selector legacy path; the single-task
Fleet entry must require an actual project policy and select its PM.
No configuration/admission/native proof requirement was added.

Added CLI commands targeting existing endpoints:

```text
task-tracker sdlc routing policy --project UUID
task-tracker sdlc routing snapshot --task UUID
task-tracker sdlc draft create --project UUID --from-file draft.json
task-tracker sdlc draft operation --project UUID --key KEY
task-tracker sdlc pm assignment reserve --task UUID --from-file reservation.json
task-tracker sdlc pm assignment read --task UUID --key KEY
task-tracker sdlc pm lease claim --task UUID --from-file lease.json
task-tracker sdlc pm lease heartbeat --task UUID --from-file heartbeat.json
task-tracker sdlc pm lease read --task UUID --key KEY
task-tracker sdlc pm completion submit --task UUID --from-file completion.json
task-tracker sdlc pm completion operation --task UUID --key KEY
task-tracker sdlc revision confirm --task UUID --revision N --from-file consent.json
task-tracker sdlc confirmation read --task UUID --revision N
```

`revision confirm` is owner-session only. Payload: `content_hash`,
`idempotency_key`, optional `expected_routing_policy_version`.
Output is the original `Confirmation` with `stage:Backlog`, even though the
transaction queues aggregate Analysis. `confirmation read` performs GET
`analysis-intent`, then bounded legacy `events?after=...` pagination. It returns
the actual `requirements.confirmed.payload.result` only when confirmation UUID,
task UUID, revision and hash match the intent. It never POSTs or fabricates consent.
Workflow uses an ordinarily authorized read credential, not a PM assignment PAT.

## Internal lifecycle contract for parent

Types: `backend/domain/src/sdlc_lifecycle.rs`.
Application methods extend the existing `SdlcService`; no new controller, DI
context or scheduler is needed. `SdlcRepository::lifecycle_repository()` is an
optional adapter implemented by `PostgresSdlcRepository`; legacy stubs default
to unavailable rather than claiming success.

```rust
SdlcService::lifecycle_command(task, actor, LifecycleCommand)
    -> Result<(LifecycleReceipt, bool), AppError>;
SdlcService::lifecycle_read(task, actor)
    -> Result<LifecycleReadback, AppError>;
SdlcService::lifecycle_operation(task, actor, key)
    -> Result<LifecycleReceipt, AppError>;
SdlcService::lifecycle_result(task, actor, result_id)
    -> Result<DeliveryInput, AppError>;
SdlcService::submit_delivery_proposal(task, actor, SubmitDeliveryProposal)
    -> Result<(DeliveryProposalReceipt, bool), AppError>;
SdlcService::delivery_proposal_operation(task, actor, key)
    -> Result<DeliveryProposalReceipt, AppError>;
SdlcService::submit_pm_feedback(task, actor, SubmitPmFeedback)
    -> Result<(PmFeedbackReceipt, bool), AppError>;
SdlcService::pm_feedback_operation(task, actor, key)
    -> Result<PmFeedbackReceipt, AppError>;
```

Approved registration in the existing controller: GET/POST `P/lifecycle`,
GET `P/lifecycle/operations/{key}`, POST `P/delivery-proposals`,
GET `P/delivery-proposals/operations/{key}`, POST `P/pm-feedback` and
GET `P/pm-feedback/operations/{key}`. Parent owns handlers, OpenAPI and registration.

`LifecycleCommand` currently has an internal `operation` discriminator with
snake_case values `ensure_queued`, `reserve`, `heartbeat`, `bind_run`, `complete`, `reconcile`.
Reserve takes `expected_task_version`; fence contains assignment/execution UUID,
integer fencing token, lease UUID and expected lease version. `bind_run` takes
actual chat/run UUIDs, `machine_subject` and dispatch receipt.

JSON stage/status are PascalCase; roles/runtime/verdicts are snake_case, including
`devops`. Receipt Option fields serialize explicitly as null, except additive
`LifecycleTask.resume_chat_id`, which is omitted when None to retain old receipts.
There are no children,
integration modes, dispatch-success flags, MCP or Hermes native-admission claims.
`LifecycleReadback` is `{observed_at,runtime_state,receipt}`: PostgreSQL `clock_timestamp()`
after locks, serialized in UTC with nine fractional digits. GET never renews a
lease or modifies the receipt; consumers compare lease expiry to observed_at.
Effective `runtime_state` is `unknown` after lease expiry, while the canonical
receipt remains unchanged. Only a verified scheduler terminal reconciliation
can release the hold without an accepted business result.
Command replay and operation readback return the original immutable receipt,
not a fresh-current wrapper. There is no separate response-hash field.

`assignment_hash` is the existing canonical JSON SHA-256 of assignment excluding
itself. Command operation hash covers the whole typed command. Namespace is
`(task_id,actor_subject,idempotency_key)`. Changed-payload replay conflicts.
`terminal.output_sha256` hashes the typed result. Proposal namespace is the same
three fields in its append-only operation ledger.

Identity checks explicitly match task/project/root/instance/owner, consent UUID,
requirements revision/hash, and routing policy hash. Root equals task. Assignment
matches the frozen task routing, stage and cycle. Neither chat UUID nor task key
is a substitute for task identity.

## Agent final command and Fleet terminal handoff

Parent registration in the existing controller:

| Endpoint to register | Auth | Input / output |
| --- | --- | --- |
| POST `P/delivery-proposals` | Bound producer machine, write | `SubmitDeliveryProposal` / `DeliveryProposalReceipt` |
| GET `P/delivery-proposals/operations/{key}` | Original producer machine, read | Original `DeliveryProposalReceipt` |

Proposal contains `fence`, `workflow_receipt:{reference,sha256}`, typed `result`,
`idempotency_key`. It does not contain a Fleet terminal receipt.
The CLI source implements the actual HTTP producer/readback calls:

```text
task-tracker sdlc result propose --task UUID --from-file result-proposal.json
task-tracker sdlc result operation --task UUID --key KEY
```

These commands require parent API registration and a built CLI before they are
available against a real server. Workflow's final instruction must execute
these calls, reconcile unknown acceptance using readback, then exit the runtime.
It must not merely tell an operator that work is complete.

Role outputs are:

| role | Required result |
| --- | --- |
| analyst | analysis artifact receipt |
| architect | design artifact receipt |
| developer | exact 40-character commit SHA, changes receipt, nonempty executed-check receipts |
| reviewer | approved/rework verdict and report receipt |
| tester | passed/rework verdict and report receipt |
| devops | deployment and verification receipts |

Artifact receipt is `{reference,sha256}`, never a success boolean.
The producer must have service write scope, exactly one ordinary assignment grant,
and the current RunBinding's machine
subject, exact live assignment fence, matching role and unexpired lease.
Readback uses service read scope and the original author namespace with fresh
active-user/project authorization. Grant format is
`task-tracker:sdlc:delivery:{task}:{assignment}:{execution}:{agent}:{fencing_token}`.
Only read/write service scopes plus this one canonical grant are allowed.
Human sessions and unrelated grants cannot submit proposals.
Strict SDLC middleware restricts this credential to its task read/proposal family;
legacy Tracker middleware denies it before profile linking or generic CRUD.
Base delegation stays generic; no scheduler token is issued to agents.

Heartbeat CAS requires the exact current lease version. Proposal, bind and Complete
accept an earlier positive version of the SAME live lease, with all stable identities
and the assignment hash still checked. A heartbeat cannot invalidate an in-flight tool
call. Complete still rejects expiry. On 409, reconcile the ORIGINAL operation first,
then GET fresh lifecycle readback with the read-only child token; never change a payload
under a previously accepted key or redispatch an unknown-acceptance run.

Proposal does not change stage/status or release capacity. Existing Fleet
consumer observes actual runtime terminal and then invokes scheduler-only
Complete with matching proposal output/hash/Workflow receipt plus the actual
Fleet terminal receipt. The receiver relies on this trusted scheduler authority;
it does not independently contact Fleet or verify remote receipt contents.
Scheduler is exactly configured `reservation_scheduler_subject`, non-human,
scopes exactly `task-tracker:write`, distinct from PM/verifier. No new business
scheduler is introduced. Fresh lifecycle read uses existing read authorization;
lifecycle operation reconciliation is scheduler-subject-only with exactly
`task-tracker:read`; POST commands use its separate exactly-write credential.

`Reconcile` requires `fence`, `terminal:{assignment_hash,chat_id,run_id,state,receipt}`,
`recovery:{disposition:"stop"}` or `{disposition:"retry_transient",replay_safety_receipt}`,
and `idempotency_key`. Terminal state is `succeeded|failed|cancelled`, never running.
Only failed, safely replayable transient operations can retry, at most three times.
An immutable reconciliation precedes hold release, with no result/stage advance.
The next assignment gets new execution/fence/attempt and must bind the previous chat.

## PM feedback and resume boundary

`SubmitPmFeedback` is `{message_id,fence:MachineFence,text,idempotency_key}`.
The actual owner session writes an immutable feedback receipt and one event in
the existing outbox, using the existing idempotency ledger. `PmFeedbackReceipt`
contains task/project/root/owner/instance, the current PM fence, original text and
hash, nullable current requirement revision and server creation time.
It never creates a clarification answer or changes requirements, consent,
stage or lease; accepting idle feedback does not require a live lease.
Parent still owns HTTP registration and Fleet delivery of this receipt:
POST `P/pm-feedback` and GET `P/pm-feedback/operations/{key}`, owner-session only.
CLI source calls them through `sdlc pm feedback submit --task UUID --from-file feedback.json`
and `sdlc pm feedback operation --task UUID --key KEY`. They are not live before registration.

The application exposes `pm_feedback_resume_contract(state,feedback,checkpoint,lease)`:
exact task/project/root/owner/instance, assignment fence, actual prior terminal,
checkpoint UUID/version and server-observed live lease produce the existing resume
identity `pm-feedback:{message_id}`. It does not dispatch or renew a lease.
Tracker has no generic checkpoint/rebind API, and its expired PM lease cannot currently
be reclaimed. Fleet must reuse
its actual checkpoint/rebind after confirming old runtime terminal, retaining
task/chat/execution/checkpoint and creating only a new runtime run. Replay must
use the feedback UUID as the stable operation identity. A live lease can use
the existing heartbeat; an expired one needs a narrowly guarded resume CAS and
new fence generation coordinated with Fleet. Do not change the ordinary expired
heartbeat rule or reopen accepted PM business completion. Post-publication
dialogue must retain the existing PM business-write freeze.

## Canonical projection and optional Context fields

`sdlc_tasks.state` is the canonical business aggregate. The delivery execution head
contains no task copy; `sdlc_delivery_tasks` is a read-only join view. Canonical
state, execution head, issue projection/history, operation and outbox commit together.
An exact transaction permit protects canonical writes, and all legacy PM/Analysis
gates remain enforced. Missing mapping fails with
`lifecycle_issue_status_mapping_missing:{stage}:{status}`, never default Ready.

Only THREE new issue statuses are seeded: `SDLC Ready` (todo), `SDLC InProgress`
(in_progress), `SDLC Completed` (done). Explicit `(stage,status)->status_id` mapping
is many-to-one: Development/Ready and Review/Ready share one UUID. Stage remains
in canonical SDLC state. No legacy status IDs or history are rewritten.

Context adds two optional, omitted-if-None fields (not emitted as null):
`status: Ready|InProgress|Completed` and
`draft_submode: brainstorming|clarification|awaiting_confirmation`.
Canonical TaskState has the same optional fields and retains other canonical JSON
through serde round trips. Rows without fields remain without fields. Legacy
Clarification history stays serialized as Clarification; Context normalizes stage to Draft.
Enabled projects' new PM Draft starts Ready/brainstorming, lease claim is InProgress,
published questions/requirements wait in Ready, owner answer returns PM to InProgress,
and confirmation queues Analysis/Ready. Waiting reason is not a status or runtime proof.

Rollout depends on parent's `SdlcConfig.single_task_project_ids` HashSet, empty by default,
from canonical CSV `TASKTRACKER_SDLC__SINGLE_TASK_PROJECT_IDS`. New EnsureQueued/Reserve
checks it AFTER original replay and BEFORE effects, returning explicit Unavailable when
disabled. Replay, reads/history, heartbeat/proposal/Complete of existing execution remain
available after disable. New canonical PM Draft fields are enabled only for listed projects.

Scoped SDLC project authorization always requires an active local user and project owner
or explicit membership. Verified human/global-admin hints do not expand access or directory.
Exact requirements confirmation remains task-owner-session-only; public Tracker legacy
authorization is unchanged except denial of the new assignment-scoped machine grant.

## Parent integration and requested verification

1. Register the SQL fragment `backend/migration/src/single_task_lifecycle.sql`
   once, after 000095, in the migration/order owned by parent. It creates delivery
   heads, immutable assignments/proposals/results/operations, run bindings and
   holds, plus immutable PM feedback. It does not rewrite historical PM rows.
   It also fixes the baseline routing snapshot gate's `pm_execution_id IS NULL`
   restriction: exact reserved/completed PM executions can now publish the
   owner-confirmed routing snapshot. Keep the explicit consent/current-policy
   checks and test reservation + routing opt-in in the same real saga.
2. Projection is implemented in the producer source, including three issue status IDs,
   the explicit mapping, canonical permits and legacy-history compatibility. Integrate
   the updated SQL fragment rather than the old stored-task-head table design.
3. Register handlers/OpenAPI/DTO schemas in the existing controller. No new
   controller is required. Producer metadata now covers delivery lifecycle/proposals,
   reconciliation, feedback and the existing PM completion event, with immutable ledger
   readbacks and without business text, reports or artifact references.
4. Legacy Analysis now uses the shared capacity view for occupied task/agent/slot
   queries; SQL cross-ledger gates and the existing common capacity mutex remain.
5. Complete Fleet's existing consumer: lease heartbeat, actual run binding,
   proposal delivery, runtime terminal, guarded terminal acceptance, and unknown
   acceptance readbacks. Retry policy permits at most three safely replayable
   transient retries. Tracker exposes guarded reconciliation, not a second scheduler.
   Unknown runtime acceptance must never produce another run.
6. Wire PM feedback, guarded lease resume and actual checkpoint/rebind. Keep
   can_send disabled until durable acceptance and the runtime resume path exist.

Requested single parent QA run, after integration:

```text
cargo test --locked -p app sdlc_lifecycle
cargo test --locked -p app sdlc::tests
cargo test --locked -p task-tracker-cli --test workflows sdlc_
cargo test --locked -p infra canonical_projection_heartbeat_race_terminal_recovery_and_disabled_replay -- --ignored --nocapture
cargo test --locked -p server --test drafts -- --ignored --nocapture
cargo test --locked -p server --test sdlc -- --ignored --nocapture
```

The new infra PG test requires a fresh disposable `TT_LIFECYCLE_TEST_DATABASE_URL`,
with database name ending `_lifecycle_test`; it never starts Docker or creates a database.
It applies the unregistered fragment only if the parent migration has not created its heads.
Source covers all six roles on one UUID, same Ready ID across Development/Review,
outbox-failure rollback, concurrent heartbeat/proposal, expired terminal rejection,
reconciliation/retry, stale assignment rejection and disable/replay. All receipts there
are explicitly synthetic fixtures, not live runtime evidence.

Remaining parent acceptance coverage includes:
concurrent reserve pool2/agent1/task1; duplicate/different-key races; fresh ACL
revocation on writes and replay; wrong subject/role/fence/revision/project/root;
lease expiry while blocked on a row; heartbeat CAS and immutable replay;
proposal while runtime lives without stage change; terminal only after actual
Fleet terminal and matching proposal; missing/proposal-changed terminal denial;
outbox-failure rollback of result/release/queue; server-clock readback after
restart; legacy Analysis compatibility; Reviewer and Tester Rework through
Developer then Review/Testing, fourth rejection held for intervention;
PM-default mismatch and default change after replay; actual owner receipt CLI;
idle feedback dedupe with unchanged clarification/requirements/lease; feedback
after PM business completion; guarded resume without a duplicate runtime run.

Local performed checks are formatting and `git diff --check` only. Source tests
were added but not executed. Full lifecycle, proposals and feedback need the
above parent registrations before any real HTTP/runtime acceptance can be claimed.
