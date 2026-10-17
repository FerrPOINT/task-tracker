# Critical PM review reservation transport (source only)

## Source and ownership

Actual transport baseline: Tracker `caf7ab3`, merged as `1cc5850` into the
isolated critical-pm-reservation checkout. Persistence dependency: `eea0dcf`;
planner dependency: `84a02e2`. Superseded auth relaxation `b1ca106` is absent.
Routes/signatures below were frozen before implementation.

Only four new Rust files and this handoff/companion patch are authored here.
No shared source, registrations, traits, API client, generated schema, migration,
SQL, database or runtime has been changed by this transport packet.

## Frozen transport

POST `/api/v1/issues/{id}/sdlc/critical-blockers/pm-reviews`

Request is the EXISTING closed `ReserveCriticalPmReview` domain DTO:

```json
{
  "blocker_id": "22222222-2222-2222-2222-222222222222",
  "blocker_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "expected_blocker_version": 2,
  "expected_task_version": 9,
  "expected_pm_owner_version": 17,
  "expected_pm_assignment_version": 17,
  "idempotency_key": "critical-pm-review:original"
}
```

All four CAS versions are positive safe integers, not fixed first-review versions.
Task/blocker UUIDs must be nonnil; hash is lowercase SHA-256; the shared domain
operation-key policy applies. No arbitrary PM selector, native receipt, lease,
result or owner confirmation is accepted in this command.

Response: existing `CriticalPmReservationResult`, 201 fresh or 200 exact replay.
`purpose=critical_business_review`, existing PM reservation with original input,
new assignment/execution and next owner/assignment versions, separate blocker and
requirements context. `dispatch_allowed=false` remains enforced by the existing
reservation DTO. No new completion/admission claims.

GET `/api/v1/issues/{id}/sdlc/critical-blockers/pm-reviews/operations/{idempotency_key}`

Response: 200 original saved `CriticalPmReservationResult`. No fresh allocation,
no POST fallback on 404, no new key on unknown acknowledgement. This is historical
operation readback, NOT dispatch authorization and NOT mutable current detail.

Both operations document 401/403/404/409/422/503. Handler success/errors, extractor
errors and strict-auth rejection are `Cache-Control: no-store`. The new sibling
router applies the SAME existing `strict_central_auth`, wrapped by no-store.
The companion patch merges it AFTER the old router's auth layer: each route runs
strict auth once, never legacy bearer/local-token fallback.

## Frozen app and repository signatures

Both new `SdlcService` methods and the existing Postgres inherent methods use:

```rust
async fn reserve_critical_pm_review(
    &self, task: Uuid, actor: &Principal, command: ReserveCriticalPmReview,
) -> Result<(CriticalPmReservationResult, bool), AppError>;

async fn critical_pm_review_operation(
    &self, task: Uuid, actor: &Principal, key: &str,
) -> Result<CriticalPmReservationResult, AppError>;
```

App performs closed command/key/task and machine scope-shape preflight, then
delegates unchanged through `LifecycleRepository`. It cannot authorize a
configured subject by itself. Persistence remains authoritative: existing
`sdlc_reservation::scheduler` for WRITE, `sdlc_lifecycle::scheduler_read` for READ,
exact configured scheduler (not orchestrator/verifier), fresh task/project/owner
ACL before original-key lookup. Paired basic scopes, human and old PM grants are
not accepted. No email gate, introspection loop or auth-policy relaxation.

The repository still owns task locking, original-key replay before allocation,
current completed PM/returned blocker validation and atomic journal/outbox CAS.
This packet adds no queue, runtime activity, initial Draft retry or terminal resume.

## CLI

```text
task-tracker sdlc critical-blocker pm-review reserve --task UUID --from-file PATH|-
task-tracker sdlc critical-blocker pm-review operation --task UUID --key ORIGINAL_KEY
```

Reserve uses the existing JSON-file/stdin input and HTTP client, preserving the
original JSON values/key; it does not regenerate CAS or substitute project defaults.
Closed local deserialization validates before HTTP. The CLI has no production
domain dependency; its validator is checked against the actual domain DTO in a
focused unit test. It reuses the existing CLI readable-key guard (already tested
against the shared domain helper), not a second new key policy. GET encodes the
original key as ONE URL segment with the existing `enc` helper.

One command performs one HTTP request. Unknown/malformed POST acknowledgement is
an error; only a subsequent explicit `operation` command performs GET. No automatic
POST retry, recovery claim or key regeneration. Existing transport error redaction
is retained, and invalid closed payload diagnostics do not echo private fields.

## Main-owned integration

Apply prerequisite planner/persistence registration patches from their accepted
handoffs if not already integrated. Then apply:

`docs/contracts/critical-pm-review-transport-registration.patch`

This exact companion patch (checked against the merged source) contains only:

- New app/API module registrations and strict-router merge.
- Two `LifecycleRepository` default-unavailable methods and Postgres delegations
  to the ALREADY accepted persistence implementation.
- OpenAPI handler/components registrations and `ToSchema` derives/constraints
  on the EXISTING five domain types; no parallel wire DTO/schema.
- CLI nested command registration/dispatch and child HTTP-test registration.

Parent owns applying these changes, genuine OpenAPI/client generation and public
API documentation/indexes. The patch is not applied in this checkout; the new
transport modules are deliberately unregistered until main integrates dependencies.
Preserve prior SQL detail companion: mutable current detail reads saved InPmReview
v3, but original Held/Return/review operations remain immutable history.

## Authored targeted checks (not executed)

Twelve new test functions: app 4, API 4, CLI unit 1, CLI HTTP 3.

- App: original payload/generation, strict read/write/human/old-PM/paired-scope
  rejection, invalid command/key/task, configured-scheduler authority distinction.
- API: two typed authenticated OpenAPI routes/closed schemas, real JSON extractor
  negatives, 201/200/no-store errors, actual missing-bearer middleware rejection.
- CLI: agreement with domain closed validation; real existing fixture HTTP capture
  of original POST, explicit replay and encoded GET; no HTTP on invalid payload;
  denial/redaction and malformed-ACK then GET-only recovery. All data synthetic.

Main's focused invocations after registration, from `backend`:

```text
cargo test -p app sdlc_critical_pm_reviews::tests
cargo test -p api routes::sdlc_critical_pm_reviews::tests
cargo test -p task-tracker-cli --bin task-tracker closed_cli_input_agrees_with_domain
cargo test -p task-tracker-cli --test workflows critical_pm_review_
```

Source-only checks performed here: rustfmt on the four new Rust files,
`git apply --check` for the companion patch, diff whitespace check and ancestry
check excluding `b1ca106`. Cargo, Docker, compile, unit/HTTP execution, PG, OpenAPI
generation and live PM review are NOT performed/claimed. Existing persistence PG
fixtures remain parent qualification, not new transport proof.
