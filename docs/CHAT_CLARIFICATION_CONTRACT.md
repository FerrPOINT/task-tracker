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
