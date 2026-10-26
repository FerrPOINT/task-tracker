# Configured CLI V1

Status: producer/assignment implementation; not installed or live accepted.
Workflow terminal readback/completion integration is pending the shared wire.

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

The configured service credential contains the stable scope
`task-tracker:sdlc:agent:<canonical-agent-uuid>` and ordinary Tracker app scopes.
Central Auth must verify service_account. Human credentials and mixed legacy
task grants are rejected. Every task read/write verifies the current assignment
subject and agent under the task lock, as well as existing active user/project
membership. A caller-provided agent ID or environment variable is not authority.
The reservation does not mint or delegate credentials.

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

Owner answers retain AnswerCommand and version CAS. Owner confirmation retains
the exact revision/hash and existing readiness/evidence checks. Only the owner
can answer/confirm; PM reads the saved confirmation via
`GET /api/v1/issues/{task}/sdlc/requirements/{revision}/confirmation`.
No automatic Analysis dispatch is added.

Configured CLI mode is selected by FLEET_CLI_PROTOCOL. It requires canonical
FLEET_AGENT_ID, nonempty TASKTRACKER_TOKEN and TASKTRACKER_API_URL. An alternate
token/origin and global PDLC/SDLC token fallback are refused. Manual legacy mode
and `--from-file -` stdin are unchanged. The separately pinned runtime CLI cohort
must include this CLI change before live acceptance.

## Integration And Checks

Integrator registers `m20261026_0000105_configured_cli` in both migration
lineages, exports the Rust API and updates generated clients. The migration is
forward-only and does not rewrite business or legacy native history.

Prepared tests: app::sdlc_configured_cli::tests (4), CLI agent_credentials::tests
(2), migration 105 unit (1), and candidates::configured_cli_tests real owned-PG
business ledger case (1). The PG case uses the existing candidates fixture and
synthetic service identities, not live Central Auth or Workflow acceptance.
No Cargo/PG execution or installation is claimed by this source packet.

Completion must not use legacy scheduler credentials or manufacture lease/native
receipts. The shared Workflow receipt endpoint/closed shape must be agreed and
read server-to-server before enabling configured completion. This first producer
packet does not claim that completion is implemented.
