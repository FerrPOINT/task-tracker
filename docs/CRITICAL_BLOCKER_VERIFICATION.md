# Critical Blocker Qualification

## Source Follow-Up: 15 October

Tracker544322c passes fmt and the previous borrowed-DB compilation point, but
workspace all-target Clippy stops with E0599 in the new reader PG fixture. It
called `claim_pm_execution_lease` instead of the actual `claim_execution_lease`.
188139a corrects that method; the same issue in the newly authored lifecycle
fixture is corrected during938b7da integration. No rerun or new PG pass is claimed.
The pure16/reader/API cases and original-Hold aggregate were not reached.
The Base-v2 QA project and owned inputs were cleaned; accepted data is unchanged.

938b7da registers the current-review lifecycle SQL/infra companion in the ONE
unreleased096 packet: claim/commands/resume/finalization, append-only routing
generations, inventory selectors and resolved-head CAS. Confirmation stays Draft
until atomic PM completion/resolution/new Analysis. Four new source-contract
units and one lifecycle PG aggregate are authored, not executed. Populated-history
migration and two complete review cycles remain required; this is not live readiness.
[Persistence contract and exact gates](contracts/critical-pm-lifecycle-persistence.md).

## PM Review Reservation Qualification

Tracker `8ac333aeb8c0a2d5adbf08aebece197d87ffa2cb` passes fmt, locked workspace
all-target check and27 focused tests: domain4/app9 planner, app4/API4 transport,
global route1, CLI HTTP3/parity1 and actual reservation PG1. The reservation
aggregate verifies transaction/history, scopes, duplicate races and rollback.
Generated Rust OpenAPI SHA256 is
`28ec367cd3655bb0e4ae5ce3c1dd866dac08e89d60b8580254320636dcd7fa9e`;
actual openapi-typescript7.13.0 client generation succeeds, SHA256
`9f33311b538e4468189bae71ddd7a27939749aee780b1f280cb3fc6b48dcdb8f`.

The combined batch is NOT wholly passing: its second, prior Hold/Return aggregate
refuses the already-filled disposable database before entering its scenario.
The runner now assigns two different databases; two input regressions PASS.
The second aggregate on the changed096 and new-source strict Clippy still require
the targeted follow-up. Tracker Clippy was not reached. Base v2 cleanup and owned
input removal are verified; accepted services/data/secrets remain unchanged.

This qualifies reservation, not PM lease/admission/continuation, revised consent,
routing generations or live inference. New current-review read/lifecycle packages
are separate source-only work and cannot inherit this result.

## Latest Qualified Return Transport

Tracker `caf7ab3f9c93946fc149d61027bee72cafb536ac`, SDK
`33ce7042a7617db59285390a67b191e3ae738dbf`: locked workspace all-target check,
fmt and strict workspace Clippy PASS. Thirteen selected tests PASS: two app,
two API, one API registration, five CLI HTTP scenarios, one CLI/domain key parity,
one numeric future probe and the actual critical hold/terminal-return PG aggregate.
The aggregate includes authorization, immutable evidence, races, SQL rollback,
Return and original-key replay; all nine scenario phases finish, in7.26s.

The test-only async phase boundaries reduce measured outer scenario size from
107216 to22528 bytes and Return helper size from68544 to10736. This is a successful
regression with unchanged assertions/fixtures, not proof of a universal overflow
cause. No stack environment, timeout or production authorization was relaxed.

Rust-generated OpenAPI SHA256:
`898c0c19efdf77a388e7ec3442e1ec4eeb4640a1481b5e87b7da1afec4a8712f`.
Runner exit0; Base v2 journal `cleaned`, owned inputs and disposable volumes cleaned.
No accepted stand, provider, model, runtime or secret was changed. Native proof in
the PG fixture is synthetic. New critical PM reservation is not covered by this
earlier run; its later focused qualification is recorded above. Actual PM
re-entry/consent/routing remain open.

## Historical Key-Fix Qualification

Tracker `3ddb1596960ffd10ec66a99b4353ef3a74a70187`, backend SDK
`33ce7042a7617db59285390a67b191e3ae738dbf`, was checked locally using the existing
Base ComposeHelper v2 runner. This was a Tracker-only batch: previously qualified
Fleet compilation/tests were not repeated. No accepted stand was replaced.

| Check | Actual result |
| --- | --- |
| Rust fmt and locked workspace all-target check | PASS |
| Critical domain / app policy | 5 /15 PASS |
| Critical producer CLI HTTP tests | 5 PASS |
| Delivery capability policy / API registration | 1 /1 PASS |
| Critical storage/terminal-return PostgreSQL aggregate | FAIL: Rust stack overflow, SIGABRT |
| Subsequent Clippy and final OpenAPI export | NOT REACHED |

Twenty-seven selected tests pass before the PG failure. The aggregate failure
does not prove atomic hold/return persistence, migrations or native behavior.
Its cause still needs numerical future-frame/recursion diagnosis; no stack limit,
timeout, assertion, ignored selector or source-history protection was weakened.
The test database is disposable; this is not a populated accepted-data upgrade.

Runner exit101 at `tracker_critical_pg`; the v2 journal is `cleaned` and owned input
cleanup is verified. Private diagnostic logs remain task-local. Permanent services,
runtime files, volumes, backups and secrets were not changed.

## Resolved CLI Failure

The preceding ae07 batch failed three producer CLI read/error/recovery tests. A
critical operation key containing a literal slash could be submitted, but the
existing Base transport intentionally rejects encoded path separators. This is
not a new route allowlist or a missing user permission.

The new, unreleased critical contract now rejects empty, over128-byte, whitespace,
control, slash/backslash and standalone dot-segment keys before side effects.
CLI/server/SQL agree; other encoded opaque characters are retained exactly.
Positive readback and negative no-HTTP cases pass. Legacy key contracts, Base path
security and dependency pins are unchanged; no second transport was added.

## Implementation And Remaining

Source registers producer POST/detail/original-key API and CLI. Existing
attachment FileStorage is supplied by production DI; missing storage fails closed.
The source also contains the qualified scheduler terminal-return transaction,
archive and public API/CLI wiring, not fabricated result/reconcile evidence.
The same-task/new-PM reservation persistence passes the earlier27 focused cases
above. New current-review lifecycle/generation source is integrated separately
and does not inherit that pass. Fleet new-PM control/admission/runtime, actual
owner reconfirmation and new-source PG acceptance remain required.

The one new unreleased096 includes the critical SQL after the lifecycle fragment.
Readonly accepted Tracker lineage is19 applied rows through0033, not096; no
released034-095 migration was edited by this packet. Actual populated0033-to-latest
qualification and controlled installation remain required.

Earlier source checks retain their own scope in
[clarification verification](CHAT_CLARIFICATION_VERIFICATION.md). No native model
inference, installed seven-agent flow, deployment or full-goal completion is claimed.
