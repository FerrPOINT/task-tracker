import type {
  LegacyRoutingPolicy,
  RoleRoutes,
  SingleTaskRoleRoute,
  SingleTaskRoutingPolicy,
} from '../sdlc'

function roleRoutes<Route>(make: (role: keyof RoleRoutes, index: number) => Route) {
  return {
    project_manager: make('project_manager', 0),
    analyst: make('analyst', 1),
    architect: make('architect', 2),
    developer: make('developer', 3),
    reviewer: make('reviewer', 4),
    tester: make('tester', 5),
    devops: make('devops', 6),
  }
}

// Isolated test response only; no production policy or native readiness fixture.
export function routingPolicy(projectId: string, instance = 'tracker-test'): LegacyRoutingPolicy {
  const routes = roleRoutes((role, index) => {
    const suffix = role.replaceAll('_', '-')
    return {
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
  })
  return {
    contract_version: 1,
    tracker_instance_id: instance,
    project_id: projectId,
    version: 5,
    routing_hash: 'e'.repeat(64),
    routes,
    verification: 'declared',
    native_ready: false,
    dispatch_allowed: false,
    author_subject: 'project-owner',
    created_at: '2026-10-03T10:00:00Z',
  }
}

export function singleTaskRoutingPolicy(
  projectId: string,
  instance = 'tracker-test',
): SingleTaskRoutingPolicy {
  const legacy = routingPolicy(projectId, instance)
  return {
    ...legacy,
    contract_version: 2,
    routes: roleRoutes<SingleTaskRoleRoute>((role) => {
      const route = legacy.routes[role]
      const suffix = role.replaceAll('_', '-')
      return {
        agent_id: route.agent_id,
        fleet_config_revision: route.fleet_config_revision,
        namespace_id: route.namespace_id,
        namespace_name: `single-task-${suffix}`,
        workflow_id: route.workflow_id,
        workflow_key: `single-task-v1:${role}`,
        profile: `single-task-v1-${suffix}`,
        workflow_catalog_version: 1,
        workflow_catalog_sha256: route.workflow_catalog_sha256,
        instruction_profile: 'single-task-v1',
        instruction_manifest_sha256: 'f'.repeat(64),
      }
    }),
  }
}
