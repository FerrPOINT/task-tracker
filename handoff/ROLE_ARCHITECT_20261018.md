# Tracker Initial Unsubmitted PM Lease Recovery

## Source And Ownership

- Repository: `FerrPOINT/task-tracker`.
- Frozen base: `beb029fd381f025fe9879d98e34bc87bfe686fc9`.
- `git fetch origin +refs/heads/main:refs/remotes/origin/main` succeeded; `git rev-parse origin/main` returned that exact SHA before creating this worktree.
- Branch: `fix/pm-unsubmitted-lease-20261018`.
- Worktree: `C:/git/azhukov/sdlc/.local/single-task-sdlc-20261013/tracker-pm-unsubmitted-lease-20261018`.
- Commit: the implementation commit containing this handoff (`git log -1 --format=%H -- handoff/ROLE_ARCHITECT_20261018.md`). Main publishes; no push/merge performed here.
- Root/nearest instructions and actual main source were read first. Dirty root checkout files, Fleet, Hermes, live runtime/config/DB/tasks were not changed. No agents/models were invoked.

## Implemented

The existing SdlcService exposes the requested POST `pm-draft-execution-lease/recover-unsubmitted` and GET `recover-unsubmitted/operations/{idempotency_key}`. It reuses scheduler write/read authorization and the original `ExecutionLeaseReceipt` / `ExecutionLeaseOperation` response types. The request is a closed DTO with exactly `expected_owner_version,fence,lease_id,expected_lease_version,session_run_id,barrier_id,idempotency_key`.

Before network IO, a repository preflight validates current authority/expiry/CAS or returns exact historical replay. A fresh POST then independently GETs configured Fleet `/internal/runtime/v1/pm/runs/{session_run_id}/unsubmitted?barrier_id=<uuid>`. This base has no separate PM resume HTTP reader: its resume trusts the configured scheduler. The new method therefore reuses the actual existing FleetConfigurationReader, `TASKTRACKER_SDLC__FLEET_URL` and `TASKTRACKER_SDLC__FLEET_READ_TOKEN`, without introducing another client/config/token service. Existing URL validation, sensitive bearer header, no proxy/redirect/retry and timeout apply; the new read also requires JSON, identity encoding and a 64-KiB bound.

The strict witness has the requested seven fields. Its `identity` reuses Tracker's existing native-admission Identity under the `PmExecutionIdentity` alias, matching Fleet's existing ten-field identity including the original `assignment_operation_key`. Every task/root/project/instance/agent/assignment/execution/version coordinate and run/barrier/lease/owner version is compared against the retained reservation and exact requested current lease. No caller boolean, witness body, terminal or checkpoint is accepted on the API.

After HTTP, the repository locks/rechecks task, latest assignment, reserved selector, owner/version, scheduler and PM-holder project access, initial single-task Draft, expired current lease and absence of native admission/completion/resume/delivery ownership. Same lease ID/holder/claimed_at and fence remain; only version (+1), DB-clock heartbeat and expiry (TTL 30 seconds) advance. Active leases are rejected and keep the ordinary heartbeat path. No task status, input, owner confirmation, outbox or original claim/history is rewritten.

Migration `m20261018_0000098_pm_unsubmitted_recovery` is registered after 0097. It adds one nullable `recovery` JSON column to existing immutable `sdlc_pm_lease_operations`; no new table. It retains exact command, independently read witness and current transaction ID, alongside existing canonical hash/result/version. Hash operation name is exactly `recover_unsubmitted_pm_execution_lease`. New evidence inserts have an identity/CAS/expiry/ACL gate; the expired-update gate accepts only matching same-transaction evidence, while preserving the original trusted-resume exception. A deferred gate rejects evidence-only commits. Claim/heartbeat NULL-evidence rows keep their original holder constraint and history. SQL verifies consistency of stored evidence; authenticated HTTP provenance is the application boundary, as opposed to a caller claim.

POST replay and GET return historical acceptance without another Fleet witness or lease renewal. Different body under the original key is rejected. Unknown ACK must use GET with that original key. Fleet's durable barrier must remain active until authoritative receipt reconciliation, including unknown/timeout; this package does not implement or register that Fleet side.

## Files

- `backend/domain/src/sdlc_execution_lease.rs`: strict request/witness and existing identity alias.
- `backend/domain/src/sdlc.rs`: repository contract for preflight, commit and operation readback.
- `backend/app/src/lib.rs`, `backend/app/src/sdlc_pm_unsubmitted.rs`: existing-service orchestration and exact command/context/witness validation.
- `backend/app/src/sdlc_configuration.rs`: independent witness GET through the existing configured reader.
- `backend/infra/src/sdlc.rs`, `backend/infra/src/sdlc_pm_unsubmitted.rs`: existing repository registration, reauthorization, locking, evidence/history and CAS.
- `backend/api/src/lib.rs`, `backend/api/src/routes/sdlc.rs`: route/OpenAPI registration, handlers and schema test.
- `backend/migration/src/lib.rs`, `backend/migration/src/m20261018_0000098_pm_unsubmitted_recovery.rs`, `backend/migration/src/pm_unsubmitted_recovery.sql`: forward migration and gates.
- `backend/app/src/sdlc_pm_unsubmitted_tests.rs`, `backend/app/src/sdlc_configuration_tests.rs`: unit/strict-witness and loopback HTTP tests.
- `backend/infra/src/sdlc_candidates_pg_tests.rs`, `backend/infra/src/sdlc_pm_unsubmitted_pg_tests.rs`: registered ignored disposable-PG regression using the existing fixture.
- `openapi/openapi.json`: scoped contract snapshot update from matching existing resume/heartbeat shapes; source-to-snapshot equality assertions added. No full generator/build was run.
- `docs/API.md`, `docs/ARCHITECTURE.md`, `docs/DATA_MODEL.md`, `docs/TZ.md`: contract/storage/trust-boundary documentation.

## Verification

Executed (no Cargo build):

- `rustfmt --edition 2024 --config skip_children=true --check <15 touched Rust files>`: PASS. The explicit file list avoids formatting unrelated code or recursively touching sibling modules.
- Python standard-library JSON parse of `openapi/openapi.json`, seven required request fields, closed properties, authenticated POST and GET operation receipt reference: PASS.
- `git diff --check` / staged diff whitespace check: PASS after trimming added documentation EOF blanks.

Authored but NOT compiled/executed here, per no-heavy-build/QA scope:

```text
cargo test --locked --manifest-path backend/Cargo.toml -p app sdlc_pm_unsubmitted::tests
cargo test --locked --manifest-path backend/Cargo.toml -p app unsubmitted_pm_readback
cargo test --locked --manifest-path backend/Cargo.toml -p api unsubmitted_pm_recovery_schema
cargo test --locked --manifest-path backend/Cargo.toml -p infra unsubmitted_pm_recovery_cas_witness_replay_and_history -- --ignored --nocapture
```

The last test requires a FRESH disposable `TT_CANDIDATES_TEST_DATABASE_URL` whose database name ends in `_candidates_test`; it refuses a previously migrated database and deliberately waits one real TTL with `pg_sleep(31)` rather than disabling triggers or forging expiry. It tests active-lease denial, scheduler-only authority, revoked PM membership, stale fence, bad witness, raw expired-update denial, same-key replay concurrency, different-key CAS contention, key/body conflict, original claim retention and lost-ACK GET after repository reload. Native witness bytes in these repository fixtures are synthetic, never live Fleet evidence. HTTP tests separately check exact authenticated witness URL and reject redirects/403/503/boolean/oversize/encoding errors. Unit tests cover all witness identity coordinates, strict DTOs, owner/fence changes, legacy/successor lifecycle and Completed status.

No PostgreSQL/migration execution, compile/typecheck, source-to-generated OpenAPI parity run, live API smoke or installation is claimed. Integrator/Fermat must run the affected gates and qualify the companion Fleet barrier before any recovery of the real task. No Cargo/lock/Base pin or original migration was edited.
