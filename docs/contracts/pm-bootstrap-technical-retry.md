# Initial PM bootstrap technical retry

Source contract; scoped callback qualification is recorded below. No installed
or live business acceptance is claimed by the reader tests.
The canonical Tracker task, owner, assignment/execution/revision, business input
and Fleet chat remain unchanged. This is NOT a requirements answer, confirmation,
critical review, Workflow completion/checkpoint, or replay of the original native POST.

## Callback Qualification: 22 October 2026

Source cf5a57e0 on SDK2ad passed local app fmt, all-targets check, strict Clippy,
and all13 configuration-reader tests, zero ignored. The tests include the scoped
180s request timeout and unchanged10s ordinary requests. Immutable Git exports,
compiled ELF/listing and actual executed names were verified; own temporary
Compose resources and inputs were cleaned. Result SHA256:
`37fce1b852218fef298941f618e9a628236f3589e8c728c1bf83ba9932d4f70d`.

This is not a new execution of the unchanged producer's PostgreSQL tests or a
server image/install gate. Current main's SDK291/native space admission adds an
independent60s original request clock; merged release must test expiry without
effects and same-key recovery, not silently increase that clock. The13-test
SDK2ad result does not qualify that changed dependency closure.

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
client or integration. The callback-budget candidate gives only this bootstrap
witness request180s (bounded native/Workflow reads); the shared client keeps10s
for configuration and unsubmitted reads, connect timeout3s. The64KiB response
limit, disabled redirects, identity JSON and no-cache checks remain unchanged.
The caller's outer deadline must cover this callback plus preflight/commit.
An unavailable witness returns503 with no
renewal; unknown original-key POST is reconciled by GET, not a new key.

The existing producer is atomic and exact-key idempotent: identical actor/key/full
command may be replayed, including concurrently or after an uncertain response.
Ledger-first readback returns the original receipt before a fresh callback;
otherwise fresh authorization/witness/fencing precede the atomic commit. This
does not prove that an earlier request had no effects, grant dispatch authority,
permit a new successor/key, or authorize deleting a caller's durable marker.
The Fleet consumer owns its saved-command recovery policy separately.

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

## Integrated source checkpoint — 23 October 2026

The candidate includes current main and its pinned SDK and request-commit guards.
All 13 Fleet configuration reader tests pass, including the request-local 180s
budget and unchanged ordinary 10s reads. The exact rebuilt SDLC PostgreSQL HTTP
case covers clarification ownership, replay and restart on an isolated database.
Source bytes remained stable; no installed runtime was changed.

The original-authority expiry/no-effects and same-key recovery matrix still
requires fresh qualification with the combined Fleet caller. A separate native
request-clock target has not run: the 3.5 GiB disk admission guard refused it.
The protocol's 60s authority lifetime is unchanged. Business PM acceptance and
the linked Fleet/Base installation remain open; this checkpoint does not mark
the package release-ready.
