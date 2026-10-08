import type { RoutingPolicy, RoleRoute } from '../sdlc'
import { routingRoles } from '../sdlc'

// Isolated test response only; no production policy or native readiness fixture.
export function routingPolicy(projectId: string, instance = 'tracker-test'): RoutingPolicy {
  const entries = Object.values(routingRoles).map((role, index) => {
    const suffix = role.replaceAll('_', '-')
    const route: RoleRoute = {
      agent_id: `00000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
      fleet_config_revision: index + 1,
      package_commit: '4b9b4c9297a13fb28a6ba2039af2f7cb719f2f58',
      package_manifest_sha256: 'c'.repeat(64),
      namespace_id: String(index + 1),
      namespace_name: `hermes-${suffix}`,
      workflow_id: String(index + 11),
      workflow_key: `hermes-sdlc:${role}`,
      profile:
        role === 'tester'
          ? 'hermes-sdlc-quality'
          : role === 'devops'
            ? 'hermes-sdlc-operations'
            : `hermes-sdlc-${suffix}`,
      workflow_catalog_version: 3,
      workflow_catalog_sha256: 'd'.repeat(64),
    }
    return [role, route]
  })
  return {
    contract_version: 1,
    tracker_instance_id: instance,
    project_id: projectId,
    version: 5,
    routing_hash: 'e'.repeat(64),
    routes: Object.fromEntries(entries) as RoutingPolicy['routes'],
    verification: 'declared',
    native_ready: false,
    dispatch_allowed: false,
    author_subject: 'project-owner',
    created_at: '2026-10-03T10:00:00Z',
  }
}
