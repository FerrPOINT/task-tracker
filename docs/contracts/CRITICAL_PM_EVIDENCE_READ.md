# Current PM sealed evidence download (source only)

Baseline: accepted reader source `98cd0f5` over Tracker `8ac333a`. The blocker
reader companion patch is still required and is NOT silently applied here.

## Frozen route and files

ONLY existing `GET /api/v1/attachments/{id}/download` with the existing verified
Central PM grant and READ scope. No new endpoint/controller, credential, public
DTO, trait, API client, schema or codegen. No ordinary list/upload/delete access.

New production helpers:

- `app/src/attachment_pm_review.rs`: PM single-download route shape and exact
  saved proposal evidence/seal validation; separate pure tests.
- `infra/src/attachment_pm_review.rs`: child of critical blocker reader; download
  using existing DB/config/FileStorage and the SAME `current_pm_reader` authority
  from `98cd0f5`, before AND after blob I/O.

Companion-only hooks, not applied to shared production files:

`docs/contracts/critical-pm-evidence-read.patch`

- Register new app helper and new infra child.
- Existing attachment service authorizes only the PM single-download shape.
- Existing attachment middleware retains Central validation/service permission,
  registered-machine denial and PM exact grant parser. PM never becomes legacy
  UserClaims; recognized PM responses/errors are no-store.
- Existing infra download dispatches PM grants exclusively to the narrow helper.
  Malformed/stale PM grants cannot fall back to technical/scheduler/human readers.
- Existing private provenance reader returns its ALREADY decoded
  `(DeliveryAssignment, RunBinding)` rather than discarding them. Existing callers
  still discard the tuple; no authority/SQL changes or duplicate provenance queries.

## Helper signatures

```rust
// app::attachment_pm_review
pub fn route(actor: &Principal, method: &str, path: &str) -> Result<bool, AppError>;
pub fn evidence(
    blocker: &CriticalBlockerReceipt, attachment: &Attachment, seal: &Seal,
) -> Result<(), AppError>;

// infra, private sdlc::critical_blockers::pm_evidence module
pub(crate) async fn download(
    db: &DatabaseConnection, config: &SdlcConfig, actor: &Principal,
    id: AttachmentId, storage: &dyn FileStorage,
) -> Result<(Attachment, Vec<u8>), AppError>;
```

## Actual read sequence

1. Borrow the configured DB pool/config, without `connect()`, registration writes
   or new credentials. Derive the ONE task from the exact PM grant, never from the
   requested attachment or a display name.
2. In the existing task/ACL lock order, `load_read` and `owner_access` check current
   PM assignment, active actor/owner and project ACL; retain existing project
   artifact rollout. Read ONLY current head's saved PM review journal.
3. The requested sealed attachment must be an exact `proposal.evidence` item:
   reference UUID, raw SHA and typed assignment/execution/role binding. Check
   task/root, seal storage key, original technical fence/role/run. Ordinary,
   unlisted sealed, foreign task and malformed references fail before FileStorage.
4. Reuse existing seal validation and persisted technical assignment/run/author
   provenance; match saved blocker's producer, owner, agent, assignment hash,
   chat and run. This is archived technical provenance, not new technical admission
   or a fabricated native/Workflow witness.
5. Invoke SAME current-PM journal/selector/lease/fence reader last, using the DB
   clock after metadata/provenance reads. Close transaction BEFORE FileStorage I/O.
6. Read existing blob, verify actual raw size/SHA with existing `verify_bytes`.
   Repeat steps 2-5 in a NEW transaction: revocation, changed assignment/review,
   lease expiry/fence drift or metadata/provenance drift denies returning bytes.
7. Require the same immutable blocker/seal/attachment snapshot and verify raw bytes
   against the final metadata again. Only then return the existing attachment DTO
   and bytes through the existing route. No byte or credential logging.

No task/ACL/lease locks span FileStorage I/O. Same-assignment heartbeats are allowed:
mutable lease version/timestamps are NOT compared as immutable snapshot identity;
each phase separately validates the actual current lease and fresh DB clock.
No new state, journal, receipt model, queue, dispatch, run or owner confirmation.

## Known producer dependency, not a test bypass

At `8ac333a`, current review `claim_pm_lease` updates top-level status without
lifecycle/head projection, so actual PG claim rolls back before the reader. This
packet remains BLOCKED BY PRODUCER CLAIM PROJECTION until Locke/main supplies the
existing lifecycle hook. SQL/routing/lifecycle guards are not changed here.

The existing ignored reader fixture still performs that actual claim. Sealed
reader cases are added AFTER it succeeds, not by skipping/replacing it or injecting
lease state. An extra unlisted sealed artifact is legitimately published through
the existing technical publisher BEFORE Hold, while its original lease is active.

## Authored qualification

Three new test functions (app 2, API no-store 1), plus a dedicated PG scenario
called from the existing ignored current-PM reader test:

- Canonical GET-only single-download route; deny list/upload/delete/HEAD, malformed
  UUID, human/ambiguous PM grant and missing READ; preserve non-PM route fallback.
- All six technical roles: exact evidence ref/SHA/binding/task/run and raw bytes;
  ordinary/foreign/wrong role/hash/ref evidence denied.
- Recognized PM success and errors no-store.
- PG scenario: exact original bytes, old grant/scheduler/missing/ordinary/unlisted
  sealed denial before storage; inactive owner and removed PM project membership
  DURING FileStorage I/O cause post-I/O denial; corrupt returned bytes denied.
  A five-second bounded mutation verifies there are no reader locks across I/O.
  State/lease/journal/outbox unchanged. All data/custody synthetic, not live proof.

Source checks only: rustfmt of new/changed owned Rust files, individual and combined
companion patch applicability, staged whitespace. Cargo/Docker/compile/test/PG/live
execution is forbidden here and NOT claimed. No changes to frozen QA source.

After parent review, apply `critical-pm-blocker-read.patch` from `98cd0f5`, then
`critical-pm-evidence-read.patch`, and integrate the producer claim projection fix.
Targeted parent commands from `backend`:

```text
cargo test -p app attachment_pm_review::tests
cargo test -p api middleware::attachment_auth::pm_review_tests
cargo test -p infra actual_current_pm_blocker_read_requires_saved_review_fresh_acl_and_current_lease -- --ignored
```

The last command uses the existing fresh disposable candidates DB/protected runner
and cleanup; no new harness or permanent resources. Prior pure expiry/fence tests
are reused, not relabelled as executed PG evidence. Apply/compile/affected QA remain
main-owned before claiming the PM download capability available.
