# Tracker clarification API v1

Implementation contract for the approved PM clarification slice. Runtime acceptance
with Fleet/Workflow and a real PM has not been performed. Rust DTOs and generated
`openapi/openapi.json` are the schema source of truth.

## Human Draft creation: Fleet saga source

Fleet directory authorization uses `GET /api/v1/sdlc/project-access` with the
current caller's bearer. Response:
`{contract_version:1,tracker_instance_id,project_ids:[UUID]}`. Project IDs are
unique and sorted, derived in one indexed SQL snapshot from the active local
central-subject identity and explicit ownership/membership. No global admin,
public-project or legacy Central Auth bypass applies. Service read access is
required; no cache may retain revoked membership. Missing/inactive local identity
is 403; verified identities with no projects return an empty array.

`POST /api/v1/projects/{project_id}/sdlc/drafts` accepts only:

```typescript
type CreateDraftCommand = {
  title: string;
  description: string;
  idempotency_key: string;
};
type CreatedDraft = {
  tracker_instance_id: string;
  project_id: string; // UUID
  task_id: string; // UUID
  root_task_id: string; // equals task_id for this slice
  task_key: string; // display key, never cross-service identity
  owner_subject: string; // exact verified Central Auth subject
  stage: 'Draft';
};
```

All fields are required; unknown request/typed-response fields are rejected.
Title must be nonblank, at most 500 Unicode characters, without control
characters. Description may be empty and supports newlines; limit 100000
characters, no NUL. Idempotency key is 1..128 UTF-8 bytes, no whitespace/control.
String content is preserved exactly; JSON object member order is insignificant.
Clients must not submit user/owner/reporter/agent/status/project/root claims.

Forward the original browser Central Auth session bearer, never a Fleet PAT or
local JWT. The existing shared `allows_service("task-tracker", POST)` policy
must pass (browser sessions follow shared service policy; no invented browser
scope claim). Active local identity must already be linked by exact `central_sub`
and have explicit project owner/member write access. Email, local UUID, global
admin and the legacy central/public-project bypass confer no access. Operator
creation owns its own Draft, never another human's. Cookies alone are not bearer
credentials. Missing/invalid auth is 401; nonhuman or denied access is 403;
missing project/original issue is 404; unavailable Auth/SDLC is 503; invalid
command is 422. Instance requires stable `TASKTRACKER_SDLC__INSTANCE_ID` matching
Fleet's `FLEET_CONTROL_TRACKER__INSTANCE_ID`; auth uses `TT_AUTH__CENTRAL_*`.
Neither orchestrator nor verifier configuration is required just to create.

201 creates issue + private Draft aggregate/binding + initial status history +
immutable creation result + `task.created` outbox event in one transaction.
No assignment, agent binding, PM dispatch or Workflow run is created here.
Idempotency namespace is `(project_id, central_subject, idempotency_key)`.
Concurrent identical payloads and retries after lost response/restart return
200 with the identical original seven-field result. Different payload is 409.
Replay rechecks live project access, active identity, original issue and exact
ownership/binding; it never repairs missing ownership or creates a replacement.
The historical result remains `Draft` after later stage progression; read
`GET /api/v1/issues/{task_id}/sdlc/context` for current state before dispatch.

Fleet must persist and reuse its creation key/payload until the outcome is
known, validate instance/project/root/owner on the typed result, then continue
its own assignment/runtime saga. Ordinary `POST /api/v1/issues` followed by
binding has no creation idempotency and must not be the saga source. Numbering
uses ordinary Tracker's maximum suffix (including deleted issues), uniqueness
constraint and bounded conflict retry, not an independent counter.

This extends the still-pending migration 000034, not a second migration.
Validate on a fresh database; do not reset, reapply or mutate an accepted shared
database that has already recorded an earlier version of 000034.

## Immutable PM Draft creation input: implemented readback

`GET /api/v1/issues/{id}/sdlc/pm-draft-input` returns `PmDraftInputResponse`:

```typescript
type PmDraftInputResponse = {
  contract_version: 1;
  tracker_instance_id: string;
  project_id: string; // UUID
  task_id: string; // UUID
  root_task_id: string; // UUID
  owner_subject: string;
  input: {
    snapshot_ref: string; // server-generated non-nil UUID
    title: string;
    description: string;
    sha256: string; // lowercase SHA-256 hex
  };
};
```

All fields are required and both objects reject unknown fields. All references
and binding identity are server-derived. The original title/description and
snapshot UUID are stored atomically in the append-only creation ledger, not
reconstructed from mutable issue fields. Hash bytes are UTF-8 compact JSON with
sorted keys, exactly `{"description":<JSON string>,"title":<JSON string>}`.
Non-ASCII characters are retained as UTF-8; JSON-required string escaping is
applied. There is no whitespace, Unicode or CRLF normalization, idempotency key,
snapshot UUID, operation name or other metadata in this content hash. This is
separate from the creation command replay hash and requirements document hash.
Title/description retain creation limits. Issue edits do not change the snapshot.
Concurrent creation, exact replay and server restart retain the same snapshot.
The existing seven-field `CreatedDraft` and `task.created` payload are unchanged.

Read authorization is exactly the existing `/context` resource policy: verified
Central Auth bearer, shared service read policy, active local central-subject
identity and explicit current project ownership/membership. This is not an
owner-only read; authorized members/operators and service-read PATs can read,
but gain no business confirmation rights. Local tokens, cookies-only requests,
disabled identities, foreign projects and admin/public bypasses are rejected.
Membership/account authorization rows and issue/aggregate use existing locks;
readback never bypasses revoked access or caches authorization. Missing/deleted
issue or binding is 404; unavailable Auth/SDLC is 503. Missing creation ledger,
historical all-null snapshot or inconsistent snapshot is 409, never a fallback
to current issue text or a fabricated original snapshot. Pending 000034 permits
only all-null or complete snapshot columns; complete refs are unique and non-nil.

Verification scope is Tracker HTTP/PostgreSQL and Rust/OpenAPI contract tests:
concurrent creation/readback, replay/restart, issue edit, exact UTF-8/CRLF and
Unicode composition vectors, append-only/partial-snapshot constraints, legacy
missing inputs, context ACL parity and unchanged operator confirmation denial.
It does not attest live PM integration or Workflow admission.

The bounded reservation below implements initial owner CAS and execution ordinal.
Trusted project mapping and Workflow authoritative namespace ownership/admission
remain external work. Replacement needs a new execution/ordinal; resume retains its
identity. Provenance/catalog/machine/hash values must come from trusted server
readback, never a browser trust blob. General Delivery contracts stay strict.
The opt-in `metadata_v1` outbox below bounds serialized responses. Legacy LIMIT
100 remains unchanged and full results can exceed Fleet's 1 MiB limits. Metadata
does not replace readable inputs or attest Workflow admission/runtime delivery.

## Initial PM Draft reservation (bounded implementation contract)

Creation acceptance readback is additive:
`GET /api/v1/projects/{project_id}/sdlc/drafts/operations/{idempotency_key}`.
The exact UTF-8 key is encoded as one path segment (`/` becomes `%2F`). Human
session plus fresh explicit project ACL is required; lookup is exclusively in
the authenticated author's project/key namespace, not operator impersonation.
200 returns the existing unchanged seven-field `CreatedDraft`, never raw content
or a new DTO/version. No retained operation is 404; retained operation with
missing/inconsistent entity, original binding or input is 409. This read never
creates, repairs or makes history current. Use it before POST replay on unknown
acceptance, then `pm-draft-input` for original source. Changed POST payload is
still 409; no legacy create/default behavior is changed.

`POST /api/v1/issues/{id}/sdlc/pm-draft-assignment` accepts exactly
`{expected_owner_version,expected_assignment_version,requested_agent_id,idempotency_key}`.
All fields are required, including explicit null for initial assignment version.
The agent UUID is only a canonical non-nil selector. Tracker has no configured
Fleet client and does not verify agent existence, role, configuration, chat,
workspace or runtime. In the response, `assignment.agent_id` retains exactly this
selector, not a concrete-agent verification receipt. No Fleet proof is fabricated.

Only a verified human owner session with current explicit project access may
reserve. Operator/PAT/orchestrator impersonation is forbidden. This initial start
permission is separate from exact requirements confirmation and never moves the
task to Backlog. Initial reservation requires a bound root Draft, verified complete
original input, owner version zero and no prior assignment/execution history.

Tracker allocates assignment/execution UUIDs, assignment version and positive
signed-64-bit ordinal in one transaction with snapshot linkage, owner-version CAS,
idempotency and the existing `pm.assigned` outbox event. The result is strict
`PmDraftReservation`: contract_version=1, variant=pm_draft_reserved, binding
{tracker_instance_id,project_id,task_id,root_task_id,owner_subject}, owner_cas
{expected_version,version}, existing five-field assignment, execution
{ordinal:<canonical decimal string>,key:SDLC-<ordinal>}, input {snapshot_ref,sha256},
assignment_operation_key=pm-draft:<assignment UUID>, admission_state=reserved,
dispatch_allowed=false. Machine subject comes from configured orchestrator
identity, never from the selector/browser; actual Base parent/child subject and
credential policy must be verified before later admission. This is not dispatch,
capability, workspace/lease, concrete-agent, Workflow or live acceptance evidence.

201 creates and 200 replays the same durable result; changed key payload/stale CAS
is 409. Owner versions use safe JSON integers; ordinal is an i64 decimal string.
Ordinal is Tracker-owned, never reset/reused; transaction rollback can leave gaps.
`SDLC-n` must always be qualified by Tracker instance across services.
`GET /api/v1/issues/{id}/sdlc/pm-draft-assignment[?idempotency_key=...]` returns
{contract_version,binding,owner_version,current,operation}; current is nullable
reservation, operation is null or {idempotency_key,request_sha256,result}. Reads
recheck context ACL; operation lookup uses the authenticated subject's namespace.
Historical result is not current authority and replay never restores current state.

New reserved enrollment blocks legacy assignment writes and every PM business
mutation until independently verified admission is implemented. Exact historical
legacy replay remains read-only. Existing non-enrolled flow/wire is unchanged.
No new metadata event type is emitted and pm.assigned remains the old assignment
DTO. Full admission must later verify actual Fleet agent/config/chat/business
workspace, owner lease/provenance, trusted project/Workflow namespace mapping,
native catalog/build manifest, scoped credential handoff and first-step gate.
Replacement remains fail-closed: trusted terminal readback plus durable Fleet and
Workflow quiescence/freeze acknowledgements under matching owner CAS are required;
task done, Workflow PASS, EOF or lease expiry do not suffice. Resume retains its
execution/ordinal. Required workspace proof is never substituted by null; genuinely
inapplicable Delivery/Tech/CI fields need explicit typed absence in the PM variant.
The full clarification/dispatch/recovery/resume/verifier/owner-confirmation and
general Delivery plan is preserved; this reservation slice does not complete it.

## PM execution ownership lease (not admission)

The lease producer applies only to a persisted current `pm_draft_reserved`
execution. It is an ownership TTL, not runtime heartbeat/readiness, authority for
run-side effects, native bundle verification, namespace claim or admission.
Reservation result JSON, owner version 1, assignment/execution/ordinal, metadata
events and the reserved business/legacy-write gates remain unchanged.

Machine-only `POST /api/v1/issues/{id}/sdlc/pm-draft-execution-lease` takes exactly
`{expected_owner_version,fence:MachineFence,idempotency_key}`. First claim is 201;
exact replay is 200. Any different claim key after a lease has ever existed is
409, even after expiry. No release, replacement or automatic reacquire exists.
`POST .../pm-draft-execution-lease/heartbeat` takes exactly
`{expected_owner_version,fence,lease_id,expected_lease_version,idempotency_key}`.
First renewal and exact historical replay both return 200. A new renewal requires
current lease ID/version CAS and `expires_at > clock_timestamp()` after locking;
version increments once and expiry becomes PostgreSQL now + 30 seconds. Clients
should renew every 10 seconds, but this cannot authorize a runtime heartbeat.
Owner/lease versions are positive safe JSON integers; all UUID inputs are
canonical and non-nil. All fields are required, objects reject unknown fields.

The eight-field `ExecutionLeaseReceipt` is
`{contract_version:1,binding:PmDraftBinding,owner_version,fence:MachineFence,
lease:{lease_id,version,holder_subject,claimed_at,heartbeat_at,expires_at},
ttl_seconds:30,heartbeat_seconds:10,dispatch_allowed:false}`. Dates are persisted
PostgreSQL UTC timestamps serialized with nine fractional digits and Z. The
immutable server-issued lease UUID identifies this single ownership generation;
version is renewal CAS, not a replacement generation. Replacement is closed and
needs a later explicit quiescence/fencing contract. No second SDLC allocator exists.

`GET .../pm-draft-execution-lease[?idempotency_key=...]` returns exactly
`{contract_version:1,binding,owner_version,fence,observed_at,
state:unclaimed|active|expired,current:ExecutionLease|null,
operation:{idempotency_key,request_sha256,result:ExecutionLeaseReceipt}|null,
dispatch_allowed:false}`. Both nullable fields are required. Expired current
remains an object; it is never changed to absent/unclaimed. Historical result can
have an older version/expiry than current and never attests live ownership.
Readback is observational, not a consume/dispatch API or a transferable capability.

Every read, command and replay uses verified Central machine bearer, exact
persisted `assignment.machine_subject`, existing grant
`task-tracker:sdlc:pm:<task>:<assignment>:<execution>:<agent>:<version>` and existing
Central service read/write policy. No new scope is introduced. Fresh active local
central-subject identity/project ACL, current owner cursor/pointer, assignment and
original input are rechecked under the existing locks. Holder is derived from
authenticated subject; no subject/credential/provenance authority is accepted in
the body. Central validates bearer identity; lease does not fabricate additional
parent/child/native provenance proof. Owner/operator human sessions cannot claim,
renew or read this machine-only resource.

Command identity is `(execution_id,verified_subject,idempotency_key)`; transport
request IDs have no replay semantics. Digests are canonical sorted-key UTF-8 JSON
`{operation:claim_pm_execution_lease|heartbeat_pm_execution_lease,payload:<command>}`.
Same key/payload returns the original receipt without touching expiry/version;
changed payload or operation is 409. Exact replay may succeed after expiry solely
as historical readback; clients must check fresh current state before relying on
ownership. Unknown heartbeat, stale fences, retained source inconsistency and
expired new renewal are 409. Unknown/expired lease cannot be automatically
reacquired; independent Fleet/Workflow quiescence remains a prerequisite to a
future recovery protocol. Auth/ACL denials and outages retain 401/403/503.

Pending 000034 adds only the lockable single-generation lease and append-only
operation ledger. Claim/renewal/ledger commit atomically; clock is read after
authorization/aggregate/lease locks. DB fences reject lease deletion, identity
replacement, non-monotonic renewal and expired updates. Full admission must still
verify all preceding workspace, namespace, native, credentials and first-step
requirements; this lease does not complete or narrow the full PM/Delivery plan.

## Fleet gateway

All paths below use the existing issue UUID under `/api/v1/issues/{id}/sdlc`.
Forward the original human Central Auth bearer. Tracker validates issuer, `sdlc`
audience, live session/token validity and `task-tracker:read/write` service grant
(browser sessions follow the shared service policy). Local JWTs are rejected.
Explicit project owner/member access is mandatory even with Central Auth enabled.
Consent additionally requires a human browser session and exact central owner
subject; an operator role does not grant proxy consent.

`GET /context` returns this typed shape (UUIDs serialize as UUID strings):

```typescript
type SdlcContext = {
  contract_version: 1;
  tracker_instance_id: string;
  project_id: string;
  task_id: string;
  root_task_id: string;
  owner_subject: string;
  stage: "Draft" | "Clarification" | "Backlog";
  requirement_revision: number | null; // 1..Number.MAX_SAFE_INTEGER when present
  waiting_reason: string | null;
  permissions: { can_answer: boolean; can_confirm: boolean };
  assignment: PmAssignment | null;
};
```

Fleet uses its fixed `TRACKER__URL` and configured `INSTANCE_ID`, validates all
binding IDs, and compares `owner_subject` with its middleware-verified
`central.user_id`. Display issue keys, email, local UUID and client URLs are never
cross-service identity. Missing SDLC binding returns 404, unavailable contracts
return 503, never an empty successful context.

- `GET /clarifications`: `{questions: ClarificationQuestion[]}`.
- `GET /requirements`: current full `RequirementsRevision` (404 if absent).
- `GET /requirements/revisions`: `{revisions: RequirementsRevision[]}`.
- `GET /requirements/{revision}`: immutable full revision.
- `GET /requirements/{revision}/diff?against={revision}`: full typed before/after.
- `POST /clarifications/{question_id}/answers`:
  `{expected_question_version, requirement_revision, selected_option_ids,
    text, comment, idempotency_key}`. Returns a durable typed answer.
- `POST /requirements/{revision}/confirm`: `{content_hash,idempotency_key}`.
  Returns the exact durable confirmation and `stage: "Backlog"`.

Questions have UUID `id/request_id/task_id/root_task_id`, safe integer `version`
and `requirement_revision`, requirement reference, assignment/execution/agent/
checkpoint identity, text, rationale, required, mode `single|multiple|text`,
stable options `{id,label,consequences,is_custom}`, optional recommendation
option ID, state `open|answered|superseded|cancelled`, and optional durable answer.
No recommendation is a submitted default. Authors and timestamps are server
derived. Replays preserve the original result; changed payloads return 409.

Requirements contain `goal`, `scope`, `exclusions`, `scenarios`,
`acceptance_criteria`, `constraints`, `dependencies`, `assumptions`, plus explicit
required `checklist` and `prerequisites` IDs. A revision contains that full document,
revision number, SHA-256 content hash, author and timestamp. Hashing uses compact
JSON of the typed document with alphabetically sorted object keys, UTF-8 bytes;
array order is significant. Never hash Markdown or a partial summary.

Machine API requires centrally verified non-session credentials, an exact
server-persisted assignment subject and a grant binding task, assignment,
execution, concrete agent and assignment version. Human requests cannot publish
questions/revisions. Readiness evidence needs a separately trusted verifier;
PM credentials and owner claims cannot mark checks passed. Missing verifier
contract/configuration fails closed. Confirmation evaluates persisted evidence
for the exact document hash and revision inside the same transaction as Backlog,
idempotency result and outbox event.

Operational provisioning, machine DTOs and verification evidence details are
documented here alongside the implementation before handoff.

## Fleet DTO agreement (1 October 2026)

Tracker stable instance configuration is `TASKTRACKER_SDLC__INSTANCE_ID` (a
nonempty stable string). Fleet must configure the identical value in
`FLEET_CONTROL_TRACKER__INSTANCE_ID`. It is never generated on restart. The
existing Central Auth bridge uses `TT_AUTH__CENTRAL_*` configuration.

All public DTO fields are snake_case. UUIDs are strings, timestamps RFC3339;
versions/revisions are positive integers bounded by `Number.MAX_SAFE_INTEGER`
(`9007199254740991`); commands outside this range are rejected and version
increments stop at this boundary. Absent optional response
values serialize as `null`. Unknown command fields (including author/ready) fail.

```typescript
type Option = { id: string; label: string; consequences: string; is_custom: boolean };
type Answer = {
  id: string; question_id: string; question_version: number;
  requirement_revision: number; selected_option_ids: string[];
  text: string | null; comment: string | null;
  author_subject: string; created_at: string;
};
type Question = {
  id: string; request_id: string; task_id: string; root_task_id: string;
  version: number; requirement_revision: number;
  requirement_reference: string | null;
  assignment_id: string; execution_id: string; agent_id: string;
  assignment_version: number; checkpoint_id: string;
  text: string; rationale: string; required: boolean;
  mode: 'single' | 'multiple' | 'text'; options: Option[];
  recommended_option_id: string | null;
  state: 'open' | 'answered' | 'superseded' | 'cancelled';
  answer: Answer | null; author_subject: string; created_at: string;
};
type RequirementsDocument = {
  goal: string; scope: string[]; exclusions: string[]; scenarios: string[];
  acceptance_criteria: string[]; constraints: string[]; dependencies: string[];
  assumptions: string[]; checklist: string[]; prerequisites: string[];
};
type RequirementsRevision = RequirementsDocument & {
  revision: number; content_hash: string; author_subject: string; created_at: string;
};
type PmAssignment = {
  assignment_id: string; execution_id: string; agent_id: string;
  version: number; machine_subject: string;
};
type Confirmation = {
  id: string; task_id: string; revision: number; content_hash: string;
  owner_subject: string; created_at: string; stage: 'Backlog';
};
type AnswerCommand = {
  expected_question_version: number; requirement_revision: number;
  selected_option_ids: string[]; text: string | null; comment: string | null;
  idempotency_key: string;
};
type ConfirmCommand = { content_hash: string; idempotency_key: string };
```

`GET /context` uses the shape above; capabilities are inside `permissions`.
`GET /clarifications` returns `{questions: Question[]}` (latest question versions,
including answered/cancelled/superseded). `GET /requirements/revisions` returns
`{revisions: RequirementsRevision[]}` in ascending revision order. Revision fields
are flat, not wrapped in `document` or `content`. `GET /requirements` and
`GET /requirements/{revision}` return the same full flat revision. `GET
/requirements/{revision}/diff?against=N` returns `{before,after}` full revisions.
Answers return `Answer`; confirmation returns `Confirmation`. A custom option
(`is_custom:true`) requires nonblank `text`; recommendations never count as answers.
`can_confirm` is true only for the owner session and an actually ready current
revision. This capability is a snapshot; POST rechecks under the task lock.

This section is the implementation DTO agreement, not evidence of deployment or
live PM acceptance. Verification results will be recorded separately.

Publishing a question invalidates confirmation readiness. Answering/cancelling
questions does not restore it: PM must publish a new requirements revision after
answers. Both checklist and prerequisites must be nonempty, with unique IDs;
each requires exact-revision/hash evidence from the configured trusted verifier.
Roots from a different project are rejected, even for users with access to both
projects; root existence/deletion and same-project access are checked on binding.

## Provisioning and machine commands

SDLC is opt-in. Set `TASKTRACKER_SDLC__INSTANCE_ID` to enable the repository;
absent configuration returns 503 on SDLC routes. Initialization stores the
instance identity in PostgreSQL and refuses a changed instance ID at restart.
Set `TASKTRACKER_SDLC__ORCHESTRATOR_SUBJECT` to the trusted assignment provisioner
and `TASKTRACKER_SDLC__VERIFIER_SUBJECT` to a separate trusted readiness verifier.
Empty provisioner disables assignment; empty verifier disables confirmation.
Provision these central subjects as active local shadow users and explicit
project members (or project owner). There is no automatic machine membership,
global-admin read exception, or email-based identity matching.

All command DTOs are strict: unknown fields return 422, particularly `author`,
`author_subject`, `owner_subject`, `ready` and `passed`. Server derives authors
and timestamps. All paths below share `/api/v1/issues/{id}/sdlc`:

```typescript
type Fence = {
  assignment_id: string; execution_id: string; agent_id: string;
  assignment_version: number;
};
type BindCommand = { root_task_id: string; idempotency_key: string };
type AssignCommand = {
  assignment: PmAssignment; expected_assignment_version: number | null;
  idempotency_key: string;
};
type PublishQuestion = {
  fence: Fence; request_id: string; question_id: string;
  expected_question_version: number | null; requirement_revision: number;
  checkpoint_id: string; requirement_reference: string | null;
  text: string; rationale: string; required: boolean;
  mode: 'single' | 'multiple' | 'text'; options: Option[];
  recommended_option_id: string | null; idempotency_key: string;
};
type PublishRevision = {
  fence: Fence; expected_requirement_revision: number | null;
  document: RequirementsDocument; idempotency_key: string;
};
type CancelQuestion = {
  fence: Fence; expected_question_version: number; idempotency_key: string;
};
type EvidenceCommand = {
  fence: Fence; requirement_revision: number; content_hash: string;
  check_id: string; evidence_reference: string; idempotency_key: string;
};
type Evidence = {
  requirement_revision: number; content_hash: string; check_id: string;
  evidence_reference: string; verifier_subject: string; created_at: string;
};
```

- `POST /binding` (`BindCommand -> SdlcContext`): human session only, exact
  issue reporter central subject plus strict project access. Task owner is
  persisted from the issue reporter, never from the command. A root must exist,
  be live, and belong to the same authorized project. Existing bindings are
  immutable and cannot be repointed by issue edits or another binding command.
- `POST /assignment` (`AssignCommand -> PmAssignment`): configured non-session
  orchestrator only, `task-tracker:write` and `task-tracker:sdlc:assign` grants.
  First version is 1; replacements increment exactly by 1 and fence old open
  questions as superseded. The instance/task/agent binding stays unique.
- `POST /requirements` (`PublishRevision -> RequirementsRevision`): current
  assigned non-session PM only; initial `expected_requirement_revision:null`,
  then exact current revision. Publish the initial full revision before asking
  questions. Mandatory open questions block further publication. Changed
  requirements after Backlog create a new Clarification revision requiring new
  evidence and owner consent; old confirmations remain historical.
- `POST /clarifications` (`PublishQuestion -> Question`): current assigned PM
  only, exact current unconfirmed requirements revision. Caller supplies stable
  UUIDs for request/question/options/checkpoint; first expected version is null.
  Editing an existing question requires its exact version; request binding
  stays fixed, changed option meaning requires a new option ID. Old versions
  and answers remain durable history; list returns latest versions.
- `POST /clarifications/{question_id}/cancel` (`CancelQuestion -> Question`):
  current assigned PM only; exact open question version.
- `POST /evidence` (`EvidenceCommand -> Evidence`): configured non-session
  verifier only, distinct from PM, with `task-tracker:write` and exact evidence
  grant. A successful command is a trusted verifier attestation of a completed
  check, bound to current revision/hash and known checklist/prerequisite ID.
  There is no user/PM boolean readiness endpoint. The separate verifier must
  actually perform its check before calling; that external component is outside
  Tracker and must be integrated before live acceptance.

PM requires the exact grant
`task-tracker:sdlc:pm:<task>:<assignment>:<execution>:<agent>:<version>` plus
`task-tracker:write`. Verifier requires
`task-tracker:sdlc:evidence:<task>:<version>` plus `task-tracker:write`.
Every machine write also checks all four fence fields and current persisted
machine subject. Browser credentials cannot publish or attest checks. Browser
sessions follow shared service policy; non-session reads always need
`task-tracker:read`, including PM and operators.

## Persistence, delivery and errors

Task-command idempotency is scoped by task + central subject + key; project-scoped
creation uses its separate namespace above. Keys are 1..128 UTF-8 bytes without
whitespace/control characters.
Payload hashing includes the operation and path question/revision identity,
uses canonical sorted-key compact JSON, and treats selected option IDs as a
set. Changed payload with the same key returns 409; identical replay returns
the original stored answer/confirmation, including IDs/time, even after a
restart. Authorization and active membership are rechecked before readback.
Binding replay returns its original context snapshot, so refetch `/context`.

Business state, immutable historical rows, idempotency result, issue status and
outbox event commit together under issue/task locks. Outbox failure rolls back
the answer/confirmation. Ordinary issue updates, board moves, transitions and
sprint moves cannot bypass the gate, enforced by a PostgreSQL issue trigger.
Unconfirmed issues use `SDLC Draft`/`SDLC Clarification`; confirmation moves to
`Backlog` and clears sprint assignment. History tables reject UPDATE/DELETE;
hard purge is blocked by foreign keys to retained SDLC history.

`GET /events?after=0` returns `{events:[{sequence,event_id,task_id,event_type,
payload,created_at}]}`, at most 100 ordered records for this authorized task.
Event payload carries contract version, immutable binding, stage, revision and
the durable command result. The next cursor is the last returned `sequence`;
empty pages preserve the previous cursor. Stable UUID event IDs are Fleet inbox
deduplication keys. Tracker keeps this log across restarts; Fleet must persist
its cursor/inbox. An event means Tracker accepted the command, not that Workflow
resumed or a runtime run started. No HTTP delivery worker or Fleet acknowledgement
is implemented inside Tracker.

Status codes: 401 invalid/local/expired/revoked bearer; 403 scope, membership,
owner or machine identity denied; 404 missing issue/binding/question/revision;
409 stale fence/version/hash, reused key with changed payload, not-ready gate;
422 invalid DTO/answer or unsafe versions; 503 Central Auth/storage configuration
unavailable. Unexpected database/transaction errors return 500, never acceptance.

## Byte-bounded metadata outbox

`GET /api/v1/issues/{id}/sdlc/events?projection=metadata_v1&after=0&limit=100&max_bytes=262144`
uses the same strict Central/service/project read authorization as legacy events.
Without projection the existing query parsing, LIMIT 100, response and payload
remain unchanged, including ignoring limit/max_bytes. Explicit unknown projection
or invalid metadata query returns 422 with a static standard validation error.
Opt-in after is canonical decimal i64 (0..9223372036854775807); limit is 1..100
(default 100); max_bytes is 1024..1048576 (default 262144). Decimal query values
reject signs, leading zeroes, whitespace, fractions and overflow.

Success is the strict object `{contract_version:1,projection:"metadata_v1",
after:string,next_after:string,has_more:boolean,events:MetadataEvent[]}`. Every
event has exactly `{sequence:string,event_id:UUID,task_id:UUID,event_type,
created_at:UTCDateTime,metadata_sha256:SHA256,payload}`. Sequence is a positive
canonical decimal i64 string, not a JavaScript number. Timestamp comes from the
persisted event and is normalized to UTC with nine fractional digits and Z.
Payload has exactly `{tracker_instance_id,project_id,root_task_id,owner_subject,
stage,current_requirement_revision:Version|null,resource}`. Version is an integer
in 1..9007199254740991; SHA256 is 64 lowercase hexadecimal characters. All fields
are required, including nullable fields; all objects reject unknown fields.

| event_type | exact resource |
|---|---|
| task.created | `{input:null\|{snapshot_ref:UUID,sha256:SHA256}}` |
| task.bound | `{}` |
| pm.assigned | `{assignment_id:UUID,execution_id:UUID,agent_id:UUID,assignment_version:Version}` |
| clarification.published | `{question_id:UUID,question_version:Version,request_id:UUID,checkpoint_id:UUID,requirement_revision:Version,state:"open",fence:Fence}` |
| clarification.cancelled | Same references, `state:"cancelled"` |
| clarification.answered | `{answer_id:UUID,question_id:UUID,question_version:Version,request_id:UUID,checkpoint_id:UUID,requirement_revision:Version,fence:Fence}` |
| requirements.published | `{requirement_revision:Version,content_hash:SHA256}` |
| requirements.evidence_recorded | `{requirement_revision:Version,content_hash:SHA256,check_id_sha256:SHA256}` |
| requirements.confirmed | `{confirmation_id:UUID,requirement_revision:Version,content_hash:SHA256}` |

Fence has the same four fields as pm.assigned. No result, aggregate, document,
answer text, options, evidence reference or raw check_id is emitted. Requirements
have a task/revision identity, no revision UUID. Evidence result/outbox has no
durable evidence-row UUID, so event_id identifies the recorded occurrence.
check_id_sha256 hashes the canonical JSON string, not raw UTF-8 check_id bytes.
Metadata/context values come from the immutable event payload; only immutable
binding is checked against the authorized task. Answer request/checkpoint/fence
come from the exact append-only question version, never current assignment.
Creation input refs come from the immutable creation ledger; absent historical
input is null, partial/nil/hash/binding conflicts fail closed. Original content
hashes remain unchanged and separate from metadata digest.

metadata_sha256 hashes `{contract_version:1,projection:"metadata_v1",event:<event
without metadata_sha256>}` using the existing canonical sorted-key compact UTF-8
JSON Value algorithm. JSON-required escapes apply; there is no Unicode, CRLF or
whitespace normalization and no JSONB-format-dependent hashing. Pagination does
not affect the event digest; replay, restart and later task edits retain it.

The task lock fences writers while selecting the first limit+1 rows ordered by
sequence. Return only the contiguous task-event prefix that fits the serialized
whole success envelope, including escaping, multibyte strings, cursors, hash and
has_more. Global sequence gaps from other tasks/rollback are normal. The bytes
measured are the bytes returned, without a second HTTP JSON serialization.
has_more includes any extra/unreturned row, including a blocked row. next_after
is the last returned sequence; empty tails preserve after. Never filter/skip an
unknown, malformed, conflicting or oversized row. A valid preceding prefix may
be returned with has_more=true before exposing the blocker on the next request.

If the first event cannot fit, a bounded strict error has exactly
`{contract_version:1,projection:"metadata_v1",code,after:string,
blocked_sequence:string,event_id:UUID,required_bytes:number|null,max_bytes:number}`.
422 metadata_budget_too_small means the whole singleton envelope can fit the
hard cap; required_bytes reports its serialized length. 409
metadata_event_unrepresentable means it exceeds the hard cap; 409
metadata_source_invalid means missing/invalid references or unsupported source
(required_bytes:null). Static codes reveal no result/text. Projection errors are
serialized once, below 1024 bytes, and never advance cursor. Dependency/database
and authorization errors retain their existing standard envelopes/statuses.

Fleet must atomically pin instance/task/projection/contract_version before its
first cursor, including an empty first page. Legacy receipts are not rehashed or
reinterpreted; switching existing consumption requires an explicit migration.
Store/verify canonical metadata digest separately from legacy payload hashes;
conflicting replay/version fails closed. Inbox insertion and cursor persistence
share a transaction. Fleet also bounds its complete inbox request after wrapping
against its own 1 MiB limit; 256 KiB default leaves headroom. Blocking errors need
durable diagnosis/backoff, not skipping or tight retry loops. These Fleet duties
and Workflow resume are external integration work, not implemented in Tracker.
