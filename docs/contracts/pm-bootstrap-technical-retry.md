# Initial PM bootstrap technical retry

Source candidate only. No Cargo, PG, HTTP, image or installed acceptance is claimed.
The canonical Tracker task, owner, assignment/execution/revision, business input
and Fleet chat remain unchanged. This is NOT a requirements answer, confirmation,
critical review, Workflow completion/checkpoint, or replay of the original native POST.

## Tracker wire

Scheduler-only POST `/api/v1/issues/{id}/sdlc/pm-draft-execution-lease/retry-bootstrap`:

```json
{
  "expected_owner_version": 1,
  "fence": {"assignment_id": "<UUID>", "execution_id": "<UUID>", "agent_id": "<UUID>", "assignment_version": 1},
  "lease_id": "<original lease UUID>",
  "expected_lease_version": 4,
  "creation_operation_id": "<original operation UUID>",
  "previous_session_run_id": "<same original operation UUID>",
  "next_session_run_id": "<linked attempt UUID>",
  "previous_native_run_ref": "<original native run>",
  "chat_id": "<same chat UUID>",
  "barrier_id": "<existing Fleet durable barrier UUID>",
  "predecessor": {
    "task_id": 1,
    "assignment_operation_key": "pm-draft:<canonical assignment UUID>",
    "assignment_revision": 1,
    "attempt": 1,
    "finalized_binding": {
      "bind_operation_key": "<original Workflow bind key>",
      "bind_request_sha256": "<original canonical request SHA256>",
      "binding_ref": "<original binding reference>",
      "session_run_id": "<previous session-run UUID>",
      "native_run_ref": "<original native run>"
    }
  },
  "idempotency_key": "<immutable original retry key>"
}
```

The nullable `finalized_binding` property is mandatory: exact finalized Present
is accepted; null requires authoritative Absent, never pending/unknown ACK.
The sample is a shape, not executable pins or authorization.

The response is the existing ExecutionLeaseReceipt (201 / exact replay 200),
always `dispatch_allowed=false`. The lease ID, holder and claimedAt are preserved;
only version+1 and DB-clock heartbeat/expiry (TTL30) change. An active lease uses
ordinary heartbeat and is rejected here. GET the original key on unknown ACK:
`.../retry-bootstrap/operations/{idempotency_key}` returns ExecutionLeaseOperation.
POST uses the existing sole scheduler write scope; GET uses scheduler read scope.

## Trusted composite Fleet witness

Tracker uses ONLY its existing configured Fleet reader origin/PAT:

`GET /internal/runtime/v1/pm/runs/{previous_session_run_id}/bootstrap-retry?barrier_id=<UUID>&next_session_run_id=<UUID>`

The Rust source of truth is `domain::sdlc_pm_bootstrap_retry`:

- `contract_version:1`, `observed_at`, exact canonical `identity`, owner and input refs;
- exact owner/lease versions, creation/old/new run IDs, native ref, chat and barrier;
- `terminal: completed|failed|cancelled|stopped` (null/running/unknown denied);
- exact `predecessor`, positive equal `initial_phase_id`/`current_phase_id`,
  `workflow_status:active`;
- `pm_enrollment`, `first_instruction`, `business_result`, `inflight_work` all
  `absent`; `generic_binding` present iff that exact finalized binding exists;
- `terminal_receipt` and `bootstrap_receipt`: existing trusted readback canonical
  reference/SHA256 pairs. No agent uploads or new evidence registry.

Fleet owns native custody validation and its existing durable barrier. It queries
B's authoritative Workflow predecessor reader with its EXISTING assignment
credential, including unknown command/read/bind status. It must not convert
timeout/404/list-empty into an absence proof. Tracker adds no Workflow credential,
client or integration. The existing reader limits remain 10s / 64KiB, redirects
disabled, identity JSON only, no-cache. An unavailable witness returns 503 with no
renewal; unknown original-key POST is reconciled by GET, not a new key.

## Persistence and successor boundary

Application preflight authorizes, checks exact old task/assignment/expired lease,
and returns a saved original-key result BEFORE network readback. HTTP does not
cross a DB transaction. The repository then reacquires task/authorization/lease
locks, rechecks identity/input/current assignment and no accepted PM commands,
and validates a <=30s witness against the DB clock.

One additive 0099 migration reuses `sdlc_pm_lease_operations.recovery`, stores
command/witness/original lease/reservation in that immutable ledger, and consumes
the original execution once (also unique successor run). Ledger insert and exact
expired lease CAS commit atomically. 0098's expired-renewal and deferred atomic
commit gates remain unchanged; its unsubmitted branch remains strict. No tables,
queue, planner, reset, original receipt overwrite or arbitrary lease reacquisition.
Down refuses destructive rollback. Original claim/heartbeat keys keep readback.

Fleet's linked operation and distinct dispatch key must bind this exact request
hash/result; the original creation/run/consent remain immutable. Workflow appends
its immutable local assignment revision2/attempt2 on the SAME local task with an
explicit canonical revision1 -> sink revision2 mapping, then ordinary bind and PM
enrollment require a fresh Running successor. The producer does not fabricate
this mapping or enroll Workflow. Pending/unknown work must reconcile first.

## Authored Qualification (Unrun)

- app `sdlc_pm_bootstrap_retry::tests`: exact finalized/unbound cursor, four
  terminals, no accepted progress, stale/foreign/input/fence/lease/receipt rejection,
  scheduler-only, closed DTO, historical replay and changed payload.
- app `sdlc_configuration::tests::bootstrap_retry_readback_uses_only_existing_fleet_origin_reader_token_and_exact_barrier`:
  local HTTP fake, exact URL/PAT and redirect/403/503/non-witness rejection.
- api `tests::bootstrap_retry_schema_is_closed_and_contains_no_caller_proof`.
- infra filter `bootstrap_retry_0099_appends_after_0098_and_refuses_destructive_down`.
- ignored infra filter `bootstrap_retry_populated_history_concurrency_bad_witness_and_lost_ack`:
  existing disposable `TT_CANDIDATES_TEST_DATABASE_URL` fixture, populated0098->0099,
  nonzero claim/heartbeat history, raw expired update rejection, same-key parallel
  replay, competing successor CAS, lost ACK and old task/issue/outbox/claim readback.

Registration is authored locally. Main reviews migration order and generates the
OpenAPI/client snapshots; generated files are deliberately not fabricated here.
