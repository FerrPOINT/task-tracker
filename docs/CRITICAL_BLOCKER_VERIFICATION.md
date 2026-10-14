# Critical Blocker Qualification

## Latest Immutable Source

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
The source also contains the scheduler terminal-return transaction and archive,
not fabricated result/reconcile evidence. Public Return wiring and the complete
same-task/new-PM execution, new revision/exact owner consent and routing generations
remain separate work.

The one new unreleased096 includes the critical SQL after the lifecycle fragment.
Readonly accepted Tracker lineage is19 applied rows through0033, not096; no
released034-095 migration was edited by this packet. Actual populated0033-to-latest
qualification and controlled installation remain required.

Earlier source checks retain their own scope in
[clarification verification](CHAT_CLARIFICATION_VERIFICATION.md). No native model
inference, installed seven-agent flow, deployment or full-goal completion is claimed.
