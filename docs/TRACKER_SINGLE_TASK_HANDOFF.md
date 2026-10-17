# Tracker single-task lifecycle: producer handoff

Baseline: `9eac790a332d9501511d8cac2a3ef0f3ac7ddc23`.
This is source implementation for parent integration, not live acceptance.
No Cargo, PostgreSQL, Docker, Fleet run or Workflow run was executed here.
No terminal receipt has been invented. New API registration is deliberately absent.

Parent integration adds API registrations and the single additive migration in
Draft [PR152](https://github.com/FerrPOINT/task-tracker/pull/152). The no-registration
statement above describes the original worker handoff, not the integrated branch.

### Profile-aware routing source extension

Routing policy/snapshot v1 retains the Base package fields and canonical JSON.
The new v2 family uses optional omitted `instruction_profile=single-task-v1` and
`instruction_manifest_sha256`, omitting both Base package fields. All seven
routes require the same source family/manifest/catalog, distinct agents and IDs,
namespace `single-task-<hyphenated-role>`, workflow `single-task-v1:<role>`,
profile `single-task-v1-<hyphenated-role>` and catalog1. Mixed families and a
Workflow profile impersonating a Base package are rejected. Analysis intent uses
the frozen Analyst workflow key. This does not bypass configuration preflight;
the profile-specific Fleet configuration observation remains to be integrated.
Synthetic DTO fixtures can be emitted with example `single_task_contract
--single-task`; its default still emits retained v1 contracts.

Delivery heartbeats update lease head and immutable operation/replay history,
not the business outbox mirrored into chat. Existing historical heartbeat events
remain readable. Source implementation and qualification are tracked separately.

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
transaction queues aggregate Analysis. `confirmation read` performs one GET
`P/requirements/{revision}/confirmation` and returns the exact saved `Confirmation`.
It never reads analysis-intent/outbox, POSTs or fabricates consent. The direct
repository reader accepts the current PM assignment grant plus service read scope,
with fresh active-user/project ACL, assignment/subject and exact revision/hash/owner
history checks, but no live lease requirement. Missing consent is 404 (WAIT), ledger
inconsistency is 409; invalid revision is 422 and unauthorized access is 403.
Historical receipt stage stays Backlog even when the task has advanced.

Parent must register this GET/OpenAPI and add its PM allowlist case. Producer source
provides `SdlcService::confirmation_read(task,actor,revision) -> Confirmation` and the
API-independent predicate `app::sdlc_confirmation::is_confirmation_read(method,parts)`.
In the existing `pm_request_policy`, AFTER grant and own-task-prefix validation,
replace the default `false` arm with that predicate on `parts.as_slice()`. No other
PM route is opened, particularly analysis-intent, owner confirmation POST or resume.
No new DTO/migration is required. The old CLI scan is removed, not retained as fallback.
Workflow waits for the real owner receipt before ending its PM Workflow/runtime;
only the trusted scheduler accepts PM Complete after actual terminal.

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

Heartbeat renews the authoritative lease head and appends its immutable operation,
but emits NO business outbox event (`event_id` is null on that operation). Fresh
readback still advances its server clock/current lease; replay retains the original
receipt. Fleet's metadata-to-chat mirror therefore receives no renewal message.
Existing historical `delivery.lease_renewed` metadata remains readable; stage/run,
proposal, result, feedback and reconciliation events are unchanged. No new message
field, schema or migration is needed to suppress heartbeat chatter.

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
New effects run a Draft/submode/status gate AFTER original-operation replay under
the same task lock: marked tasks require Draft Ready/InProgress and brainstorming
or awaiting_confirmation, with no open structured question (optional included).
Legacy unmarked Draft/Clarification permits absent status/submode, never an open
question or explicit clarification mode; history is not normalized in storage.
After exact owner confirmation, new feedback is 409 even while the PM fence remains
unchanged for its closing Workflow. Original receipts/replay stay available through
confirmation, revision changes and lease expiry, with no duplicate business event.
Focused source tests: app `new_pm_feedback_only_accepts_draft_dialogue_and_preserves_legacy_state`
and the existing infra `pm_resume_typed_inputs_expiry_cas_dedup_atomicity_and_history`
now cover rejection/no side effects, real awaiting-confirmation feedback and
immutable replay/readback. These additions have not been executed by this worker;
Cargo/PG QA stays with parent.
Parent still owns HTTP registration and Fleet delivery of this receipt:
POST `P/pm-feedback` and GET `P/pm-feedback/operations/{key}`, owner-session only.
CLI source calls them through `sdlc pm feedback submit --task UUID --from-file feedback.json`
and `sdlc pm feedback operation --task UUID --key KEY`. They are not live before registration.

Approved continuation read source: GET `P/pm-feedback/messages/{message_id}`
returns the actual immutable `PmFeedbackReceipt`, including original owner text
and hash. Source handler is `sdlc_delivery::feedback_message`; parent adds its
router/lib/OpenAPI registration. `SdlcService::pm_feedback_message(task,actor,id)`
uses explicit `task-tracker:read`, current PM grant/bound subject, fresh active
user/project ACL, latest assignment and saved owner/full identity/fence checks.
The strict central allowlist permits only canonical UUID own-task GET, never a
machine lookup of the human operation key or new POST rights. Generic scheduler
reader is not a holder reader. Saved receipt read requires no live lease and
keeps historical requirement revision/null through later confirmation; it has
no journal/outbox/state/Workflow side effects. No new schema/migration is needed.
Focused source tests: app `pm_feedback_message_reader_requires_read_and_current_bound_pm_grant`,
`pm_feedback_message_readback_preserves_original_input_and_checks_every_binding`;
API `pm_feedback_message_policy_is_exact_task_canonical_uuid_get_only`,
`saved_feedback_handler_contract_is_scoped_get_and_original_receipt`;
existing PM resume PG fixture tests actual saved-message read-only access, revoked
membership/disabled holder, wrong subject/stale grant, no broadened human-key
read, expiry and historical receipt equality without side effects. Not executed
by this worker; parent retains Cargo/PG/live qualification.

The legacy application helper `pm_feedback_resume_contract(state,feedback,checkpoint,lease)` remains unchanged:
exact task/project/root/owner/instance, assignment fence, actual prior terminal,
checkpoint UUID/version and server-observed live lease produce the existing resume
identity `pm-feedback:{message_id}`. It does not dispatch or renew a lease.
Its old checkpoint/feedback wire and stable `pm-feedback:{message_id}` identity are
preserved for compatibility. It does not grant expired-lease renewal or generic rebind.

New typed source methods are `SdlcService::resume_pm_execution_lease(task,actor,command)`
returning `(ExecutionLeaseReceipt,replayed)` and `pm_resume_operation(task,actor,key)`
returning `ExecutionLeaseOperation`. Parent registers POST
`P/pm-draft-execution-lease/resume` and GET `.../resume/operations/{key}` in the
existing controller/OpenAPI. POST requires only configured scheduler subject with
exactly `task-tracker:write`; GET uses its separate exactly-read child token.
Human/PM/other machine callers cannot resume. Both enforce fresh local project ACL;
new effects additionally enforce the holder's current active local project ACL.

`ResumePmExecutionLease` is strict snake_case:
`{expected_owner_version,lease_id,expected_lease_version,input,checkpoint,previous_run_id,previous_run_terminal,idempotency_key}`.
`input` has exactly `{input_kind:answer|feedback|confirmation,input_id:UUID}`,
referencing real Answer.id / PmFeedbackReceipt.message_id / Confirmation.id.
`checkpoint` has exactly `{checkpoint_id,checkpoint_revision,checkpoint_sha256,fence:MachineFence,requirement_revision,content_hash}`;
revision/hash are REQUIRED null/null before publication and exact current values
after publication. Shortnames are frozen for Locke/main coordination. Existing
`PmFeedbackCheckpoint` / resume helper DTOs are not changed.

Before POST, the trusted Fleet consumer reads actual current Workflow checkpoint
and actual old-run terminal via its existing separately credentialed runtime GET
`/internal/runtime/v1/pm/runs/{run}` / callback receipt, validating identity pairs.
Tracker rechecks configured producer, current reserved assignment/execution/fence,
owner/lease CAS, task/project/root/instance/owner and current requirements, then
loads the real input from its own immutable ledger. Answer also matches its exact
question checkpoint/version/fence; confirmation is exact current owner consent.
`previous_run_terminal:ArtifactReceipt` alone is NOT independently verified proof;
Tracker makes no new outbound Fleet call and accepts no caller terminal boolean.

The same lease UUID, claimed_at, holder, execution/assignment/fence survive;
server-clock TTL30 and version+1 commit atomically with input consumption and any
canonical Draft status/issue projection. The existing SQL fragment adds immutable
`sdlc_pm_resume_inputs`; no second migration. Each input, old run and checkpoint
revision is consumed once, and older checkpoint revisions cannot replay as new work.
Only an exact same-transaction journal entry permits expired lease CAS. Ordinary
expired claim/heartbeat still fail. No new outbox/input event, question or answer is
fabricated. Answer/feedback stay Draft (legacy Clarification); confirmation only
closes PM Workflow in Backlog/Analysis, never reopens business writes or completed PM.

Fleet reuses its existing checkpoint/rebind and the same task/chat/execution, with
only a new runtime run after actual old-run terminal. Original operation replay/GET
returns the original receipt and hash, including after completion; fresh clock/current
is the separate existing PM lease GET. Request hash is canonical
`{"operation":"resume_pm_execution_lease","payload":<entire command>}` SHA256.
On 409 CAS, no input is consumed: read fresh lease and actual checkpoint/run before
retrying an unaccepted command. Unknown acceptance first reads the original key's
operation; never blindly change an accepted payload or start a duplicate run.
Existing PM lease GET stays holder/PM-grant scoped: Fleet uses its existing
delegated holder reader for fresh CAS. Scheduler read-only authorizes the new
resume operation GET only; it is not a new generic PM read privilege.

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
Only explicitly marked new PM Draft in an enabled project starts Ready/brainstorming; lease claim is InProgress,
published questions/requirements wait in Ready, owner answer returns PM to InProgress,
and confirmation queues Analysis/Ready. Waiting reason is not a status or runtime proof.

Rollout depends on parent's `SdlcConfig.single_task_project_ids` HashSet, empty by default,
from canonical CSV `TASKTRACKER_SDLC__SINGLE_TASK_PROJECT_IDS`. New EnsureQueued/Reserve
checks it AFTER original replay and BEFORE effects, returning explicit Unavailable when
disabled. Replay, reads/history, heartbeat/proposal/Complete of existing execution remain
available after disable. New canonical PM Draft fields additionally require explicit
creation `flow:"single-task-v1"`; omission never auto-enrolls a legacy task.

### Explicit Draft marker and automatic confirmation snapshot

`CreateDraftCommand.flow:Option<DraftFlow>` accepts only omitted or the exact
`single-task-v1` string. Omission is skipped on serialization, preserving original
create command hashes; explicit null/booleans/unknown strings fail. Marker persists
in `TaskState.retained_fields` / canonical `sdlc_tasks.state`, with immutable/root/
creation-ledger SQL guards in the same existing fragment. It cannot be added later
to an existing Draft, bound task or PM reservation. CreatedDraft/Context are unchanged.

Creation rechecks active ACL, then original key/hash before any new rollout or policy
prerequisite. A new marked effect requires enabled project and real stored seven-role
policy v2 validated by main's existing routing functions. It neither allocates PM
assignment nor freezes policy yet. Initial PM reservation rechecks the current
authoritative v2 PM default for a marked task, without changing old reservations.
Existing Fleet Draft saga owns actual runtime readiness checks via existing SDK
credentials before mutation; no new proof flag/client/force-config is added here.

On marked owner confirmation, existing `freeze_routing` takes current policy in the
same consent transaction regardless of omitted `expected_routing_policy_version`.
Provided version remains an additional CAS. Frozen snapshot/intent/canonical issue
projection/operation/outbox all commit or roll back together under existing project
locking. Replay does not reselect policy. Legacy explicit-version behavior is unchanged;
historical PM assignment is never silently rebound to the snapshot's newer PM route.
SQL additionally denies first delivery enrollment or fresh delivery assignments
without marker; existing historical terminal/readback paths stay available.

Parent integrates the creation DTO schema and Fleet mutation/readiness gate. No API
registration/OpenAPI generated output/CLI changes are included here. Missing policy
returns NotFound, incompatible policy Conflict, disabled new creation Unavailable.
The following execution-reference package closes the fresh Delivery allocator gap;
historical missing references still block technical dispatch and are never invented.

### Delivery execution reference

`DeliveryAssignment.execution:Option<DeliveryExecution>` has exactly
`{ordinal:String,key:String}`. Fresh Reserve always fills it from
`nextval(pg_get_serial_sequence('sdlc_pm_executions','ordinal'))`: the same existing
sequence PM already uses. It is allocated after fresh ACL/original command replay
and existing task CAS/capacity guards, then included in immutable assignment JSON
and assignment hash. Existing lifecycle GET and operation readbacks return it without
a new endpoint or wrapper. Ordinal is positive canonical bigint decimal, key exactly
`SDLC-<ordinal>`; malformed present values fail strict decode/hash validation.

Omission preserves old assignment wire/hash/materialization, including existing
compatibility examples. No historical row/operation is backfilled. New inserts
require a reference, with a JSON shape/range constraint and unique delivery ordinal
index in the same SQL fragment. Caller cannot select a display key. Assignment,
hold, canonical state/issue projection, operation and outbox commit or roll back
together; PostgreSQL sequence gaps on rollback are expected. Original replay does
not allocate again. A new retry execution/fence gets a new key; same PM execution
resume keeps its previous PM key.

Uniqueness is scoped to new-profile PM + general Delivery, including Analyst.
Legacy Analysis still consumes `sdlc_analysis_workflow_task_ordinal` in its old
Workflow namespace; that allocator and old receipts are untouched. Historical
display strings may coincide across families without an execution identity
collision: canonical identity is Tracker instance + execution UUID, and Workflow
key use is namespace/profile scoped. No stopped writers/high-water/setval/cutover
or new sequence is added. Parent owns Fleet DTO/generated registration. Its
technical Workflow builder requires Some execution; absent means blocked, never
taskkey/chatUUID-derived fallback.

Requested execution-reference checks (source tests only, not run here):

```text
cargo test --locked -p app delivery_execution_reference_is_canonical_and_closed
cargo test --locked -p app optional_delivery_execution_preserves_legacy_wire_and_binds_new_hash
cargo test --locked -p infra canonical_projection_heartbeat_race_terminal_recovery_and_disabled_replay -- --ignored --nocapture
```

The existing fresh `_lifecycle_test` PG fixture now checks shared PM/Delivery ordinal
allocation, rejected CAS/auth and replay ACL revocation before allocation, rollback gaps without partial
assignment/hold/operation/outbox, concurrent PM plus same-key Delivery Reserve and original replay,
persisted receipt/readback, new retry key, six role-key uniqueness and unchanged
legacy Analysis sequence. All runtime observations remain synthetic fixtures.

Focused checks requested after integration (Cargo/PG not run by this worker):

```text
cargo test --locked -p app draft_flow_omission_preserves_hash_and_marker_survives_pm_transitions
cargo test --locked -p app marked_draft_requires_bound_seven_role_v2_policy_not_runtime_claims
cargo test --locked -p infra draft_flow_creation_and_atomic_owner_snapshot_preserve_legacy_and_replay -- --ignored --nocapture
```

The new ignored PG source test requires fresh disposable
`TT_DRAFT_FLOW_TEST_DATABASE_URL` ending `_draft_flow_test`. It covers legacy hashes,
explicit flow conflicts, missing/v1/disabled prerequisites, origin/no-retrofit guards,
PM-default binding, exact owner/revision/hash/CAS failures, outbox-failure atomic
rollback, concurrent policy update, snapshot-derived intent and original readbacks
after policy change/disable/ACL revocation. Existing lifecycle/resume PG fixtures now
explicitly opt in with v2 routes; parent's `4e3a527` evidence-scope fix is retained.

Scoped SDLC project authorization always requires an active local user and project owner
or explicit membership. Verified human/global-admin hints do not expand access or directory.
Exact requirements confirmation remains task-owner-session-only; public Tracker legacy
authorization is unchanged except denial of the new assignment-scoped machine grant.

## Parent integration and requested verification

1. Register the SQL fragment `backend/migration/src/single_task_lifecycle.sql`
   once, after 000095, in the migration/order owned by parent. It creates delivery
   heads, immutable assignments/proposals/results/operations, run bindings and
   holds, plus immutable PM feedback and typed PM resume consumption. It does not
   rewrite historical PM rows. Apply the updated fragment before the new lease
   reader (which also validates scheduler resume receipts) is deployed.
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
6. Register the two PM resume methods above and wire typed input, guarded lease
   resume and actual checkpoint/rebind in Fleet/Workflow. Keep
   can_send disabled until durable acceptance and the runtime resume path exist.

Requested single parent QA run, after integration:

```text
cargo test --locked -p app sdlc_lifecycle
cargo test --locked -p app sdlc::tests
cargo test --locked -p app sdlc_pm_resume
cargo test --locked -p domain resume_inputs_are_closed_short_names_and_checkpoint_nulls_are_required
cargo test --locked -p task-tracker-cli --test workflows sdlc_
cargo test --locked -p infra canonical_projection_heartbeat_race_terminal_recovery_and_disabled_replay -- --ignored --nocapture
cargo test --locked -p infra pm_resume_typed_inputs_expiry_cas_dedup_atomicity_and_history -- --ignored --nocapture
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

The PM resume PG source test requires a separate fresh disposable
`TT_PM_RESUME_TEST_DATABASE_URL` ending `_pm_resume_test`. It covers real saved
feedback/answer/confirmation IDs, strict scheduler auth and ACL revocation, actual
server-time expiry, ordinary heartbeat/reclaim denial, rollback on lease update
failure, original historical replay, no input/outbox fabrication, consumed input/run/
checkpoint denial, heartbeat-CAS readback/retry, concurrent same-key dedup, live
heartbeat after resume, requirement supersession, owner-confirmation close without
reopening PM writes, and immutable journal. Runtime artifacts there are synthetic
trusted-scheduler fixtures only, not evidence of live Fleet verification.

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
