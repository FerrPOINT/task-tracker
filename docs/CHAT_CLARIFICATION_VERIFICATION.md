# Tracker Clarification Verification

Historical baseline date: 1 October 2026. Later sections record completed
sidecar/follow-up checks, including the 2 October input snapshot release.
Scope is Tracker only; no sibling repo or production UI edits, merge, deployment
or real PM acceptance. Earlier baseline warnings/skips are historical, not the
current follow-up gate status.

The original clarification report below records its earlier snapshot. The
Human Draft Sidecar section records the final creation-sidecar verification.

## Baselines And Environment

- Tracker start: `569c1b9249aed1d552a5d3444c92f73dde94691d`.
- Read-only shared Base checkout: `e17525b95921a6a5db7fa3274ea4fcf611787c8a`.
- Linux Docker Rust 1.91.1, PostgreSQL 17, isolated disposable databases.
- Tracker mounted at `/work/task-tracker`; Base mounted read-only at
  `/work/services-base`. All source changes are confined to Tracker.
- Windows `cargo +1.91.0 fmt` cannot resolve the absent sibling Base path in
  this isolated checkout. Linux mounts supply it without modifying a sibling.

## Gates

- PASS: `cargo fmt -- --check` (own workspace only; Base read-only).
- PASS: `cargo check --workspace --all-targets --locked`.
- PASS: `cargo test --workspace --locked -- --test-threads=4`:
  503 reported passed, 21 ignored, zero failed. Includes five SDLC application
  tests and the flat strict revision OpenAPI schema test. The legacy
  `central_subject` migration test returns early without its dedicated database
  environment variable; its reported pass is not PostgreSQL evidence.
- PASS, separately against PostgreSQL 17: `cargo test -p server --test sdlc
  --locked -- --ignored --nocapture`, with `TT_SDLC_TEST_DATABASE_URL` pointing
  only to a disposable Tracker test database: one compound real HTTP/SQL test.
- PASS: `cargo clippy --workspace --all-targets --locked`, with six existing
  `collapsible_if` warnings in unchanged domain repositories and application
  comment/report/sprint services. No warnings in new SDLC files. Strict
  `cargo clippy --workspace --all-targets --locked -- -D warnings` fails at the
  existing `domain/src/repositories.rs:288`; strict warning-clean is not claimed.
- PASS: `cargo run -p api --bin gen-openapi --locked` regenerated OpenAPI;
  13 SDLC paths, flat strict `RequirementsRevision`, revision maximum
  9007199254740991, and all schema references resolve.
- PASS: `git diff --check`.

The PostgreSQL test applies all migrations and exercises actual server wiring
with a synthetic ES256 Central Auth issuer/JWKS and session/PAT responses:

- Local/expired/wrong-audience bearer rejection and unavailable Auth failure.
- Strict membership despite the legacy central bypass, including a global-admin
  nonmember and membership revocation; cross-project root rejection.
- Reporter-derived owner binding, stable instance mismatch rejection, human-only
  owner consent, no operator consent and no caller-supplied author.
- Assigned machine publication, insufficient service scope rejection, strict
  DTOs and unsafe integer rejection; single/multiple/text/custom option answers,
  stale question versions, cancellation and mandatory-question publication gate.
- Full flat requirements detail/diff, a new final revision after answers, exact
  hash/revision evidence from a distinct configured verifier, and owner-only
  confirmation committing Backlog. Direct SQL and legacy HTTP status changes
  cannot bypass confirmation; immutable history rejects UPDATE/DELETE.
- Injected answer-outbox failure rolls back business inserts. Eight concurrent
  identical answers produce one durable result; changed payload replay returns
  409. Tracker restart preserves answer/confirmation IDs/time and outbox records.

This is isolated backend integration evidence, not live Central Auth, Fleet,
Workflow, verifier execution or browser acceptance. The 18 legacy ignored infra
database tests and two legacy server smoke tests were not explicitly run here.
No production migration, coverage measurement or Windows-linked binary test was
performed. Earlier interrupted runs and failed fixture attempts do not count as
successful gates. Test infrastructure uses only exact Tracker-owned container
names; no Docker prune or broad cleanup and no Fleet container operations.

## Remaining Integration Gaps

- Real Central Auth credentials/scopes, exact stable Fleet instance match,
  and live project memberships have not been accepted on a deployed stack.
- Fleet must provision/persist the current PM assignment and its scopes,
  dispatch structured PM tool calls, persist the inbox/cursor and project events.
- Independent readiness verifier is required. Tracker stores exact trusted
  attestations; the external verifier must execute real checks before submitting.
  Missing verifier configuration fails confirmation closed.
- Workflow safe-stop/checkpoint/rebind/resume and uncertain runtime acceptance
  readback remain external. Tracker has no runtime dispatch or delivery worker.
- Real PM question -> owner answers -> final document -> exact confirmation ->
  Backlog, browser acceptance and restart of all services are not live accepted.
- No frontend checks/screenshots or whole-stack Compose deployment were run;
  the task explicitly excludes UI and sibling changes.

## Traceability

Strict auth/owner identity: `api/middleware/sdlc_auth.rs`, repository SQL access.
Versions/modes/stale fences: `domain/sdlc.rs`, `app/sdlc.rs`, app unit tests.
Full content/hash/exact consent/evidence: revision DTOs and command application.
Durability/idempotency/outbox/legacy gate: migration 000034, PostgreSQL repository
and server HTTP integration test. Exact Fleet DTOs: [contract](CHAT_CLARIFICATION_CONTRACT.md).

## Human Draft Sidecar (1 October 2026)

Baseline: `0cffd7d8c5fe5d9b12776c226635780beb583a4f`, branch
`feat/pm-clarification`. Scope is Tracker only; no Base/Fleet/Workflow writes.
Pending migration 000034 was extended, with no second migration file and no
shared/accepted database migration or reset. Wire is locked in the contract:
`POST /api/v1/projects/{project_id}/sdlc/drafts`, strict human request and
seven-field `CreatedDraft`, 201 new / 200 replay / 409 changed payload.

Final source gates (after outbox/schema/operation-ID changes):

- PASS: Rust 1.88 `cargo fmt --all -- --check`.
- PASS: Rust 1.88 `cargo check --workspace --all-targets --locked`.
- PASS: Rust 1.88 and installed stable 1.98 strict
  `cargo clippy --workspace --all-targets --locked -- -D warnings`.
  Only the six approved baseline `collapsible_if` conditions were flattened;
  behavior and tests were retained, no lint suppression.
- PASS: Rust 1.88 `cargo test --workspace --locked -- --test-threads=4`:
  505 passed, 22 ignored, zero failed. Includes creation boundary validation,
  strict typed OpenAPI checks and global operation-ID uniqueness assertion.
- PASS: clean PostgreSQL 16, actual TCP HTTP `server --test drafts --ignored`.
  The final fresh database applied the full migration chain. Twelve concurrent
  identical commands yield one issue/binding/ledger/event and exact replay;
  concurrent changed payload conflicts. Forced ordinary-number unique conflict
  retries allocation; deleted numbers are not reused. Last-write outbox failure
  rolls back every business row. Membership removal fences a waiting replay;
  disabled/local/email-only/admin-nonmember/PAT identities and spoofed fields
  are rejected. Reporter drift/deletion invalidates replay. Creation ledger is
  append-only and rejects missing/null/unknown result fields. Lost response and
  Tracker shutdown/restart recover the original result without extra rows.
  No PM assignment, agent binding or runtime run is created.
- PASS: separate PostgreSQL HTTP `server --test sdlc --ignored` regression;
  separate actual PostgreSQL `migration --test central_subject` with its env set.
- PASS: generated OpenAPI plus `pnpm generate:api`, `pnpm openapi:check`,
  `pnpm typecheck`, and final frontend 45 files / 253 tests. The former `events`
  collision was fixed only by naming the SDLC outbox operation `sdlc_events`;
  route/wire and ordinary SSE remain unchanged. Generated TS stays Git-ignored.
- PASS: curl without bearer on the actual Draft route returns 401; positive
  browser creation/replay is exercised by the real HTTP integration suite.
- PASS: README validation, four script tests, final `git diff --check`.
  Three generated tracked `.pyc` files were restored byte-for-byte to baseline
  after verifying their Python sources were unchanged; none are task changes.

WSL bridge timeouts were transient; no WSL/Docker/shared service restart or prune
was performed. Test builds used `/tmp/tracker-pm-draft-target`. PostgreSQL used
only a Tracker-owned disposable cluster `/tmp/tracker-pm-draft-pg-1001`, port
55444, and explicitly named disposable databases. No runtime snapshots, keys,
secrets, backups, volumes or pinned images were changed. Docker was not used.
Cleanup completed: the temporary HTTP server and own cluster were stopped;
only the verified own data/socket directories and their disposable databases
were removed. The compilation target/test log is retained, not a running service.
The 18 legacy ignored infra DB tests and two Docker server smoke tests remain
unrun; the two ignored SDLC suites were run separately as described above.
No live Central Auth/Fleet/Workflow/PM or UI acceptance is claimed. No push/PR.

## Project Access Follow-Up (1 October 2026)

Baseline: `24f0f1e`, preserved on `feat/pm-clarification`. Scope is Tracker
project access only. `GET /api/v1/sdlc/project-access` uses verified Central
service-read authentication and one indexed SQL snapshot of active local
central-subject identity plus explicit ownership/membership. No admin, public,
email or local-ID bypass, per-project external requests or scope cache is used.
Only pending migration 000034 is extended with the membership index.

Final completed release gates:

- PASS: Rust 1.88 `cargo fmt --all --check`,
  `cargo check --workspace --all-targets --locked`, and strict
  `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`.
- PASS: installed stable Rust 1.98 strict Clippy with the same flags.
  No warning suppression or baseline warning exemption was added.
- PASS: Rust 1.88 `cargo test --workspace --locked -- --test-threads=4`:
  506 passed, 22 ignored, zero failed. Four threads match the Draft sidecar
  baseline; no incomplete serial attempt counts as a completed gate.
- PASS: the unchanged project-access implementation and Draft creation were
  exercised through real Tracker TCP HTTP on a fresh PostgreSQL 17.11 database
  in the preceding handoff: `server --test drafts --ignored`, one compound test,
  18.43 seconds. The full migration chain and membership index were applied.
  Authorization/scopes, owner/member deduplication, active identity, revocation,
  auth outage, Draft concurrency/rollback/restart were covered. Synthetic
  Central Auth fixtures were used; this is not deployed multi-service acceptance.
- PASS: generated OpenAPI drift, generated TypeScript client, `pnpm openapi:check`,
  `pnpm openapi:compat`, `pnpm typecheck`, `pnpm lint`, `pnpm format:check`,
  `pnpm build`, and frontend 45 files / 253 tests with four workers.
  The generated client remains Git-ignored by repository policy.
- PASS: README structural validation, four documentation/CI script tests,
  credential-pattern scan and final diff whitespace checks.

The default suite leaves 18 legacy infra database tests, two server Docker
smokes and two SDLC HTTP suites ignored. The Draft HTTP suite has the separate
PG17 evidence above; the other ignored suites were not rerun in this follow-up.
The central-subject migration test reports a pass without its dedicated database
environment and is not new PostgreSQL evidence. Coverage and live browser/PM
acceptance were not run. No Fleet/Base/Workflow files, shared runtime, images,
containers, secrets or accepted databases were modified by this follow-up.
Own disposable PG16/PG17 databases and roles from the preceding scope QA were
cleaned; no container was stopped. Base PR #126 is a dependency for live PM
delegation; dispatch/sagaAuth and real verifier/Workflow acceptance remain
external work. Publishing a Draft PR does not complete the full integration plan.

## Immutable Input Snapshot Follow-Up (2 October 2026)

Baseline: `7c98010`, preserving Draft creation `24f0f1e`. Scope is original
creation input storage/readback only, on the same Draft PR #114. No new migration
file: four all-null-or-complete snapshot columns and unique task/snapshot indexes
extend pending 000034. No accepted/shared migration was reapplied or reset.

Implemented: `GET /api/v1/issues/{id}/sdlc/pm-draft-input` returns strict
`PmDraftInputResponse`/`PmDraftInput`, exactly the agreed binding plus snapshot
ref, original title/description and separate canonical exact UTF-8 SHA-256.
It reuses context resource ACL/locks, rechecks hash/binding, and fails closed
without historical input. CreatedDraft, task.created, machine assignment/scopes
and owner-only business confirmation remain unchanged.

Completed focused and release gates:

- PASS: Rust 1.88 fmt/check and strict all-targets/all-features Clippy, locked;
  stable Rust 1.98 strict Clippy with the same flags. No lint suppression.
- PASS: exact UTF-8/CRLF/Unicode-composition hash vectors, checked independently
  against Node SHA-256. JSON escaping/order is canonical; string content is exact.
- PASS: final clean PostgreSQL 17.11 real TCP HTTP Draft/input compound suite,
  6.08 seconds, with fresh full migrations and synthetic Central Auth fixture.
  Covers twelve concurrent creates/readbacks, replay/restart, actual HTTP issue
  edit, original snapshot/hash stability, partial/immutable ledger checks,
  historical absent/all-null inputs, context ACL parity, project/account
  revocation, foreign project, valid local HS256 rejection, service-read PAT,
  operator read without owner confirmation, and Auth outage. Curl without bearer
  on the new route returns 401. No PM/run is created.
- PASS: separate PG17 HTTP clarification/owner-confirmation regression suite,
  8.54 seconds. Only own disposable databases/roles were created and removed;
  the existing validation container was neither stopped nor changed.
- PASS: regenerated OpenAPI and byte-for-byte drift; existing CreatedDraft,
  CreateDraftCommand, AssignCommand, PmAssignment and SdlcContext schemas are
  unchanged. Generated TypeScript client and OpenAPI check/compatibility pass.
- PASS: frontend 45 files / 253 tests, four workers; typecheck/lint/semantic/
  format/build. Generated client stays Git-ignored. README and four script tests,
  credential-pattern scan and diff whitespace checks pass.
- PASS: Rust 1.88 locked whole-workspace tests with four threads:
  508 passed, 22 ignored, zero failed, including strict input OpenAPI/hash tests.

Default ignored legacy PG/Docker tests and the dedicated central-subject
migration test were not separately rerun; no coverage, UI/browser or live
Central/Fleet/Workflow/PM acceptance is claimed. Both ignored SDLC HTTP suites
have the separate PG17 evidence above. No runtime/image/volume/config changes.

Remaining: Owner CAS, persisted execution ordinal, trusted project mapping,
Workflow authoritative namespace ownership/admission and metadata_v1 byte-bound
outbox are not implemented by this follow-up. Current outbox LIMIT 100 is not a
byte bound. The exact existing machine scope remains
`task-tracker:sdlc:pm:<task>:<assignment>:<execution>:<agent>:<version>`.
No fabricated queue/workspace/decomposition/run receipts or weakened Delivery
contract are introduced. The full integration plan remains incomplete.

## Byte-Bounded Metadata Follow-Up (2 October 2026)

Baseline: `addcb081`, on the same Draft PR #114. The opt-in `metadata_v1`
projection is implemented for all nine persisted event types. The legacy
default query/wire, existing content hashes, version and machine scopes remain
unchanged. No schema migration, dependency or shared runtime change is needed.
Strict DTOs are in `backend/domain/src/sdlc_metadata.rs`, projection/canonical
hash and exact serialized byte selection in `backend/app/src/sdlc_metadata.rs`,
immutable reference reads in `backend/infra/src/sdlc.rs`, and HTTP opt-in handling
in `backend/api/src/routes/sdlc.rs`. OpenAPI and the exact wire description in
`docs/CHAT_CLARIFICATION_CONTRACT.md` are synchronized.

Completed gates and contract evidence:

- PASS: Rust 1.88 locked workspace/all-targets check, strict
  all-targets/all-features Clippy, and final fmt check.
- PASS: Rust 1.88 `cargo test --workspace --locked -- --test-threads=1`:
  515 passed, zero failed, 22 ignored, including all five metadata unit tests,
  strict OpenAPI variants and opt-in-only canonical query validation.
- PASS: final PostgreSQL 17 real TCP HTTP suites on two fresh disposable
  databases with the full migration chain and synthetic Central Auth:
  `drafts --ignored` (11.44 seconds) and `sdlc --ignored` (27.13 seconds).
  All nine event types are checked against real legacy persisted resources;
  no raw result, question, answer, requirements document or aggregate is exposed.
  The valid requirements source exceeds 1 MiB. Metadata exercises Unicode,
  escaping, exact B-1/B/B+1 byte thresholds, 121 additional events with global
  sequence gaps, count/byte prefix pagination, empty/max-i64 cursors, first and
  middle oversized/corrupt blockers, static bounded 422/409 errors, nil/partial/
  conflicting references, and the hard-cap unrepresentable event.
- PASS: byte-identical replay after restart, real mutable issue edits and PM
  reassignment/new requirements/new question version. Original creation snapshot
  and historical answer fence remain unchanged. Existing ACL, membership/account
  revocation, foreign/local-token denial, and auth outage fail closed.
- PASS: actual captured synthetic HTTP bodies `tracker-metadata-created.http.json`
  (802 bytes, one event) and `tracker-metadata-all8.http.json` (17688 bytes,
  twenty events) cover all nine types together. Independent Node canonical SHA-256
  verifies all 21 event digests and UTC timestamps with nine fractional digits
  and `Z`. Fleet independently decoded these exact bytes and verified all nine
  types, Unicode/escaping/gaps and preserved resource refs/digests. Captures are
  handoff artifacts, not fixtures from a deployed environment; regeneration is
  documented under `TT_SDLC_METADATA_GOLDEN_DIR` in `docs/TESTING.md`.
- PASS: generated OpenAPI byte-for-byte drift; all pre-existing component schemas
  remain unchanged. Generated TypeScript client, OpenAPI check/compatibility,
  frontend typecheck/lint/semantic/format/build and 45 files / 253 tests pass.
- PASS: four documentation/CI script tests, README structural validation,
  credential-pattern scan and final whitespace checks.

Both targeted ignored PG HTTP suites have separate real database evidence above.
The other ignored legacy infra/Docker suites, dedicated central-subject migration
database test, coverage, UI/browser and live Central/Fleet/Workflow/PM acceptance
were not run. This does not claim full cross-service acceptance. Own temporary
PG container/databases were removed; synthetic wire captures remain for handoff.
Existing runtime groups, pinned images, secrets and protected volumes were not
modified. Pending 000034 is unchanged by this projection follow-up.

Metadata byte-bound outbox is no longer remaining work. Owner CAS, execution
ordinal, trusted project mapping, Workflow namespace ownership/admission and live
dispatch/verifier acceptance remain external. Fleet must explicitly pin projection
and contract version before its first cursor, and keep metadata digests separate
from legacy receipt hashes. No legacy receipts may be reinterpreted.
