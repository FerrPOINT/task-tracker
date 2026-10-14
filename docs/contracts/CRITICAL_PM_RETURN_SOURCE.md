# Critical PM return: Tracker source packet

Baseline: `12934599d02b31fdff70fcece2ff59b1fab2a966`.
Implements the first bounded slice of Fleet's accepted
`docs/contracts/CRITICAL_PM_RETURN_V1.md`. The complete vertical flow remains a
required goal, not phase2. This packet does not expose an endpoint or CLI command.

## Proposed public types

`domain::sdlc_critical_blockers` is intentionally not registered in `lib.rs` yet.
Integration owns public DTO/API/OpenAPI registration and migration ordering.

- `CriticalBlockerKind`: `requirements_contradiction | missing_business_decision`.
- `CriticalBlockerOption`: `{option_id: UUID, label, consequences}`.
- `SubmitCriticalBlocker`: `{fence: DeliveryFence, requirement_revision,
  content_hash, kind, question, rationale, options, recommendation: UUID|null,
  evidence: AttachmentArtifact[], idempotency_key}`.
- `CriticalBlockerReceipt`: blocker UUID/version; instance/project/task/root,
  owner/producer/agent/role; assignment hash/chat/run; routing snapshot/generation;
  immutable proposal/hash; current state; nullable PM review, resolution and actual
  technical terminal; created_at UTC nanoseconds. Assignment/execution come from
  `proposal.fence`, not invented references.
- `CriticalBlockerState`: `proposed | held | awaiting_pm_review | in_pm_review |
  resolved`. `proposed` is internal preparation only, never POST acknowledgement
  or a persisted public effect. POST first returns `held`; return produces
  `awaiting_pm_review`.
- `CriticalPmReviewRef`: `{assignment_id, execution_id}`. Null until a real NEW PM
  allocation. `CriticalResolutionRef`: `{requirement_revision, content_hash,
  routing_generation}`. Null until real new revision/consent/completion effects.
- `CriticalPmReviewGuard`: `{blocker_id, blocker_sha256,
  expected_blocker_version, expected_task_version, fence, idempotency_key}`.
- `ReturnCriticalToPm`: `{review: CriticalPmReviewGuard,
  terminal: RuntimeTerminalObservation}`. The nested review identifies the held
  blocker/current CAS; its key is the return operation's own original key.
  There is no standalone public hold command/API or `RequestCriticalPmReview`.

All request objects are closed. Actor/owner identity is not accepted in requests.
Question/rationale bounds are 4096/8192 UTF-8 bytes; optional 0..8 options with unique
non-nil stable IDs, labels <=256 bytes and consequences <=4096 bytes; 1..16 unique
sealed artifact references. Omitted options deserialize as an empty array; one
option is valid. No fabricated alternatives are required for text-mode PM review.
Recommendation is optional and must reference an existing option ID; it is never
an owner answer, and is invalid when options are empty.
Shape validation cannot decide whether free text is genuinely a business
contradiction; it does reject infra/Rework/ordinary-engineering wire kinds.

## Implemented pure policy

`app::sdlc_critical_blockers` reuses existing strict delivery grant, scheduler auth,
assignment hash, routing validation and canonical JSON SHA helpers. No new deps.

- `prepare_and_hold(current: &LifecycleReceipt, actor: &Principal,
  config: &SdlcConfig, proposal: &SubmitCriticalBlocker,
  allocation: BlockerAllocation) -> Result<CriticalPolicyChange, AppError>`:
  exact opted-in task/root, enabled project, current Running
  assignment/grant/agent/run/owner/routing/revision, live lease held by configured
  scheduler; derives identity from server state and verified actor. Server-only
  `BlockerAllocation` supplies real blocker UUID, persisted current generation and
  observed_at. It is not a wire object or allocator replacement. The CURRENT
  technical grant atomically requests waiting on its own assignment; no scheduler
  principal is constructed or impersonated. Private fresh preparation and shared
  hold logic produce one plan to persist under the SAME task lock. Stage, status,
  runtime, assignment, lease, binding, routing and prior results are unchanged.
- `replay_blocker`: compares canonical typed proposal and hash; returns ORIGINAL
  journal receipt, never replaces it with mutable detail. Caller must first
  perform fresh ACL and original-key authorization.
- `ensure_technical_progress_allowed`: deny progress while held/PM pending/review,
  including fail-closed waiting-reason checks. Call INSIDE existing transactions
  for Complete/Reserve/EnsureQueued and ordinary Reconcile (not just HTTP gates).
  Heartbeat and dedicated matching-terminal return remain possible.
- `return_to_pm`: same scheduler/custody/CAS and exact existing native terminal
  observation; releases technical projection only after terminal identity checks.
  Draft/Ready/runtime Terminal + `critical_pm_review_queued` is deliberately NOT
  technical Ready/Queued. Clears current technical result/lease/binding/resume
  pointers, preserving the old receipt for immutable history and the routing
  snapshot verbatim. Creates neither PM assignment nor owner confirmation.

`CriticalPolicyChange {lifecycle, blocker}` is an internal, non-wire transaction
plan. Inputs are never mutated, including on overflow/failure. Actual Fleet
readback is the existing trusted boundary: receipt-shaped JSON is not a separate
runtime proof. Sealed artifact shape/provenance is not proof of uploaded bytes.

Receipt versions describe persisted public effects, not internal preparation:
POST returns/saves Held v1 and lifecycle version N+1. Scheduler terminal return
produces AwaitingPmReview v2 and increments the current lifecycle version once
(N+2 without intervening versioned operations). The original POST journal
still returns exactly Held v1 after that return; replay rejects a mutable detail
or internal Proposed candidate passed in place of the original Held v1 receipt.
Failed validation/overflow returns
no plan and mutates neither input. There is no acknowledged Proposed window in
which Complete can win after a successful POST.

## Required persistence and integration

No migration, repository trait, router or applied SQL has been changed. Existing
storage cannot safely accept this transition unmodified:

1. Reuse `PostgresSdlcRepository::load` task/project/owner ACL, task lock, existing capacity
   singleton/hold locks, canonical projection permits, delivery operation journal
   and SDLC outbox. POST validates CURRENT technical authority and atomically
   persists proposal + held state + original Held v1 operation receipt under the
   SAME task lock as Complete/Reserve. No ACK before that commit, no deferred
   scheduler hold. If Complete wins the lock first, POST fails fresh validation
   without accepting a proposal; if POST commits first, Complete/Reserve fail
   the held guard. Native terminal must precede capacity release.
2. Add bounded immutable blocker proposal/operation history and current selection
   (names/schema remain integration decisions). Original-key journal stores exact
   operation result; detail exposes current state separately. Do not put blockers
   into typed delivery result/proposal tables or represent them as completed
   Workflow receipts. Changed body ->409, concurrent same-key replay ->same saved
   receipt. Recheck live owner/project and assignment-scoped evidence before write.
   PM evidence reads need an exact current-review grant/assignment plus a match to
   the stored blocker's explicit evidence references and sealed metadata/bytes.
   Ordinary project attachment access and the completed initial PM grant remain
   denied; blocker acceptance must not broaden the existing attachment scopes.
3. Existing `sdlc_delivery_hold_release_gate`, `sdlc_delivery_head_gate` and
   `sdlc_delivery_canonical_gate` in `single_task_lifecycle.sql` currently require
   accepted delivery result/reconciliation. Add narrow blocker-terminal-aware
   branches in a NEW migration; do not manufacture `operation=reconcile` or a
   delivery result to bypass those checks. Persist/archive exact prior head,
   terminal return, projection, capacity release and PM enqueue atomically.
4. `sdlc_delivery_legacy_exclusion` currently rejects new `sdlc_assignments` for
   every existing delivery task, even after terminal release. PM review needs an
   explicit blocker-scoped exception requiring actual terminal, no technical
   custody and current review purpose. Initial PM reservation remains unchanged.
   Reuse PM execution allocator, PM worker/run journal/outbox, same task-bound PM
   chat, fresh assignment/execution/version. No second queue or initial reuse.
5. Keep `sdlc_task_routing_snapshots` row/unique constraint untouched as generation1.
   Proposed additive storage: append-only later generation records keyed by
   `(task_id,generation)`, unique snapshot/confirmation identity, exact revision/
   hash/policy payload; current-generation selection under task CAS. Generation1
   references original snapshot bytes. Later generations default to the frozen
   task policy, NOT current project policy. Replace agents only through separately
   authorized explicit operator replacement/readiness validation.
6. Fresh review-only PM write authorization must retain Analysis freeze for every
   other caller. Existing exact_snapshot, canonical/routing triggers and candidate
   discovery must select current generation explicitly; while PM edits the new
   revision, held prior routing is history, not permission to dispatch.
7. After actual NEW owner confirmation + NEW PM Workflow completion + native
   terminal, atomically select new generation and restart Analysis with fresh
   execution IDs and no old result chain. Old grant, consent replay, callback,
   blocker, routing generation or proposal cannot release that new cycle.

The remainder above is required implementation, not a claimed working fallback.
No live activation is implied by this source packet.

## Qualification handoff

`critical-pm-return-registration.patch` is the proposed TWO module exports for
integration; it is not applied. Without them the tests do not collect. After main
applies registration in a proper source/dependency context, from `backend`:

```text
cargo test --locked -p domain --lib sdlc_critical_blockers::tests:: -- --list
cargo test --locked -p app --lib sdlc_critical_blockers::tests:: -- --list
cargo test --locked -p domain --lib sdlc_critical_blockers::tests::
cargo test --locked -p app --lib sdlc_critical_blockers::tests::
```

Expected nonzero collection: domain 5 tests, app 14 tests. Source formatting and
registration patch applicability only are checked locally; Cargo/PG/Docker are
not executed. Fixtures and native/artifact observations are explicitly synthetic.

Required DB/HTTP negatives still belong to the integration witness: concurrent
hold vs Complete/Reserve; crash/replay during capacity release/PM reserve;
unauthorized evidence/raw bytes; fake native terminal/owner answer; old consent/
grant/result/generation after review; project policy drift; direct SQL bypass of
hold/PM admission; pool2/capacity1; no technical dispatch before NEW PM completion.
