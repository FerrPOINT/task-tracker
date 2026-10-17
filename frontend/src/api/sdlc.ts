import { api } from './client'
import type { components } from './generated'

export type SdlcContext = components['schemas']['SdlcContext']
export type RequirementsRevision = components['schemas']['RequirementsRevision']
export type AnalysisIntent = components['schemas']['AnalysisIntent']
export type Confirmation = components['schemas']['Confirmation']
export type MetadataEvent = components['schemas']['MetadataEvent']
export type MetadataPage = components['schemas']['MetadataPage']
export type ConfirmCommand = components['schemas']['ConfirmCommand']
export type RoleRoute = components['schemas']['RoleRoute']
export type RoleRoutes = components['schemas']['RoleRoutes']
export type LegacyRoleRoute = RoleRoute & {
  package_commit: string
  package_manifest_sha256: string
  instruction_profile?: never
  instruction_manifest_sha256?: never
}
export type SingleTaskRoleRoute = RoleRoute & {
  instruction_profile: 'single-task-v1'
  instruction_manifest_sha256: string
  package_commit?: never
  package_manifest_sha256?: never
}
export type LegacyRoutingPolicy = Omit<
  components['schemas']['RoutingPolicy'],
  'contract_version' | 'routes'
> & {
  contract_version: 1
  routes: { [Role in keyof RoleRoutes]: LegacyRoleRoute }
}
export type SingleTaskRoutingPolicy = Omit<
  components['schemas']['RoutingPolicy'],
  'contract_version' | 'routes'
> & {
  contract_version: 2
  routes: { [Role in keyof RoleRoutes]: SingleTaskRoleRoute }
}
export type RoutingPolicy = LegacyRoutingPolicy | SingleTaskRoutingPolicy

export const routingRoles = {
  project_manager: 'project_manager',
  analyst: 'analyst',
  architect: 'architect',
  developer: 'developer',
  reviewer: 'reviewer',
  tester: 'tester',
  devops: 'devops',
} as const satisfies Record<keyof RoleRoutes, keyof RoleRoutes>

export class SdlcRequestError extends Error {
  constructor(public readonly status: number) {
    super('SDLC request failed')
  }
}

export type SdlcSnapshot = {
  context: SdlcContext
  requirements: RequirementsRevision | null
  intent: AnalysisIntent | null
}

function isRoutingPolicy(
  data: components['schemas']['RoutingPolicy'],
  context: SdlcContext,
): data is RoutingPolicy {
  const hash = (value: unknown) => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value)
  const positive = (value: number) => Number.isSafeInteger(value) && value > 0
  const numericId = (value: string) =>
    typeof value === 'string' &&
    /^[1-9][0-9]{0,18}$/.test(value) &&
    BigInt(value) <= 9223372036854775807n
  const uuid = (value: string) =>
    typeof value === 'string' &&
    /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(value) &&
    value !== '00000000-0000-0000-0000-000000000000'
  const identityFields = [
    'agent_id',
    'namespace_id',
    'workflow_id',
  ] as const satisfies readonly (keyof RoleRoute)[]
  const singleTask = data.contract_version === 2
  const routeFields = [
    'agent_id',
    'fleet_config_revision',
    'namespace_id',
    'namespace_name',
    'workflow_id',
    'workflow_key',
    'profile',
    'workflow_catalog_version',
    'workflow_catalog_sha256',
    ...(singleTask
      ? ['instruction_profile', 'instruction_manifest_sha256']
      : ['package_commit', 'package_manifest_sha256']),
  ]
  if (
    (data.contract_version !== 1 && data.contract_version !== 2) ||
    data.project_id !== context.project_id ||
    data.tracker_instance_id !== context.tracker_instance_id ||
    !positive(data.version) ||
    !hash(data.routing_hash) ||
    data.verification !== 'declared' ||
    data.native_ready !== false ||
    data.dispatch_allowed !== false ||
    !data.routes ||
    !data.routes.project_manager ||
    Object.keys(data.routes).length !== Object.keys(routingRoles).length ||
    Object.values(routingRoles).some((role) => {
      const route = data.routes[role]
      const suffix = role.replaceAll('_', '-')
      const profile = singleTask
        ? `single-task-v1-${suffix}`
        : role === 'tester'
          ? 'hermes-sdlc-quality'
          : role === 'devops'
            ? 'hermes-sdlc-operations'
            : `hermes-sdlc-${suffix}`
      return (
        !route ||
        Object.keys(route).length !== routeFields.length ||
        Object.keys(route).some((field) => !routeFields.includes(field)) ||
        !uuid(route.agent_id) ||
        !positive(route.fleet_config_revision) ||
        (singleTask
          ? route.instruction_profile !== 'single-task-v1' ||
            !hash(route.instruction_manifest_sha256) ||
            route.instruction_manifest_sha256 !==
              data.routes.project_manager.instruction_manifest_sha256
          : route.package_commit !== '4b9b4c9297a13fb28a6ba2039af2f7cb719f2f58' ||
            !hash(route.package_manifest_sha256) ||
            route.package_manifest_sha256 !==
              data.routes.project_manager.package_manifest_sha256) ||
        !numericId(route.namespace_id) ||
        !numericId(route.workflow_id) ||
        route.namespace_name !== `${singleTask ? 'single-task' : 'hermes'}-${suffix}` ||
        route.workflow_key !== `${singleTask ? 'single-task-v1' : 'hermes-sdlc'}:${role}` ||
        route.profile !== profile ||
        route.workflow_catalog_version !== (singleTask ? 1 : 3) ||
        !hash(route.workflow_catalog_sha256) ||
        route.workflow_catalog_sha256 !== data.routes.project_manager.workflow_catalog_sha256
      )
    }) ||
    identityFields.some(
      (field) =>
        new Set(Object.values(data.routes).map((route) => route[field])).size !==
        Object.keys(routingRoles).length,
    )
  )
    return false
  return true
}

export async function getSdlcRoutingPolicy(
  context: SdlcContext,
  signal?: AbortSignal,
): Promise<RoutingPolicy | null> {
  const { data, response } = await api.GET('/api/v1/projects/{project_id}/sdlc/routing-policy', {
    params: { path: { project_id: context.project_id } },
    signal,
  })
  if (response.status === 404) return null
  if (!data) throw new SdlcRequestError(response.status)
  if (!isRoutingPolicy(data, context)) throw new SdlcRequestError(409)
  return data
}

export async function getSdlcSnapshot(
  id: string,
  signal?: AbortSignal,
): Promise<SdlcSnapshot | null> {
  const contextResult = await api.GET('/api/v1/issues/{id}/sdlc/context', {
    params: { path: { id } },
    signal,
  })
  if (contextResult.response.status === 404) return null
  if (!contextResult.data) throw new SdlcRequestError(contextResult.response.status)
  const context = contextResult.data
  if (context.contract_version !== 1 || context.task_id !== id) throw new SdlcRequestError(409)

  let requirements: RequirementsRevision | null = null
  if (context.requirement_revision != null) {
    const result = await api.GET('/api/v1/issues/{id}/sdlc/requirements/{revision}', {
      params: { path: { id, revision: context.requirement_revision } },
      signal,
    })
    if (!result.data) throw new SdlcRequestError(result.response.status)
    requirements = result.data
    if (requirements.revision !== context.requirement_revision) throw new SdlcRequestError(409)
  }

  let intent: AnalysisIntent | null = null
  if (context.stage === 'Analysis') {
    const result = await api.GET('/api/v1/issues/{id}/sdlc/analysis-intent', {
      params: { path: { id } },
      signal,
    })
    if (result.response.status !== 404 && !result.data)
      throw new SdlcRequestError(result.response.status)
    intent = result.data ?? null
    if (
      intent &&
      (intent.contract_version !== 1 ||
        intent.task_id !== id ||
        intent.project_id !== context.project_id ||
        intent.root_task_id !== context.root_task_id ||
        intent.tracker_instance_id !== context.tracker_instance_id ||
        intent.requirement_revision !== context.requirement_revision ||
        intent.content_hash !== requirements?.content_hash ||
        intent.stage !== 'Analysis' ||
        intent.status !== 'Ready' ||
        intent.role !== 'Analyst' ||
        (intent.workflow !== 'hermes-sdlc:analyst' &&
          intent.workflow !== 'single-task-v1:analyst') ||
        intent.mode !== 'analysis' ||
        intent.scope !== 'business' ||
        intent.cycle !== 0 ||
        intent.attempt !== 0 ||
        intent.operation_key !== `analysis:${intent.confirmation_id}`)
    )
      throw new SdlcRequestError(409)
    if (intent) {
      // The task's frozen publication, never today's project defaults, pins its workflow.
      const result = await api.GET('/api/v1/issues/{id}/sdlc/routing-snapshot', {
        params: { path: { id } },
        signal,
      })
      if (result.response.status === 404) {
        if (intent.workflow !== 'hermes-sdlc:analyst') throw new SdlcRequestError(409)
      } else {
        if (!result.data) throw new SdlcRequestError(result.response.status)
        const routing = result.data
        if (
          !routing.policy ||
          !isRoutingPolicy(routing.policy, context) ||
          routing.contract_version !== routing.policy.contract_version ||
          routing.task_id !== id ||
          routing.root_task_id !== context.root_task_id ||
          routing.project_id !== context.project_id ||
          routing.tracker_instance_id !== context.tracker_instance_id ||
          routing.confirmation_id !== intent.confirmation_id ||
          routing.requirement_revision !== intent.requirement_revision ||
          routing.content_hash !== intent.content_hash ||
          intent.workflow !== routing.policy.routes.analyst.workflow_key
        )
          throw new SdlcRequestError(409)
      }
    }
  }
  return { context, requirements, intent }
}

export async function confirmRequirements(id: string, revision: number, command: ConfirmCommand) {
  const { data, response } = await api.POST(
    '/api/v1/issues/{id}/sdlc/requirements/{revision}/confirm',
    {
      params: { path: { id, revision } },
      body: command,
    },
  )
  if (!data) throw new SdlcRequestError(response.status)
  if (
    data.task_id !== id ||
    data.revision !== revision ||
    data.content_hash !== command.content_hash ||
    data.stage !== 'Backlog'
  )
    throw new SdlcRequestError(409)
  return data
}

export async function getSdlcMetadata(
  id: string,
  after: string,
  signal?: AbortSignal,
): Promise<MetadataPage> {
  const { data, response } = await api.GET('/api/v1/issues/{id}/sdlc/events', {
    params: {
      path: { id },
      query: { projection: 'metadata_v1', after, limit: 100, max_bytes: 65536 },
    },
    signal,
  })
  if (!data) throw new SdlcRequestError(response.status)
  if (
    !('projection' in data) ||
    data.projection !== 'metadata_v1' ||
    data.contract_version !== 1 ||
    data.after !== after ||
    !/^(0|[1-9][0-9]*)$/.test(data.next_after) ||
    (data.has_more && BigInt(data.next_after) <= BigInt(after)) ||
    data.events.some((event) => event.task_id !== id)
  )
    throw new SdlcRequestError(409)
  return data
}
