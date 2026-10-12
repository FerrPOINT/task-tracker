# PM producer lease and historical admission

Source, owned PostgreSQL history checks and PM producer lease checks are
qualified. Final coordinated immutable artifacts remain pending.

The registered PM actor cannot supply readiness in its command. For a reserved
execution, the service authorizes the command and reads the original receipt or
reservation under the task lock. PM producer writes now use Tracker-owned
reservation, exact machine subject/scope/fence and a current execution lease;
they do not call Fleet for `/internal/runtime/v1/pm/executions/{execution_id}/admission`.

Legacy admission rows remain strictly versioned closed responses:

- `contract_version=1` is the original native custody proof. Its DTO and
  validator remain unchanged.
- `contract_version=2`, `profile=fleet_mcp_custody_v1` is the Fleet MCP custody
  proof for the PM-first Fleet integration. It proves the effective Fleet
  configuration, accepted Hermes run/session/binding, immutable dispatch request
  hash and the configured/advertised Fleet MCP surface. It does not claim live
  provider inventory, model output, pre-model hooks or fabricated tool names.

Both historical profiles bind assignment, agent, execution, task/root/project/
instance, run/session/fence and configuration hash; neither is caller-provided
readiness. Unknown versions, unknown profiles and unknown DTO fields are
rejected by the historical validator.

Additive migration 000091 stores immutable v1 observation history with the
command transaction for installations that already collected it.
Additive migration 000093 permits the v2 profile in existing installations
without changing the original migration or rewriting history. Its downgrade
refuses while v2 history exists. Repeated history for the same execution/run may refresh
`observation_ref` and `observed_at`, but cannot change the profile, identity,
fence, accepted run/session/binding, configuration revision/hash or dispatch
request hash.

Historical admission is not a standing database write grant. When an authorized
command changes a reserved task, the same transaction records its exact next state
in `sdlc_pm_state_write_permits`; the reservation trigger consumes that one-use
permit while applying the state update. A failed command rolls back both permit
and state, and an unscoped update remains rejected.

PM producer writes need a current Tracker execution lease held by the assigned
machine. Owner answer/confirmation and the separate verifier still use existing
history when present, allowing the original owner response while PM waits.
Generic assignment replacement, missing/expired/foreign leases and stale/foreign
fences remain refused. Original idempotent receipts remain readable without Fleet
native readback.

The initial reservation remains `reserved` with `dispatch_allowed=false`, separate
from native observation. Analysis admission/dispatch is outside this PM vertical.

Required qualification: real PostgreSQL command/rollback/history, actual native
roundtrip across the three owners, restart/refusals and final integrated checks.
