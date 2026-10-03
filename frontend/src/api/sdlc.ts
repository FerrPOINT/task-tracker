import { api } from './client'
import type { components } from './generated'

export type SdlcContext = components['schemas']['SdlcContext']
export type RequirementsRevision = components['schemas']['RequirementsRevision']
export type AnalysisIntent = components['schemas']['AnalysisIntent']
export type Confirmation = components['schemas']['Confirmation']
export type MetadataEvent = components['schemas']['MetadataEvent']
export type MetadataPage = components['schemas']['MetadataPage']
export type ConfirmCommand = components['schemas']['ConfirmCommand']

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
        intent.workflow !== 'hermes-sdlc:analyst' ||
        intent.mode !== 'analysis' ||
        intent.scope !== 'business' ||
        intent.cycle !== 0 ||
        intent.attempt !== 0 ||
        intent.operation_key !== `analysis:${intent.confirmation_id}`)
    )
      throw new SdlcRequestError(409)
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
