# Critical Return transport source packet

Baseline `614d2526bccd8bee111ef2d96313b0572728aba3`, plus the unchanged storage
packet674 cherry-picked as `5c579cd`. Main heartbeat and dot-key validation are
retained. This NEW source does not inherit frozen ae07 QA results and is not a
published CLI, registered endpoint or live qualification until main integrates it.

## Exact separate contract

| Method/path under `/api/v1/issues/{id}/sdlc` | Body/result | Authority |
| --- | --- | --- |
| POST `/critical-blockers/returns` | Existing `ReturnCriticalToPm` -> existing `CriticalPolicyChange` | Configured scheduler WRITE |
| GET `/critical-blockers/returns/operations/{idempotency_key}` | Original `CriticalPolicyChange` | Configured scheduler READ |

Result is closed `{lifecycle: LifecycleReceipt, blocker: CriticalBlockerReceipt}`.
The existing app plan is moved to the domain critical module (serde/ToSchema), with
an app re-export preserving existing source users and stored JSON/hashes. No new
business fields, lifecycle enum variant, SQL journal, PM assignment or routing
generation is introduced. Shared `TrackerLifecycleCommand` and existing
`/lifecycle/operations/{key}` do NOT carry this command.

POST returns201 new /200 original replay. GET returns200 saved plan or404 absent.
Both handler results use no-store, including app errors and POST JSON rejection.
Existing strict Central authentication remains mandatory; extractor/auth failures
outside these handlers keep the existing parent middleware behavior. Typed JSON
rejects extra owner/holder claims and unknown/nonterminal native states.

Existing `LifecycleRepository` adds ONLY these default-Unavailable ports:

```rust
async fn return_critical_to_pm(&self, task: Uuid, actor: &Principal,
    command: ReturnCriticalToPm) -> Result<(CriticalPolicyChange, bool), AppError>;
async fn critical_pm_return_operation(&self, task: Uuid, actor: &Principal,
    key: &str) -> Result<CriticalPolicyChange, AppError>;
```

App preflight denies human/trusted-human, PM/technical grant, mixed/broad scopes,
wrong READ/WRITE shape and invalid/dot keys without rewriting the caller's body.
It is NOT configured scheduler authorization: the actual repository still checks
the configured subject (not orchestrator/verifier), fresh project/active owner ACL,
and the full current CAS/assignment/execution/fence/run policy before writing.
Unknown outcome recovery reads the existing immutable674 Return archive under
`(task, scheduler subject, original key)`, verifies stored command/hash and returns
saved after-receipt/blocker, never a reconstruction from current state. Read needs
no fresh live lease; it is immutable history, not permission for fresh work.

Fleet MUST read actual matching native terminal before POST; stop ACK, unknown,
expiry or receipt-shaped text is insufficient. No new terminal proof service is
added. Main integrates this separate command into its EXISTING durable command
journal/envelope. This packet creates no second transport queue or Fleet adapter.

## CLI source

```text
task-tracker sdlc critical-blocker return-to-pm --task UUID --from-file return.json
task-tracker sdlc critical-blocker return-operation --task UUID --key ORIGINAL_KEY
```

`--from-file -` retains existing stdin behavior. POST uses the exact supplied
`review.idempotency_key`/body; no inferred keys, actor flags, header key, automatic
POST retry or auto-dispatch. On unknown outcome preserve body and use explicit
original-key GET. Legacy `critical-blocker operation` still reads the producer's
original Held v1 operation, not a Return. Existing lifecycle GET may separately
read current canonical state; it is not the Return journal receipt.

CLI loopback tests deliberately use partial TRANSPORT response stubs, not actual
native terminal or persisted readback. Typed plan/body tests and existing PG
fixture assertions are separate. CLI does not gain a production domain dependency.

## Main-owned registration

`critical-pm-return-wiring-registration.patch` is NOT applied here. It adds ONLY
two Postgres UFCS trait delegates, two routes on the existing strict-Central SDLC
router family and ApiDoc paths/types. No DI/module/dependency/client/migration
registration is changed. Main generates OpenAPI/client after applying the patch.
No SQL change: the same674 fragment follows lifecycle SQL within ONE unreleased096.
Required populated0033 -> latest qualification remains separate; no production
DB was queried or changed by this package.

## Targeted qualification inputs

No Cargo/Docker/PG execution. Rustfmt, diff and registration-patch applicability
are source checks only. Main must first collect nonzero selectors from `backend`:

```text
cargo test --locked -p app --lib sdlc_critical_blockers::tests::return_transport_ -- --list
cargo test --locked -p api --lib routes::sdlc_delivery::tests::critical_return_ -- --list
cargo test --locked -p task-tracker-cli --test workflows sdlc_critical_return_ -- --list
cargo test --locked -p infra --lib sdlc::candidates::tests::critical_blocker_tests:: -- --list --ignored
```

Expected new tests: app2, API2, CLI5. Existing ignored PG aggregate1 is extended
with absent-before-commit Return GET, exact saved readback, producer/owner/WRITE
read denial, dot/unknown key denial and zero mutation. Its environment remains
`TT_CANDIDATES_TEST_DATABASE_URL`, fresh disposable DB ending `_candidates_test`.
Do not count source-format success or ae07's frozen snapshot as these tests PASS.

Remaining: main registration/actual qualification, Fleet actual stop/readback plus
durable envelope integration, new same-task PM review purpose/reservation, scoped
evidence read, new revision/exact owner consent/routing generation, actual PM
Workflow/native completion and fresh Analysis. Return success alone proves none
of those subsequent effects.

## Keyfix companion

This successor requires main's NEW-critical-only keyfix providing
`domain::sdlc_critical_blockers::valid_operation_key(&str) -> bool` and the matching
pure guard/SQL changes. Return app preflight and archive GET reuse that actual
domain helper. CLI production uses main's identical local `readable_key`: domain
remains dev-dependency only. A new CLI unit test compares it with the domain helper
(opaque punctuation, slash/backslash/dot rejection, UTF-8 byte bounds).

CLI validates the supplied key BEFORE all critical POST/original-key GET requests,
including Return. Valid fixtures use `critical:original:1?x=+%#` and
`return:original:1?x=+%#`; no encoded path separator is required. Return no-HTTP
negatives cover invalid keys in BOTH body and read selector. Base path security,
legacy/global key validators, dependencies, SQL and all endpoint/receipt shapes
are unchanged in this companion. Main's keyfix is a required source prerequisite,
not a locally fabricated helper or an inherited QA PASS.

Additional collection input (one authored CLI parity unit):

```text
cargo test --locked -p task-tracker-cli --bin task-tracker critical_readable_key_matches_domain -- --list
```

No Cargo/HTTP/PG/Docker was executed for this companion. Main owns the next
Tracker-only immutable QA; unchanged Fleet does not need another build here.
