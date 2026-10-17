# Critical PM Review Persistence

Signatures frozen before implementation, baseline main 1b006c8 plus the reviewed
3173a2b/84a02e2 planner. Return source is the accepted ae07b57/674 packet; bc13d55
and 16d881e are transport references only. Superseded b1ca106 is NOT included.

Inherent methods on PostgresSdlcRepository (no shared trait/API edits here):

```rust
pub async fn reserve_critical_pm_review(
    &self, task: Uuid, actor: &Principal, command: ReserveCriticalPmReview,
) -> Result<(CriticalPmReservationResult, bool), AppError>;

pub async fn critical_pm_review_operation(
    &self, task: Uuid, actor: &Principal, key: &str,
) -> Result<CriticalPmReservationResult, AppError>;
```

WRITE uses existing scheduler(actor, config) unchanged. Original-key GET uses
existing scheduler_read(actor, config) unchanged: a separate read child, not
paired READ/WRITE scopes. Both retain current active actor/owner/project access.
Replay compares the exact saved command/hash before allocation or fresh CAS.
Readback is history, not authority to dispatch or mutate the current selector.

Source records come from the locked task/current PM selector and completed PM
row, original first PM assignment/input and actual persisted terminal Return.
The existing sequence allocates the ordinal. One transaction persists journal,
new existing PM assignment/execution rows, canonical pre-admission plan, PM cursor,
unchanged technical terminal execution, and outbox. Initial/held/return records
are not overwritten; no PM lease/native admission/result/consent is inserted.

Main includes critical_pm_review.sql AFTER critical_pm_return.sql in the same
unreleased096 migration. No released migration is changed. Exact journal rows
in the CURRENT transaction permit only the saved before->after state/cursor and
the one saved new PM assignment. Existing consent/canonical projection permits,
capacity exclusions and unrelated write guards remain in force. A deferred gate
requires the complete reservation/projection/outbox before COMMIT.

The companion detail patch changes only the COALESCE read selection to prefer
the saved review after_blocker (InPmReview v3 for the first review), then Return,
then Held. It does not change current detail authorization or original-key Held
history. Main owns applying registrations, migration inclusion, DI/API/codegen
and later PM admission/lease/runtime/context transitions.

New PG tests are children of the existing critical candidates fixture. All
runtime/Workflow proof in that fixture is synthetic, not live qualification.
Tests require a fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending
_candidates_test and the supplied registration/migration patch. No Cargo/Docker
execution is authorized in this packet. The 13 pure planner cases are reused.

Main integration order: retain 3173a2b + 84a02e2, apply the existing pure-module
registration patch, then the new review registration and detail patches. Keep
bc13d55/16d881e transport source separate; do not include b1ca106. The review patch
does not wire a new public route or relax either scheduler credential policy.

Focused qualification commands for main after applying the patches:

```text
cargo test -p domain sdlc_critical_pm_reservation::tests --lib
cargo test -p app sdlc_critical_pm_reservation::tests --lib
cargo test -p infra actual_critical_pm_review_reservation_is_atomic_scoped_and_replayable --lib -- --ignored
```

Each PG invocation needs the existing fresh disposable candidates database fixture; this packet
does not create a DB runner or execute these commands. Authored PG checks include
strict separate credentials, owner/project revocation, stale CAS, unbacked cursor,
journal-only commit rejection, late-write rollback, concurrent duplicate and
original-key restart readback with unchanged sequence, immutable history/current
selector/detail and absence of new leases/native admissions/completions.

This is reservation persistence only. Existing PM edit/admission/runtime/resolution
transitions after InPmReview require main/Locke's subsequent bounded wiring;
current PM-grant detail policy is deliberately not broadened by the read companion.

Packet checks: rustfmt --check for both new Rust files and git apply --check for
both new handoff patches passed on the stated baseline. The shared source files
were not manually edited. Rust compile, the 13 pure tests and the new ignored PG
aggregate were NOT executed; SQL syntax/atomic behavior still need main's actual
PostgreSQL qualification. No Docker/Cargo, runtime mutation or push was performed.
