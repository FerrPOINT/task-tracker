# Fresh Analysis Configuration Preflight

Tracker reads Fleet's existing protected configuration owner endpoint and checks
it against the exact immutable prepared Analysis assignment. This is the first
actual consumer of the mapping in `SDLC_ROUTING_V1.md`, not execution admission.
No shared DB, scheduler, migration, capacity mutation, dispatch or model call is
added. Existing reservation APIs and receipts remain unchanged.

`GET /api/v1/issues/{id}/sdlc/analysis-reservation/configuration-preflight`
requires the same fresh Central human/project ACL or exact machine
`task-tracker:read` as reservation readback. PM grants and write-only scheduler
credentials are rejected. After the remote request Tracker authorizes again and
reads the current immutable assignment and lease: expiry, heartbeat/CAS change or
revoked ACL rejects the observation. No transaction stays open during owner HTTP.

Operator configuration uses `TASKTRACKER_SDLC__FLEET_URL` (origin only) and
`TASKTRACKER_SDLC__FLEET_READ_TOKEN` (a separate Base PAT). Fleet registers the
reader subject and concrete agent allowlist using its existing configuration
reader policy. The incoming caller credential is never forwarded to Fleet.
Missing or invalid operator config returns 503 for preflight; it does not enable
local auth or prevent unrelated Tracker startup. Secrets must remain outside Git.

The request uses Fleet's real `/internal/runtime/v1/agents/{id}/configuration`:
no redirect, proxy, HTTP retry or cache; 3-second connect / 10-second total budget
and 64-KiB response bound. Unknown DTO fields and versions are rejected. Observation
age is at most 5 seconds with 2 seconds future clock-skew allowance, checked again
after repository authorization and lease readback. Agent UUID,
effective config revision, role (Fleet `dev_ops` maps only to `devops`), package
commit/manifest, namespace/profile, Workflow IDs/key/catalog version/hash and
skills revision must match the frozen route. Package modes must include the
assignment mode. `managed_files_verified` must be true. The current v1 contract
has both runtime-ready fields false and nonempty blockers; contrary claims are
rejected rather than interpreted as a new native capability.

200 returns `configuration_matched:true`, exact assignment ID/hash, lease
version/fence, concrete agent/config revision and Fleet observation ref/time.
`runtime_ready:false` and `dispatch_allowed:false` are enforced in implementation
and OpenAPI. The response is not a durable receipt or frozen Fleet run/config ACK;
the remote config may change after the observation. Native execution admission,
Workflow assignment acceptance and trusted execution stop remain mandatory gates.
409 denotes missing/inactive/changed reservation or mismatched/stale observation;
503 denotes unavailable/unregistered/malformed counterpart; 401/403 preserve
Central/project authorization. All successful responses use `Cache-Control:no-store`.

Tests use actual HTTP and disposable PostgreSQL but a synthetic Fleet owner and
Central issuer. They are not real configured Hermes, execution or full cross-app
acceptance. The production group freeze remains in force.

The disposable PostgreSQL HTTP gate additionally accepts `TT_SDLC_TEST_CURL=1`
to check this endpoint with curl using only the synthetic fixture reader token.
It requires curl on that QA runner and can save its response alongside the other
receipts using `TT_SDLC_TEST_EVIDENCE_DIR`. Neither variable enables product code.
