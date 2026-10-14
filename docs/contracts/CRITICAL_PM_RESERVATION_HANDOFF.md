# Critical PM Review Reservation Planner

Signature freeze before implementation, amended after 3173a2b review to support
subsequent completed PM generations. Baseline Tracker 614d252; returned-state
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
    pub previous_reservation: &'a PmDraftReservation,
    pub current_completed: &'a PmDraftCompletionReceipt,
    pub original_pm_assignment: &'a PmAssignment,
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
issue/task lock, with current actor/owner/project ACL. `current_completed` is
the actual CURRENT completion row matching `previous_reservation`, not an older
receipt or caller-supplied receipt-shaped evidence. `original_pm_assignment` is
the frozen first PM assignment loaded from immutable task history, not a current
project-default or caller-selected agent. Allocation
uses server UUIDs and the existing DB execution sequence. The planner cannot
prove persistence, uniqueness, raw artifact bytes or native terminal observations.

This packet starts a PM review after the actual current completed PM execution,
including subsequent reviews of the SAME task. Assignment and owner versions are
arbitrary matching positive bounded current versions; each next version is +1,
never reset to 1. The original concrete PM, task/root/owner and original Draft input
stay unchanged. Blocker moves from persisted AwaitingPmReview at its exact current
CAS version to InPmReview at version +1 with its new PM assignment/execution ref;
the actual positive routing generation is retained, not forced to 1. No result,
terminal receipt or consent is created. Old completion cannot authorize a new
reservation even if its task/owner/agent match the current generation.

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

Dependency before registration/qualification: main `3ddb159` supplies the shared
`domain::sdlc_critical_blockers::valid_operation_key`. The new reservation DTO uses
that exact helper; there is no copied fallback validator. This checkout remains
on baseline614 plus the module packet, so this dependency must be integrated by
main before compiling registered modules.

Authored coverage: 4 domain cases (closed command, bounded CAS, explicit purpose,
shared readable original-key policy) and 9 app cases (same PM/task/input with new IDs and coherent pre-admission state,
scheduler-only/old-grant denial, exact CAS, current completion/original input provenance,
contradictory canonical/return context, allocation uniqueness/task version overflow,
2->3 and 17->18 subsequent reviews, older completion/generation and PM replacement
denial, bounded assignment/owner/blocker overflow).
Fixtures are synthetic and do not claim live evidence. The planner performs no
HTTP, clock/lease renewal, native probing, storage reads or side effects.
