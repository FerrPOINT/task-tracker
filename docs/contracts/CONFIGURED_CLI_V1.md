# Configured CLI V1

Recorded status (2026-10-27): Tracker backend `d5fca554` is installed as image
`3f084e40`. Focused Rust/PG checks, forward-105 populated-history preservation
and Rust OpenAPI/client parity passed on their recorded source revisions.
See the [Tracker installation record](../../CHANGELOG.md#2026-10-26-configured-cli-pm--source-verified-tracker-installed).

The existing PM11/FLEETQA-8 task has assisted owner-confirmed Backlog/Ready
acceptance, including authoritative Workflow completion and exact completion
replay, with no Analysis intent or downstream assignment. Fresh FLEETQA-9
acceptance remains open; this is not full release or merge readiness.
See [Fleet's recorded state](https://github.com/FerrPOINT/fleet-control/blob/39db485d1aa64f9844ecf434b7b3ad1fe90b26ef/docs/CURRENT_STATE.md#configured-cli-replacement-in-progress).

The newer CLI discovery/payload help at `80f62d13` is source-qualified: all 30
Linux CLI workflow tests passed. Its CLI-only runtime candidate is qualified,
but the updated CLI is not yet installed or active in the retained PM runtime.
Source qualification does not establish fresh PM acceptance.
See the [CLI qualification record](../../CHANGELOG.md#2026-10-27-pm-publish-cli-discovery).

## Creation And Reservation

Existing owner APIs and their idempotency keys are retained. There is no token
issuer in this protocol. The owner uses the configured persistent PM subject.

`POST /api/v1/projects/{project}/sdlc/drafts` accepts the existing title,
description and idempotency_key plus flow `single-task-v1` and the explicit
interaction_protocol `configured-cli-v1`. The CreatedDraft response returns the
same marker. Omission remains legacy; explicit null or an unknown value is invalid.

`POST /api/v1/issues/{task}/sdlc/pm-draft-assignment` accepts the existing
expected_owner_version (initially 0), expected_assignment_version (initially null),
requested_agent_id and idempotency_key. Configured requests additionally require
the exact interaction_protocol and machine_subject, a canonical nonnil Central
Auth subject UUID distinct from the owner, scheduler, orchestrator and verifier.
The existing project PM routing default still constrains requested_agent_id.

The response is PmDraftReservation plus the marker. In particular,
assignment.machine_subject is the persistent identity and execution.key is the
existing server-generated `SDLC-N`, not a Hermes run. dispatch_allowed remains
false: the reservation is not native execution authority. GET of the same
assignment endpoint with idempotency_key returns the original operation/current
reservation. Exact replay is 200; creation is 201; changed payload conflicts.

## Agent Authorization

The persistent service credential uses ordinary `task-tracker:read` and
`task-tracker:write` scopes (both required), with ordinary
`project-workflow:read`/`project-workflow:write` for the Workflow CLI. No custom
agent scope or per-task token is required. Central Auth must verify
service_account; the verified principal is neither a human session nor a trusted
human PAT. Human credentials and mixed legacy PM/delivery task grants are
rejected. The explicit task protocol selects configured authorization. Every task
read/write verifies the exact canonical assignment.machine_subject and current
assignment under the task lock, as well as existing active user/project
membership. Command agent/execution/assignment/version fences come from that
assignment; a caller-provided agent ID or environment variable is not authority.
The reservation does not mint or delegate credentials.

Initial project admission uses the existing owner-only member API with the
explicit Central Auth subject UUID and project role `member`, not the agent's
profession. Service-account lookup requires the caller's existing administrative
Auth session and verifies current identity activity before local JIT. Service
accounts are not added to the human directory. Keep a Workflow-only credential
for the Workflow CLI: its existing scope contract rejects other application
scopes even when the account itself legitimately works in both applications.

The separate server readback credential retains the existing exact
`{task-tracker:read}` machine-reader admission and current active project ACL.
It can read context, requirements and the saved confirmation without being the
assigned PM or a configured scheduler subject. It cannot publish, cancel,
confirm or complete. A foreign subject with read+write is not a read-only reader
and cannot borrow this exception; PM mutations still require the exact assigned
service subject and current fence. No reader registry or new permission is added.

The capability permits only the existing context, input, clarification,
requirements, confirmation-read and completion business routes. It does not
permit owner answers/confirmation, assignment changes, scheduler or native
execution lease endpoints. The new marker is immutable from original creation;
legacy histories cannot be retrofitted. Configured tasks cannot acquire a native
execution lease. Existing applied migrations are unchanged.

## Business Commands

Context returns interaction_protocol, current assignment, workflow_task_key,
expected_assignment_version and current requirement revision/content_hash.
Requirements publication retains the existing document, assignment fence,
expected_requirement_revision and idempotency key. An initial working revision
can precede questions. Configured question publication retains the same business
fields but omits checkpoint_id. Legacy publication still requires a nonnil
checkpoint. No fabricated checkpoint is stored for configured questions.

Owner answers retain AnswerCommand and version CAS. Only the owner can confirm
the exact current revision/hash after a valid business document and resolution
of mandatory questions, under the current assignment and existing project ACL.
For configured-cli-v1 this does not require native/checkpoint proof, a trusted
readiness verifier, or evidence for each checklist/prerequisite. These document
items are business content, not synthetic infrastructure evidence. Legacy
readiness/evidence behavior is unchanged. PM reads the saved confirmation via
`GET /api/v1/issues/{task}/sdlc/requirements/{revision}/confirmation`.
The confirmation receipt truthfully reports `stage: Draft`; the aggregate remains
Draft/InProgress with `waiting_reason: waiting_for_workflow_completion`. Confirmed
requirements are immutable. No routing/source snapshot or Analysis is created
by configured consent. CLI/deployment readiness is qualified separately.

Configured CLI mode is selected by FLEET_CLI_PROTOCOL. It requires canonical
FLEET_AGENT_ID, nonempty TASKTRACKER_TOKEN and TASKTRACKER_API_URL. An alternate
token/origin and global PDLC/SDLC token fallback are refused. Manual legacy mode
and `--from-file -` stdin are unchanged. The separately pinned runtime CLI cohort
must include this CLI change before live acceptance.

## Completion

The existing `task-tracker sdlc pm completion submit --task <uuid> --from-file -`
reads this closed JSON body from stdin (no token/receipt materialization):

```json
{
  "interaction_protocol": "configured-cli-v1",
  "fence": {
    "assignment_id": "<current assignment UUID>",
    "execution_id": "<Tracker execution UUID>",
    "agent_id": "<current agent UUID>",
    "assignment_version": 1
  },
  "requirement_revision": 2,
  "content_hash": "<actual current requirements SHA256>",
  "namespace_id": 2,
  "workflow_execution_ref": "<Workflow enrollment execution UUID>",
  "workflow_operation_key": "<saved terminal report key>",
  "workflow_result_version": 4,
  "idempotency_key": "<stable completion key>"
}
```

Numbers/UUIDs above are selectors/examples, never defaults. They must come from
current Tracker context and the real saved Workflow result. The same body goes
to `POST /api/v1/issues/{task}/sdlc/pm-draft-completion`. The current assigned PM
can call it; the owner, admin PAT or scheduler cannot substitute for that PM.
Unknown fields (including caller-supplied receipts/native flags) are rejected.

Tracker uses fixed server settings TASKTRACKER_SDLC__WORKFLOW_URL and
TASKTRACKER_SDLC__WORKFLOW_READ_TOKEN, the existing trusted PM assignment-service
read integration, NOT the agent credential. Both must be configured together.
The reader makes one bounded/no-redirect/no-proxy/no-retry POST to the existing
`/internal/runtime/terminal-receipt/readback`. This POST only reads history.
Selectors are protocol, task SDLC-N, namespace_id, execution_ref, operation_key,
concrete_agent_ref, assignment_ref, requirements_revision/sha256, result_version.
No URL or credential is accepted from the completion caller.

Closed response is `{ok:true,complete:true,receipt:{...},workflow_receipt:{reference,sha256}}`.
The receipt includes the selectors plus tracker_task_id, request_sha256,
owner_subject, confirmation_id, history_id, status `done`. Tracker additionally
requires tracker_execution_id, assignment_version and machine_subject from the
immutable Workflow enrollment. The recorded assisted acceptance exercised this
shared receipt tuple; missing fields still fail closed.
The canonical hash/reference and full tuple are checked, then current assignment,
revision/hash and real saved owner confirmation are rechecked under Tracker lock.
No transaction spans HTTP. A completed Hermes request is not a prerequisite.

The configured result has interaction_protocol, completion_id, Tracker binding,
fence/owner_version, requirement_revision/content_hash, confirmation_id, the
verified workflow_receipt/hash, stage Backlog, status Ready, admission transition,
idempotency_key and created_at. No lease/native receipt is synthesized. Its
append-only ledger, exact Draft-to-Backlog/Ready aggregate and issue projection,
and admission transition commit atomically with pm.completed. Until that commit
the task remains Draft, including while Workflow completes. Failed/foreign/stale
terminal receipt leaves it Draft; consent alone cannot publish. The original
confirmation receipt remains Draft even after publication. No Analysis intent,
assignment or downstream dispatch is created. Legacy confirmation timing is
unchanged.

Configured tasks are explicitly excluded before pagination from existing delivery
candidate discovery. The existing EnsureQueued and Reserve consumer commands
also reject this protocol, even when the project enables single-task delivery.
Project enablement alone is not opt-in to Analyst or later roles. No new queue
or authority flag is introduced; legacy discovery and consumers are unchanged.

Exact replay is 200, first commit is 201, changed key payload is 409. GET of the
same completion route returns `{interaction_protocol,current}`; GET
`.../pm-draft-completion/operations/{key}` returns key/request_sha256/result.
Existing CLI `completion read` and `completion operation` expose these readbacks.
Legacy completion DTOs/readbacks remain on their original branch.

## Integration And Checks

The installed backend includes `m20261026_0000105_configured_cli`, registered in
both migration lineages, with the Rust API export and generated clients updated.
The migration is forward-only and does not rewrite business or legacy native history.

Original focused test inventory: app::sdlc_configured_cli::tests (4),
app::sdlc_configured_cli::completion_tests (8), CLI agent_credentials::tests (2),
migration 105 units (3), and candidates::configured_cli_tests real owned-PG case
(1). The latter now covers owner answers/confirmation without verifier/evidence,
Draft-only consent/readback, refusal of publication before completion, foreign/
stale terminal tuples, competing completion replay, changed payload, append
immutability and no lease/native/evidence/routing/source/Analysis rows.
It uses the existing candidates fixture and synthetic service/Workflow receipts,
not live Central Auth/provider acceptance. The HTTP unit exercises the actual
reader against a controlled server, not a real Workflow deployment.
The PG case requires 105 in the real Migrator registry before fixture setup,
then verifies its actual ledger row and installed function after ordinary up.
There is no direct SQL leaf fallback or synthetic migration ledger insertion.
The same PG case verifies empty delivery candidates and refusal of EnsureQueued/
Reserve after completion without data changes or queue/hold/assignment rows.
The pure `sdlc_candidates::tests::configured_cli_pm_never_opts_in_to_downstream_discovery`
case also refuses hypothetical downstream heads while preserving legacy eligibility.

Reference scoped commands (105 is registered in the installed backend):

```sh
cargo test -p app sdlc_configured_cli::
cargo test -p task-tracker-cli --bin task-tracker agent_credentials::tests::
cargo test -p migration m20261026_0000105_configured_cli::tests::
cargo test -p infra --lib sdlc::candidates::tests::configured_cli_tests:: -- --ignored --test-threads=1
```

The PG selector requires a fresh disposable TT_CANDIDATES_TEST_DATABASE_URL
ending `_candidates_test`. The recorded qualification above is separate from
this status correction; no new Cargo/PG run, installation or activation was performed.
