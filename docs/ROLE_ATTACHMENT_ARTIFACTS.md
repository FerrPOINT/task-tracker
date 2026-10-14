# Role Attachment Artifacts

Source packet based on integration a17e8f70382f2a01b5bc1bf35be4bfd23833b0dd.
Existing attachment routes, metadata table, repository, disk storage and CLI are
reused. No lifecycle/candidate wire, migration, dependency, scope, planner,
controller, artifact table or separate store is introduced. Main owns the
registrations/DI below and generated OpenAPI; this packet alone is not live.

## Wire And Commands

- POST `/api/v1/issues/{task_id}/attachments`: multipart `file`, existing 201 response.
- GET `/api/v1/issues/{task_id}/attachments`: existing `{attachments:[...]}` response.
- GET `/api/v1/attachments/{attachment_id}/download`: verified raw bytes.

The existing AttachmentResponse gains only an optional field, omitted on legacy:

```json
{"artifact":{"receipt":{"reference":"tracker-attachment:<UUID>","sha256":"<raw-byte SHA256>"},"binding":{"assignment_id":"<UUID>","execution_id":"<UUID>","role":"analyst"}}}
```

`issue_id` is the real task=root. ArtifactReceipt and DeliveryResult are unchanged.
No execution token, secret credential, owner impersonation or caller-selected
lease/role is added. The opaque reference resolves only against the configured
Tracker owner, never against a caller-supplied URL. Consumers download bytes and
compare their raw SHA with the receipt. This is not a Workflow terminal receipt.
Binding roles are `analyst`, `architect`, `developer` (including Rework), `reviewer`,
`tester`, `devops`; Fleet's `dev_ops` mapping remains outside this Tracker wire.

```text
task-tracker attachment upload --issue UUID --file PATH --content-type application/json
task-tracker attachment list --issue UUID
task-tracker attachment download UUID --out PATH
```

Upload/list bypass generic issue GET only for canonical non-nil UUID input.
Human issue keys retain their existing lookup. Upload prints the actual server
AttachmentResponse, including artifact receipt; download's existing output is
unchanged. Fleet uses the actual upload receipt in the role-specific DeliveryResult
and obtains Workflow terminal evidence separately. Forge job artifacts remain
DevOps-only and require actual Forge job/attempt identity.

## Authorization And Integrity

- Central-authenticated delivery Principal retains the existing exact grant
  `task-tracker:sdlc:delivery:<task>:<assignment>:<execution>:<agent>:<fence>`.
  POST requires existing `task-tracker:write`; GET requires existing read scope.
- Only exact upload/list/download resources are machine-routed. PM grants, nil or
  noncanonical IDs, other methods/resources and generic service accounts are denied.
  Existing generic bearer middleware still rejects delivery grants everywhere else.
- Configured SchedulerRead is a separate existing authority: GET download only,
  fresh local project ACL, and only artifacts of the current assignment explicitly
  referenced by its retained current proposal. No scheduler upload/list permission.
- The actual local machine user is author. Active local ownership/membership is
  checked under locks without Central/global-admin bypass. Task flow/root/instance,
  exact confirmed routing, current immutable assignment and run-binding ledgers,
  grant, running status and actual lease are checked. Lease holder is the configured
  scheduler; agent subject must match run binding, never lease holder.
- Writes retain the issue/task/head/ACL locks through byte publication and metadata
  insert, checking the server clock again before insertion. Exact upload replay is
  stable while the current running fence/lease remains valid. Stale/expired writes,
  including upload retries after assignment advancement, fail closed.
- Producer reads its own current-assignment sealed artifacts plus exact refs from
  the accepted predecessor chain. Unaccepted other-run uploads, ordinary attachments
  and unrelated same-project artifacts are not readable with a delivery grant.
  Traversal rejects cycles and is bounded to 128 records. Legacy Analysis is not
  retroactively converted to a sealed attachment artifact.
- Existing human reads retain legacy authentication but sealed reads/list metadata
  require fresh active local owner/member ACL, including under Central mode.
- Reserved filename-safe storage keys retain assignment/execution/run/role/raw SHA.
  Stable UUIDv5 identity also binds task, normalized filename and MIME. Server derives
  all authority from actual ledgers. Legacy random-key attachments remain unsealed.
- Disk publication stages a complete file, syncs it and atomically hard-links it
  without replacement. Concurrent exact replay verifies the winner's bytes. A
  checksum mismatch is an error, never permission to overwrite or delete evidence.
  Sealed downloads are bounded by the existing upload limit and verify SHA/size.
- Sealed delete is denied at service, repository and FileStorage boundaries for all
  callers, including admin. Generic metadata save cannot create a sealed attachment.

The filesystem and PostgreSQL are not one atomic resource. Metadata commits only
after successful immutable byte readback. If DB commit fails, a sealed blob may
remain without a metadata row; exact authorized retry adopts those same bytes.
No error cleanup deletes the sealed destination or a concurrent winner. A process
crash may leave its private staging file. No new cleanup registry or broad purge is
introduced; existing owner retention procedures must handle confirmed orphan data.

## Main Registration Handoff

1. `backend/domain/src/lib.rs`: `pub mod attachment_artifact;`
2. `backend/app/src/lib.rs`: `pub mod attachment_artifacts;`
3. `backend/infra/src/sdlc.rs`:

   ```rust
   #[path = "attachment_artifacts.rs"]
   pub(crate) mod attachment_artifacts;
   ```

4. `backend/api/src/middleware/mod.rs`: `pub mod attachment_auth;`
5. In `backend/api/src/lib.rs`, move only the existing upload/list/download routes
   to an attachment router using `attachment_auth`, outside the generic bearer
   layer. Preserve the existing upload `DefaultBodyLimit`. DELETE stays protected
   by generic bearer auth. Do not stack generic bearer/strict SDLC auth on the three
   attachment routes: both intentionally reject delivery attachment capabilities.
6. In existing AppContext/server DI, construct AttachmentServiceImpl with
   `.with_artifact_config(sdlc_config.clone())` using the SAME validated config and
   existing FileStorage Arc used by SDLC/deployment. Do not re-read or invent a
   second config selector/pool. Its existing constructor defaults to disabled
   artifact policy, leaving legacy callers unchanged until main wires it.
7. Add AttachmentArtifact/ArtifactBinding to existing API schema registration if
   not included by schema traversal, and regenerate the existing OpenAPI export.
8. For PG test reuse, append ONLY this child registration to
   `backend/infra/src/sdlc_candidates_pg_tests.rs`:

   ```rust
   #[path = "attachment_artifacts_pg_tests.rs"]
   mod artifact_tests;
   ```

The PG test uses the existing Fixture, actual owner commands, locks, history and
attachment rows. Fleet/native/Workflow observations are explicitly synthetic.
It does not create a database, pool, runner or Docker project. It transfers the
existing fixture connection for the final real AttachmentRepository delete check.

## Focused QA Requests

These are requests for main QA, not claimed executed tests. From `backend`, first
collect each selector using `-- --list` and assert nonzero, then execute:

```text
cargo test -p domain attachment_artifact::tests::
cargo test -p app attachment_artifacts::tests::
cargo test -p app services::attachment::tests::
cargo test -p api routes::attachments::tests::
cargo test -p task-tracker-cli attachment_tests::
cargo test -p infra storage::tests::sealed_bytes_concurrent_replay_tamper_and_delete
cargo test -p infra sdlc::candidates::tests::artifact_tests::actual_role_artifacts_are_fenced_replayed_predecessor_scoped_and_immutable -- --ignored --exact
```

PG env: `TT_CANDIDATES_TEST_DATABASE_URL`, fresh disposable DB name ending
`_candidates_test`, as required by the existing fixture. Do not run its two fresh-DB
tests together against one database. This artifact PG selector includes an actual
31-second server-clock expiry check; no trigger or protected-ledger weakening.

After registration, main should also run its existing Central-auth route harness
for these three routes (delivery grants, PM denial, configured SchedulerRead,
foreign-project and global-admin hints), source OpenAPI and all-target Clippy.

## Changed Source Paths

```text
backend/domain/src/attachment_artifact.rs
backend/domain/src/repositories.rs
backend/app/src/attachment_artifacts.rs
backend/app/src/context/dtos.rs
backend/app/src/context/traits.rs
backend/app/src/services/attachment.rs
backend/infra/src/attachment_artifacts.rs
backend/infra/src/attachment_artifacts_pg_tests.rs
backend/infra/src/repos.rs
backend/infra/src/storage.rs
backend/api/src/dto/mod.rs
backend/api/src/middleware/attachment_auth.rs
backend/api/src/routes/attachments.rs
backend/cli/src/commands.rs
docs/ROLE_ATTACHMENT_ARTIFACTS.md
```
