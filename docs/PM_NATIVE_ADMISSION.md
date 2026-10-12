# PM native admission

Source, owned PostgreSQL history checks and the diagnostic native PM roundtrip
are qualified. Final coordinated immutable artifacts remain pending.

The registered PM actor cannot supply readiness in its command. For a reserved
execution, the service authorizes the command and reads the original receipt or
reservation under the task lock. It releases that transaction before requesting
the configured Fleet owner.

`GET /internal/runtime/v1/pm/executions/{execution_id}/admission` is private Fleet
readback. Tracker accepts strictly versioned closed responses:

- `contract_version=1` is the original native custody proof. Its DTO and
  validator remain unchanged.
- `contract_version=2`, `profile=fleet_mcp_custody_v1` is the Fleet MCP custody
  proof for the PM-first Fleet integration. It proves the effective Fleet
  configuration, accepted Hermes run/session/binding, immutable dispatch request
  hash and the configured/advertised Fleet MCP surface. It does not claim live
  provider inventory, model output, pre-model hooks or fabricated tool names.

Both profiles bind assignment, agent, execution, task/root/project/instance,
run/session/fence and configuration hash; neither is caller-provided readiness.
Unknown versions, unknown profiles and unknown DTO fields are rejected.

Tracker uses deployment-only `TASKTRACKER_SDLC__FLEET_NATIVE_URL` and
`TASKTRACKER_SDLC__FLEET_NATIVE_READ_TOKEN`. Redirects are disabled and the response
is bounded. Tracker then reauthorizes, compares reservation, validates freshness
and lease and checks the monotonic run fence before committing. Additive migration
000091 stores immutable versioned observation history with the command
transaction. Repeated history for the same execution/run may refresh
`observation_ref` and `observed_at`, but cannot change the profile, identity,
fence, accepted run/session/binding, configuration revision/hash or dispatch
request hash.

Historical admission is not a standing database write grant. When an authorized
command changes a reserved task, the same transaction records its exact next state
in `sdlc_pm_state_write_permits`; the reservation trigger consumes that one-use
permit while applying the state update. A failed command rolls back both permit
and state, and an unscoped update remains rejected after admission.

Machine writes need current observation. Owner answer/confirmation and the
separate verifier require existing history, allowing the original owner response
while PM waits. Generic assignment replacement and stale/foreign fences remain
refused. Original idempotent receipts remain readable during an admission outage.

The initial reservation remains `reserved` with `dispatch_allowed=false`, separate
from native observation. Analysis admission/dispatch is outside this PM vertical.

Required qualification: real PostgreSQL command/rollback/history, actual native
roundtrip across the three owners, restart/refusals and final integrated checks.
