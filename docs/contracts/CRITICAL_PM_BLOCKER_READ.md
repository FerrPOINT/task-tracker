# Current PM critical blocker read (source only)

Baseline: Tracker `8ac333a`, merged as `c76bd65`. Main's schema derives were retained
when resolving the old planner add/add conflict. No superseded `b1ca106` dependency.

## Coordinated scope

Production edits are ONLY a companion patch, not applied here:

`docs/contracts/critical-pm-blocker-read.patch`

- `app/src/sdlc_critical_blockers.rs`: canonical detail GET shape and pure current
  review/PM/fence/lease authorization helper, plus dedicated test registration.
- `infra/src/sdlc_critical_blockers.rs`: replace blanket PM denial ONLY for detail
  with same-transaction current-review verification. Other callers/history unchanged.
- `api/src/middleware/sdlc_auth.rs`: allow ONLY canonical blocker detail GET for
  the exact PM task and existing basic READ scope. Dedicated policy test registration.
- Existing critical PG fixture: register a NEW child test file; no existing test rewrite.

No lifecycle/routing/SQL guard, schema, trait, registration outside these hooks,
API client, controller, endpoint, attachment reader or credential changes.
Main/Locke retain lifecycle/routing/persistence ownership. Frozen QA source is untouched.

## Existing route, narrow authority

GET `/api/v1/issues/{task}/sdlc/critical-blockers/{blocker_id}` remains the ONLY route.
The strict Central middleware still verifies the bearer/service scope and exact
PM task grant before entering the existing handler/service/repository. Shape
allowance is NOT authorization to any supplied blocker UUID.

The repository's EXISTING task-locked transaction does all of the following:

1. `load_read`: fresh active actor/project ACL; root/task scope; exact current PM
   assignment grant and latest persisted assignment. Existing mixed delivery-grant
   checks remain; old PM grants fail before reading the requested payload.
2. Existing `owner_access`: fresh active owner and project ownership/membership.
3. Existing current-head detail query; the requested blocker must be saved
   `InPmReview`, not Held/Awaiting/Resolved, and linked to the current assignment
   AND execution in the actual immutable `sdlc_critical_pm_reviews` row.
4. Immutable journal `after_blocker` must equal returned detail; journal reservation
   must equal existing `PostgresSdlcRepository::reservation`, including original
   input and actual owner/assignment versions. Current PM cursor/execution must
   match, with existing admission state `reserved`.
5. Existing `lease_current` validates actual retained lease operation/row, holder,
   TTL and owner/fence. A final DB `clock` observation requires claimed <= heartbeat
   <= observed < expiry, same holder and exact assignment/execution/agent/version.

No stage/status/submode is invented as an additional read gate. Requirements
context is the original blocker context: subsequent PM edits must not make the
original question unreadable merely by adding a new requirements revision.
Unclaimed/expired/missing/inconsistent lease or selector fails closed. No fallback
to old PM execution, scheduler impersonation, project-default PM or archived grant.

The helper signatures in the companion patch are:

```rust
pub fn is_pm_critical_blocker_read(method: &str, parts: &[&str]) -> bool;

pub fn authorize_critical_pm_blocker_read(
    state: &TaskState, actor: &Principal, blocker: &CriticalBlockerReceipt,
    review: &CriticalPmReservationResult, current: &PmDraftReservation,
    lease: &ExecutionLeaseReceipt, observed_at: DateTime<Utc>,
) -> Result<(), AppError>;
```

Only the existing repository calls the pure helper after same-transaction source
verification. No new ports or persisted structures. Reads do not mutate PM/Tracker
state, lease, journal or outbox. Human owner/scheduler/technical detail behavior and
all original-key operation readers remain unchanged. PM receives no operation
history, list/read-all, return/reservation write or dispatch capability.

## Sealed evidence: demonstrated gap, no access widened

At this baseline `api/middleware/attachment_auth.rs::attachment_auth` explicitly
denies every PM grant. `infra/attachment_artifacts.rs::authority` accepts current
technical delivery or scheduler READ authority and needs a technical assignment
and binding; Return has deliberately cleared both. Its `allowed` function grants
only own current technical artifacts or exact accepted predecessor output, and
scheduler proposal artifacts. Merely allowing a PM URL in middleware would not
authorize these bytes and must NOT become general attachment access.

Blocker POST already checks `proposal.evidence` against actual attachment task,
producer, seal/run/binding and raw SHA via `verify_evidence`. The detail response
returns these existing typed references, not raw bytes. This patch does NOT add
PM list/download or open ordinary attachments.

Minimal future contract (needs separately approved existing-reader hooks, no new
credentials/schema/endpoint): ONLY the existing single attachment download may
accept current PM READ authority after the SAME current review/owner/project/lease
check. The requested attachment must equal an exact item in that saved blocker's
`proposal.evidence` (reference, SHA and producer binding); validate the existing
seal/provenance against the retained technical assignment/run and task; verify
raw bytes with the existing FileStorage/`verify_bytes` before returning. No current
technical assignment requirement for that narrow archived-evidence case, no
ordinary attachment list/upload/delete, arbitrary attachment ID or old PM access.
It must be rechecked before returning bytes after storage I/O if authority expiry
or revocation can occur during the download. This permission is NOT implemented
or claimed available here; metadata detail read alone is not evidence-download proof.

## Dedicated authored regressions

Seven new test functions, not executed:

- App (5): current review in all three Draft submodes, no mutation; old/foreign PM
  task/agent/execution/version and missing READ denial; wrong blocker/journal context;
  expiry/future clock/holder/owner/every fence component; canonical GET-only routes.
- API policy (1): exact-task canonical detail GET; deny writes/history/operations,
  foreign task, ambiguous/human grant and attachment routes.
- Existing PG fixture child (1 ignored): real initial completed PM -> actual
  Hold/Return/review journal -> current PM lease claim -> detail read. Denies old PM,
  unclaimed review, foreign blocker/task, operation history, revoked actor project
  access, inactive owner and owner project access. No state/outbox/lease mutations
  by reads. Native/Workflow observations are synthetic; no live runtime proof.

After main applies the companion patch, targeted commands from `backend`:

```text
cargo test -p app sdlc_critical_blockers::pm_blocker_read_tests
cargo test -p api middleware::sdlc_auth::critical_pm_read_tests
cargo test -p infra actual_current_pm_blocker_read_requires_saved_review_fresh_acl_and_current_lease -- --ignored
```

The PG command requires a fresh disposable candidates fixture DB with the current
unreleased migrations, under main's existing protected runner/cleanup. Lease
expiry/fence negatives are pure tests, not claims of PG clock advancement.

Qualification here: rustfmt on the three new Rust test files, companion
`git apply --check`, staged whitespace and forbidden-ancestor check only.
No Cargo/Docker, compile, pure/API/PG execution, OpenAPI or live proof. Main must
apply/review the patch and run affected qualification; it is not applied to the
working production hooks in this checkout.
