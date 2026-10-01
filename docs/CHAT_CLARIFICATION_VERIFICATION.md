# Tracker Clarification Verification

Date: 1 October 2026. Scope: Tracker backend/tests/docs only. No sibling repo
or production UI edits. No push, merge, deployment or real PM acceptance.

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
