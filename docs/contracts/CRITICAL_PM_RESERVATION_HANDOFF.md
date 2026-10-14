# Critical PM Review Reservation Planner

Signature freeze before implementation. Baseline Tracker 614d252; returned-state
semantics are from the separately reviewed 674f8c7. Only new domain/app modules
and this handoff/registration patch belong to this packet. No repository, SQL,
API, queue, native dispatch or shared registration changes.

## Frozen Types

`domain::sdlc_critical_pm_reservation`:

- `ReserveCriticalPmReview { blocker_id: Uuid, blocker_sha256: String,
  expected_blocker_version: i64, expected_task_version: i64,
  expected_pm_owner_version: i64, expected_pm_assignment_version: i64,
  idempotency_key: String }`.
- `CriticalPmReservationPurpose::CriticalBusinessReview`, serialized as
  `critical_business_review`.
- `CriticalPmRequirementsRef { revision: i64, content_hash: String }`.
- `CriticalPmReviewContext { blocker_id: Uuid, blocker_sha256: String,
  blocker_version: i64, requirements: CriticalPmRequirementsRef }`.
- `CriticalPmReservationResult { purpose: CriticalPmReservationPurpose,
  reservation: PmDraftReservation, context: CriticalPmReviewContext }`.

`app::sdlc_critical_pm_reservation`:

```rust
pub struct CriticalPmReservationSource<'a> {
    pub state: &'a TaskState,
    pub lifecycle: &'a LifecycleReceipt,
    pub blocker: &'a CriticalBlockerReceipt,
    pub initial_reservation: &'a PmDraftReservation,
    pub initial_completion: &'a PmDraftCompletionReceipt,
    pub original_input: &'a PmDraftInputResponse,
    pub pm_owner_version: i64,
    pub pm_admission_state: &'a str,
}

pub struct CriticalPmReservationAllocation {
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    pub ordinal: i64,
}

pub struct CriticalPmReservationPlan {
    pub state: TaskState,
    pub lifecycle: LifecycleReceipt,
    pub blocker: CriticalBlockerReceipt,
    pub result: CriticalPmReservationResult,
}

pub fn prepare_critical_pm_reservation(
    source: CriticalPmReservationSource<'_>,
    actor: &Principal,
    config: &SdlcConfig,
    command: &ReserveCriticalPmReview,
    allocation: CriticalPmReservationAllocation,
) -> Result<CriticalPmReservationPlan, AppError>;
```

## Persistence Boundary

All source arguments must be actual persisted records loaded under the existing
issue/task lock, with current actor/owner/project ACL. `initial_completion` is
the actual completion row, not caller-supplied receipt-shaped evidence. Allocation
uses server UUIDs and the existing DB execution sequence. The planner cannot
prove persistence, uniqueness, raw artifact bytes or native terminal observations.

This packet starts the first PM review after an actual completed initial PM
execution. Initial assignment/owner versions are 1; new versions are 2. The
original concrete PM, task/root/owner and original Draft input stay unchanged.
Blocker moves from persisted AwaitingPmReview v2 to InPmReview v3 with its new PM
assignment/execution reference. No result, terminal receipt or consent is created.

Canonical state stays Draft/Ready with the returned technical terminal cursor,
no technical assignment/lease/binding/result. Reservation is NOT runtime admission.
Task version advances once; submode becomes existing Brainstorming; the current
confirmation marker is cleared, but revisions/confirmations remain immutable
history. Existing canonical projection construction is reused, not bypassed.
The actual PM running projection/admission is a subsequent main-owned operation.

Existing SQL cannot persist this plan yet: migration 0095 pins PM owner version
to 1 and forbids replacing PM execution/assignment; 0096/critical guards also need
the exact review transition. Main/Locke must add a journal-backed narrow exception,
not reset initial reservation guards or mutate historical migration receipts.

Persist the reservation, next canonical/technical projections, blocker detail,
original command hash/result and outbox atomically. Replay must compare the exact
original key/body and return its saved result BEFORE allocating IDs/ordinal or
checking fresh CAS; changed body is conflict. The pure planner only handles fresh
planning, never journal lookup or retry/dispatch. `dispatch_allowed` stays false.

Fleet must select this new execution for credentials/lease/tools/completion,
reuse the original task+concrete-PM chat, and never repeat CreateDraft or resume
the completed initial run. The nested PmDraftReservation retains its existing
variant for storage compatibility; the outer typed purpose distinguishes review
and must never be unwrapped into the initial saga.

## Qualification

Focused pure tests are authored in the two new modules; Cargo/Docker are not run
in this packet. Apply the handoff registration patch in main before qualification.
No claim of persisted or live review admission is made.

Main-owned focused commands after registration:

```text
cargo test -p domain sdlc_critical_pm_reservation::tests --lib
cargo test -p app sdlc_critical_pm_reservation::tests --lib
```

Authored coverage: 3 domain cases (closed command, bounded CAS, explicit purpose)
and 6 app cases (same PM/task/input with new IDs and coherent pre-admission state,
scheduler-only/old-grant denial, exact CAS, initial completion/input provenance,
contradictory canonical/return context, allocation uniqueness/version overflow).
Fixtures are synthetic and do not claim live evidence. The planner performs no
HTTP, clock/lease renewal, native probing, storage reads or side effects.
