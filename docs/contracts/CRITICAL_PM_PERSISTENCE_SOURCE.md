# Critical hold and terminal return persistence source packet

Return successor to `96567089945a055778d94fa8736cbc6efca8a899` (POST persistence).
POST -> atomic Held, scheduler terminal ReturnToPm, original-key replay/detail
and technical progress guards are implemented in source here.
The full critical PM return remains required; this packet is not live qualification.

## Existing trait delegates

Main owns `LifecycleRepository`, app/API/CLI registration and storage DI. Its agreed
three signatures are unchanged:

```rust
async fn submit_critical_blocker(&self, task: Uuid, actor: &Principal,
    command: SubmitCriticalBlocker) -> Result<(CriticalBlockerReceipt, bool), AppError>;
async fn critical_blocker(&self, task: Uuid, actor: &Principal,
    blocker_id: Uuid) -> Result<CriticalBlockerReceipt, AppError>;
async fn critical_blocker_operation(&self, task: Uuid, actor: &Principal,
    key: &str) -> Result<CriticalBlockerReceipt, AppError>;
```

This packet implements inherent methods in the new child module
`backend/infra/src/sdlc_critical_blockers.rs`. Read methods have those exact
signatures. The write delegate uses:

```rust
pub async fn submit_critical_blocker_with_storage(
    &self, task: Uuid, actor: &Principal, command: SubmitCriticalBlocker,
    storage: &dyn domain::FileStorage,
) -> Result<(CriticalBlockerReceipt, bool), AppError>
```

The repository currently has only db/config. Main must supply the EXISTING
attachment storage through its production DI and fail closed Unavailable when
absent. No new storage, caller verification flag, owner principal or terminal
proof is introduced. Trait fakes may retain main's default Unavailable methods.
Use explicit inherent/UFCS delegation to avoid recursive trait method calls.

## Actual transaction

1. Existing producer grant validation, `load` issue/task/project/account locks and
   fresh owner active/project membership checks. No Central effective-admin bypass.
2. Original-key lookup scoped by `(task,actor,key)` precedes fresh-write/lease
   guards, but follows producer/run/assignment and fresh ACL validation.
   Same body returns exact saved Held v1; changed body409. Readback after heartbeat
   still returns the original fence/lease-version receipt, not a rebuilt one.
3. For a new POST: exact canonical revision/routing, saved assignment/run rows,
   original generation1 snapshot and retained existing capacity row. Current
   technical grant requests its own waiting; scheduler identity is not forged.
   Nonrenewing proposal fencing permits a positive frozen startup lease version
   <= current for the SAME lease/assignment/execution/fencing token. Future
   versions or different custody are denied. Original payload/hash is not rewritten
   after heartbeat. Main's app-only current_custody refinement is a prerequisite;
   scheduler terminal return still requires the EXACT current lease version.
4. Existing sealed attachment metadata and author rows are locked. Compare actual
   task/assignment/execution/role/run and exact server seal with every requested
   artifact. Read raw bytes from existing FileStorage; reuse `Seal` and
   `app::attachment_artifacts::verify_bytes`. An unfinished Workflow is not a seal.
5. Recheck the live lease using a fresh DB clock AFTER raw I/O, before any writes.
   Persist blocker/previous head, original Held v1 journal, current selection,
   existing canonical projection/permits, and `critical.blocker_held` SDLC outbox
   event in one transaction. Event ID is the actual allocated blocker UUID.
6. Fresh submit_delivery_proposal and Complete/Reserve/EnsureQueued/Reconcile call the new guard after
   exact_revision under the same locks. Old operation replay is immutable/no-op;
   fresh progression fails while held. Heartbeat remains available. Both existing
   Reconcile dispositions release custody, so neither may bypass a critical hold.

These are Tracker business-write fences, not a filesystem/native runtime barrier.
The required Fleet native stop/readback watcher is not implemented by these guards.

Detail permits actual task owner, current scoped producer, or configured scheduler
READ after fresh project/owner ACL. Old PM grants are denied. Original-key GET is
the original producing child's own key, not a project-wide key search. After Return
only its original-key recovery may use exact archived custody; fresh detail and
writes cannot use that archive. No evidence
bytes or other task transcripts are returned by the blocker read method.

## Persisted scheduler terminal Return

Proposed INHERENT signature, for main to freeze before exposing any shared trait/API:

```rust
pub async fn return_critical_to_pm_with_terminal(
    &self, task: Uuid, actor: &Principal, command: ReturnCriticalToPm,
) -> Result<(app::sdlc_critical_blockers::CriticalPolicyChange, bool), AppError>
```

`CriticalPolicyChange` is the EXISTING non-wire internal `{lifecycle, blocker}`
plan; bool means original operation replay. No new public DTO/trait registration
or endpoint is made here. Original operation is persisted under `(task,actor,key)`
with exact command/hash and both result components, not a new generic journal.
Existing `ReturnCriticalToPm` carries `review` (blocker/hash/version/task-version,
DeliveryFence, key) and the closed `RuntimeTerminalObservation`.

1. Only the configured scheduler WRITE principal may enter. Existing task/project/
   issue locks and fresh owner ACL apply, including replay. No producer impersonates
   a scheduler and no caller supplies an observed owner/holder.
2. Original return key/body replays the saved plan; changed body409. New operation
   requires current canonical revision, original Held v1, exact current task/blocker
   and lease version, same assignment/execution/fencing token/lease and recorded run.
   Heartbeat and Return serialize on the same task lock. Unlike the producer's
   nonrenewing startup fence, Return must refresh its exact current lease CAS.
3. The existing pure `return_to_pm` validates the closed terminal identity. Trusted
   Fleet MUST have actually read native terminal before calling; a receipt-shaped
   string, unknown state, stop ACK or expiry alone is not proof. This repository
   does not introduce a terminal verifier service or fetch arbitrary receipt URLs.
4. Persist an append-only `sdlc_critical_pm_returns` row containing the exact actual
   old head/binding/lease, original command/hash, new lifecycle and Awaiting v2.
   Lock/delete ONLY the old assignment's capacity row. Reuse canonical projection,
   permits, mapped issue status and `critical.returned_to_pm` outbox in one transaction.
   The allocated return UUID is the existing outbox event ID.
5. The new canonical state is Draft/Ready, runtime terminal, waiting
   `critical_pm_review_queued`; assignment/lease/binding/result and old continuation
   refs are cleared. Original routing generation1, owner/root, cycle/retry counters,
   assignment/run history, Held v1 journal and typed results remain unchanged.
   Current blocker detail reads AwaitingPmReview v2 with actual terminal observation;
   PM review/resolution refs remain null. Original Held GET/POST replay is still v1.

No result/reconciliation is manufactured to release capacity. Awaiting state still
blocks ordinary technical progression. This packet does NOT reserve a new PM run,
reuse the completed initial PM assignment, publish requirements or advance routing.
Main's app heartbeat refinement `83a71c4` and narrow dot-key rejection are required
when integrating; no global legacy key validator/body/hash rewrite is introduced here.

## Unregistered SQL and registration

`backend/migration/src/critical_pm_return.sql` is ONE new additive fragment, not
an applied migration. Authoritative parent READ ONLY inspection: accepted sdlc1
Tracker DB has last migration0033 (19 rows), no096; Git main also has no096.
PR152 owns ONE unreleased096. Parent includes `single_task_lifecycle.sql` THEN
`critical_pm_return.sql` in ONE atomic096 execution. No097 and no edits to released
034..095. An old unreleased hold-only096 candidate is NOT an accepted upgrade baseline.
These are parent-reported/source facts, not a worker production migration PASS.

Tables: immutable `sdlc_critical_blockers`, `sdlc_critical_blocker_operations`, and
current-selection `sdlc_critical_blocker_heads`, plus the immutable Return journal/
old-head archive `sdlc_critical_pm_returns`. This is not another
business queue. Original routing row/history remains unchanged as generation1.
The slice verifies that actual original snapshot before setting generation1;
later generations require main's authoritative selector and additive storage.

New insert/commit triggers require a current running assignment and atomically
matching held projection, retained capacity, original journal and existing outbox.
Deferred validation rejects a proposal-only commit. Additive projection triggers
permit only the exact waiting/version change, then custody-preserving heartbeat;
they reject unbacked critical waiting. New progress triggers protect assignment,
typed result/proposal, reconcile and capacity release. Existing canonical/consent/
lease/history guards remain installed. Only three existing functions are replaced
in this NEW fragment with narrow exact-journal branches: capacity release, delivery
head and canonical state. Original permit/owner-consent/lease checks and all ordinary
result/reconcile paths are retained. New Return insert/commit gates enforce exact
before/after state, current lease CAS, retained original routing and actual atomic
release/projection/outbox. Journal-only commit and unrelated state edits are denied.
Empty new tables leave legacy paths unchanged. Current blocker selection and original
Held history cannot be mutated/deleted; later PM transitions need main's explicit
authorization/storage integration, not disabling these guards.

`critical-pm-persistence-registration.patch` proposes ONLY the infra child module
and new PG test child under the EXISTING candidates fixture. It is not applied.
Main also owns trait delegation, storage DI, migration/namespace/API/OpenAPI/CLI
registration and the new outbox event's metadata/mirror handling. No route/CLI is
advertised available by this packet.

## Qualification

No Cargo/Docker/PG execution in the worker checkout. Source format, diff and patch
applicability checks only. Static comparison of all three replaced lifecycle
functions also confirms their original bodies are unchanged except the exact
critical-return branches; this is NOT SQL execution. The new PG file reuses the existing fresh fixture,
connection/config/owner/PM setup and migrations; it defines no DB runner or pool.
It applies this fragment only if main's migration has not already created it.
All runtime/Workflow observations and the corrupt storage provider are synthetic.
The fixture fails explicitly if it sees an old hold-only fragment without Return;
it does not silently upgrade an unreleased candidate schema. This fresh-fixture
test is NOT the required populated0033 -> latest upgrade test, which main must run
against the accepted baseline in disposable QA.

After module registration, from `backend` main must FIRST verify nonzero collection:

```text
cargo test --locked -p infra --lib sdlc::candidates::tests::critical_blocker_tests:: -- --list --ignored
```

Expected: ONE aggregate PG test named
`sdlc::candidates::tests::critical_blocker_tests::actual_critical_post_is_atomic_replayed_authorized_held_and_race_fenced`.
Execute that exact selector with `--ignored --exact --test-threads=1` only in the
existing main-owned disposable QA fixture. Environment is
`TT_CANDIDATES_TEST_DATABASE_URL`; DB name must end `_candidates_test` and start
without `public.seaql_migrations`, enforced by the existing fixture.

Authored assertions: direct proposal-only commit and unbacked waiting denied; corrupt raw bytes/hash/
invalid recommendation/owner impersonation create no effects; late SQL failure
rolls back all writes; concurrent exact POST gives one saved receipt; original key
and owner/scheduler detail; pre-POST heartbeat with unchanged startup body/hash,
future lease version/new lease/new fencing token denied; foreign scope/project member/admin-hint read denied;
changed replay409; waiting-only
projection and generation1 retained; heartbeat permitted; fresh EnsureQueued,
Reserve, fresh normal completion proposal, Complete and both Reconcile dispositions
denied without mutation; pre-hold completion proposal replay remains unchanged; direct hold release and
journal rewrite denied; real task-lock POST/Complete race has exactly one winner
and cannot commit an acknowledged proposal without hold.

Same nonzero aggregate selector also exercises Return: producer/owner/reader denied;
stale task/blocker/lease CAS, future lease/new lease/fence/run/hash, empty receipt and
unknown/nonterminal DTOs denied without changes; unbacked Draft and journal-only
commit denied; direct SQL rejects unknown terminal, stale CAS, unrelated state and
dot operation keys (local new-table checks only); late outbox failure restores capacity/canonical/issue/journal; concurrent
duplicate Return gives exactly one winner and one identical replay; old head archived
verbatim, own capacity released, another task untouched; Draft/Ready/Terminal and
Awaiting v2 detail exact; original Held v1 read/POST replay survives release without
artifact I/O; changed-key body409, second return/fresh technical progress denied;
Return journal update/delete denied; no typed result/reconcile/assignment fabricated.

Remaining required work: main registration/DI and actual SQL/Rust qualification,
public Return contract/wiring and Fleet actual stop/terminal handoff, new task-bound PM purpose/chat, scoped evidence read,
new revision/actual owner consent, append-only routing generations, PM actual
completion/terminal and fresh Analysis. This packet claims none of those effects.
