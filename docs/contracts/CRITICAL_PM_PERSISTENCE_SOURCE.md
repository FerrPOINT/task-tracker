# Critical POST persistence source packet

Successor to `462df3914e8f3cd13f2f33a0dce0141339be20ca`.
Only POST -> atomic Held, original-key replay/detail and technical progress guards
are implemented here. Existing pure ReturnToPm is NOT a persisted operation.
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
   guards, but follows current producer/run/assignment and fresh ACL validation.
   Same body returns exact saved Held v1; changed body409. Readback after heartbeat
   still returns the original fence/lease-version receipt, not a rebuilt one.
3. For a new POST: exact canonical revision/routing, saved assignment/run rows,
   original generation1 snapshot and retained existing capacity row. Current
   technical grant requests its own waiting; scheduler identity is not forged.
4. Existing sealed attachment metadata and author rows are locked. Compare actual
   task/assignment/execution/role/run and exact server seal with every requested
   artifact. Read raw bytes from existing FileStorage; reuse `Seal` and
   `app::attachment_artifacts::verify_bytes`. An unfinished Workflow is not a seal.
5. Recheck the live lease using a fresh DB clock AFTER raw I/O, before any writes.
   Persist blocker/previous head, original Held v1 journal, current selection,
   existing canonical projection/permits, and `critical.blocker_held` SDLC outbox
   event in one transaction. Event ID is the actual allocated blocker UUID.
6. Existing Complete/Reserve/EnsureQueued/Reconcile entry calls the new guard after
   exact_revision under the same locks. Old operation replay is immutable/no-op;
   fresh progression fails while held. Heartbeat remains available. Both existing
   Reconcile dispositions release custody, so neither may bypass a critical hold.

Detail permits actual task owner, current scoped producer, or configured scheduler
READ after fresh project/owner ACL. Old PM grants are denied. Original-key GET is
the current producing child's own key, not a project-wide key search. No evidence
bytes or other task transcripts are returned by the blocker read method.

## Unregistered SQL and registration

`backend/migration/src/critical_pm_return.sql` is ONE new additive fragment, not
an applied migration or edit to 034/096. Main chooses migration identity/order
and namespace registration after existing single-task lifecycle SQL.

Tables: immutable `sdlc_critical_blockers`, `sdlc_critical_blocker_operations`, and
held-only current-selection `sdlc_critical_blocker_heads`. This is not another
business queue. Original routing row/history remains unchanged as generation1.
The slice verifies that actual original snapshot before setting generation1;
later generations require main's authoritative selector and additive storage.

New insert/commit triggers require a current running assignment and atomically
matching held projection, retained capacity, original journal and existing outbox.
Deferred validation rejects a proposal-only commit. Additive projection triggers
permit only the exact waiting/version change, then custody-preserving heartbeat;
they reject unbacked critical waiting. New progress triggers protect assignment,
typed result/proposal, reconcile and capacity release. Existing canonical/consent/
lease/history guards remain installed and unchanged. Empty new tables leave
legacy tasks unaffected. Current heads cannot be mutated/deleted in this hold-only
slice: real PM return needs explicit additive gates, not manufactured reconciliation.

`critical-pm-persistence-registration.patch` proposes ONLY the infra child module
and new PG test child under the EXISTING candidates fixture. It is not applied.
Main also owns trait delegation, storage DI, migration/namespace/API/OpenAPI/CLI
registration and the new outbox event's metadata/mirror handling. No route/CLI is
advertised available by this packet.

## Qualification

No Cargo/Docker/PG execution in the worker checkout. Source format, diff and patch
applicability checks only. The new PG file reuses the existing fresh fixture,
connection/config/owner/PM setup and migrations; it defines no DB runner or pool.
It applies this fragment only if main's migration has not already created it.
All runtime/Workflow observations and the corrupt storage provider are synthetic.

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
and owner/scheduler detail; foreign scope/project member/admin-hint read denied;
changed replay409; waiting-only
projection and generation1 retained; heartbeat permitted; fresh EnsureQueued,
Reserve, Complete and both Reconcile dispositions denied without mutation; direct hold release and
journal rewrite denied; real task-lock POST/Complete race has exactly one winner
and cannot commit an acknowledged proposal without hold.

Remaining required work: main registration/DI and actual SQL/Rust qualification,
Fleet stop/terminal handoff, new task-bound PM purpose/chat, scoped evidence read,
new revision/actual owner consent, append-only routing generations, PM actual
completion/terminal and fresh Analysis. This packet claims none of those effects.
