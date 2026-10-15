import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  confirmRequirements,
  getSdlcMetadata,
  getSdlcRoutingPolicy,
  getSdlcSnapshot,
  SdlcRequestError,
} from './sdlc'
import { routingPolicy, singleTaskRoutingPolicy } from './test-fixtures/routing-policy'
import type { AnalysisIntent, SdlcContext, RequirementsRevision, RoutingPolicy } from './sdlc'
import type { components } from './generated'

const { GET, POST } = vi.hoisted(() => ({ GET: vi.fn(), POST: vi.fn() }))
vi.mock('./client', () => ({ api: { GET, POST } }))
const task = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'
const confirmation = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'
const hash = 'a'.repeat(64)
const context: SdlcContext = {
  contract_version: 1,
  tracker_instance_id: 'tracker-test',
  project_id: task,
  task_id: task,
  root_task_id: task,
  owner_subject: 'owner',
  stage: 'Analysis',
  requirement_revision: 2,
  waiting_reason: 'queued_for_analysis',
  permissions: { can_answer: false, can_confirm: false },
  assignment: null,
}
const requirements: RequirementsRevision = {
  revision: 2,
  content_hash: hash,
  author_subject: 'pm',
  created_at: '2026-10-03T10:00:00Z',
  goal: 'Exact requirements',
  scope: ['scope'],
  exclusions: [],
  scenarios: ['scenario'],
  acceptance_criteria: ['criterion'],
  constraints: [],
  dependencies: [],
  assumptions: [],
  checklist: ['check'],
  prerequisites: ['prerequisite'],
}
const intent: AnalysisIntent = {
  contract_version: 1,
  tracker_instance_id: context.tracker_instance_id,
  project_id: task,
  task_id: task,
  root_task_id: task,
  intent_id: task,
  confirmation_id: confirmation,
  requirement_revision: 2,
  content_hash: hash,
  stage: 'Analysis',
  status: 'Ready',
  role: 'Analyst',
  workflow: 'hermes-sdlc:analyst',
  mode: 'analysis',
  scope: 'business',
  cycle: 0,
  attempt: 0,
  operation_key: `analysis:${confirmation}`,
  created_at: '2026-10-03T10:00:00Z',
}
const success = (data: unknown) => ({ data, response: { status: 200 } })
const frozenRouting = (policy: RoutingPolicy): components['schemas']['TaskRoutingSnapshot'] => ({
  contract_version: policy.contract_version,
  snapshot_id: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
  tracker_instance_id: context.tracker_instance_id,
  project_id: task,
  task_id: task,
  root_task_id: task,
  confirmation_id: confirmation,
  requirement_revision: requirements.revision,
  content_hash: hash,
  policy,
  created_at: '2026-10-03T10:00:00Z',
})

describe('SDLC generated API adapter', () => {
  beforeEach(() => {
    GET.mockReset()
    GET.mockResolvedValue({ response: { status: 404 } })
    POST.mockReset()
  })
  it('reads the exact revision and the separate queued intent without admitting execution', async () => {
    GET.mockResolvedValueOnce(success(context))
      .mockResolvedValueOnce(success(requirements))
      .mockResolvedValueOnce(success(intent))
    await expect(getSdlcSnapshot(task)).resolves.toEqual({ context, requirements, intent })
    expect(GET.mock.calls.map((call) => call[0])).toEqual([
      '/api/v1/issues/{id}/sdlc/context',
      '/api/v1/issues/{id}/sdlc/requirements/{revision}',
      '/api/v1/issues/{id}/sdlc/analysis-intent',
      '/api/v1/issues/{id}/sdlc/routing-snapshot',
    ])
    expect(GET.mock.calls[1]?.[1].params.path).toEqual({ id: task, revision: 2 })
    expect(POST).not.toHaveBeenCalled()
  })
  it.each([401, 403, 409, 503])('keeps status %i distinct from empty context', async (status) => {
    GET.mockResolvedValue({ response: { status } })
    await expect(getSdlcSnapshot(task)).rejects.toMatchObject({ status })
  })
  it('returns empty only for missing context and does not invent a requirement', async () => {
    GET.mockResolvedValue({ response: { status: 404 } })
    await expect(getSdlcSnapshot(task)).resolves.toBeNull()
    GET.mockResolvedValue(success({ ...context, stage: 'Draft', requirement_revision: null }))
    await expect(getSdlcSnapshot(task)).resolves.toMatchObject({ requirements: null, intent: null })
  })
  it.each([
    'content_hash',
    'requirement_revision',
    'project_id',
    'root_task_id',
    'workflow',
    'attempt',
  ])('rejects a conflicting intent %s instead of claiming Ready', async (field) => {
    GET.mockResolvedValueOnce(success(context))
      .mockResolvedValueOnce(success(requirements))
      .mockResolvedValueOnce(success({ ...intent, [field]: field === 'attempt' ? 1 : 'mismatch' }))
    await expect(getSdlcSnapshot(task)).rejects.toEqual(new SdlcRequestError(409))
  })
  it('keeps missing intent distinct from a verified queued intent', async () => {
    GET.mockResolvedValueOnce(success(context))
      .mockResolvedValueOnce(success(requirements))
      .mockResolvedValueOnce({ response: { status: 404 } })
    await expect(getSdlcSnapshot(task)).resolves.toMatchObject({ intent: null })
    expect(GET).toHaveBeenCalledTimes(3)
  })
  it.each([
    { family: 'v1', makePolicy: routingPolicy },
    { family: 'v2', makePolicy: singleTaskRoutingPolicy },
  ])(
    'reads an Analysis intent against its actual frozen $family routing, not current project defaults',
    async ({ makePolicy }) => {
      const policy = makePolicy(task)
      const routedIntent = { ...intent, workflow: policy.routes.analyst.workflow_key }
      const signal = new AbortController().signal
      GET.mockResolvedValueOnce(success(context))
        .mockResolvedValueOnce(success(requirements))
        .mockResolvedValueOnce(success(routedIntent))
        .mockResolvedValueOnce(success(frozenRouting(policy)))
      await expect(getSdlcSnapshot(task, signal)).resolves.toEqual({
        context,
        requirements,
        intent: routedIntent,
      })
      expect(GET.mock.lastCall).toEqual([
        '/api/v1/issues/{id}/sdlc/routing-snapshot',
        {
          params: { path: { id: task } },
          signal,
        },
      ])
      expect(GET.mock.calls.some(([path]) => path.includes('/projects/'))).toBe(false)
      expect(POST).not.toHaveBeenCalled()
    },
  )
  it.each([401, 403, 409, 503])(
    'does not fall back to legacy intent on frozen routing HTTP %i',
    async (status) => {
      GET.mockResolvedValueOnce(success(context))
        .mockResolvedValueOnce(success(requirements))
        .mockResolvedValueOnce(success(intent))
        .mockResolvedValueOnce({ response: { status } })
      await expect(getSdlcSnapshot(task)).rejects.toMatchObject({ status })
    },
  )
  it('rejects a profile intent without a frozen snapshot', async () => {
    GET.mockResolvedValueOnce(success(context))
      .mockResolvedValueOnce(success(requirements))
      .mockResolvedValueOnce(success({ ...intent, workflow: 'single-task-v1:analyst' }))
    await expect(getSdlcSnapshot(task)).rejects.toMatchObject({ status: 409 })
  })
  it.each([
    ['contract_version', 1],
    ['task_id', confirmation],
    ['root_task_id', confirmation],
    ['project_id', confirmation],
    ['tracker_instance_id', 'foreign'],
    ['confirmation_id', task],
    ['requirement_revision', 3],
    ['content_hash', 'b'.repeat(64)],
    ['policy', routingPolicy(task)],
  ])('rejects frozen routing mismatch %s without claiming Ready', async (field, value) => {
    const policy = singleTaskRoutingPolicy(task)
    GET.mockResolvedValueOnce(success(context))
      .mockResolvedValueOnce(success(requirements))
      .mockResolvedValueOnce(success({ ...intent, workflow: 'single-task-v1:analyst' }))
      .mockResolvedValueOnce(success({ ...frozenRouting(policy), [field]: value }))
    await expect(getSdlcSnapshot(task)).rejects.toMatchObject({ status: 409 })
    expect(POST).not.toHaveBeenCalled()
  })
  it('rejects a legacy workflow mislabeled with a single-task snapshot', async () => {
    GET.mockResolvedValueOnce(success(context))
      .mockResolvedValueOnce(success(requirements))
      .mockResolvedValueOnce(success(intent))
      .mockResolvedValueOnce(success(frozenRouting(singleTaskRoutingPolicy(task))))
    await expect(getSdlcSnapshot(task)).rejects.toMatchObject({ status: 409 })
  })
  it('posts only the exact confirmation pin/key and keeps its Backlog publication receipt', async () => {
    const command = { content_hash: hash, idempotency_key: 'exact-confirmation-operation' }
    const receipt = {
      id: confirmation,
      task_id: task,
      revision: 2,
      content_hash: hash,
      owner_subject: 'owner',
      created_at: '2026-10-03T10:00:00Z',
      stage: 'Backlog',
    }
    POST.mockResolvedValue(success(receipt))
    await expect(confirmRequirements(task, 2, command)).resolves.toEqual(receipt)
    expect(POST).toHaveBeenCalledWith('/api/v1/issues/{id}/sdlc/requirements/{revision}/confirm', {
      params: { path: { id: task, revision: 2 } },
      body: command,
    })
  })
  it('reads the existing project policy with generated path parameters and abort signal', async () => {
    const policy = routingPolicy(task)
    const signal = new AbortController().signal
    GET.mockResolvedValue(success(policy))
    const original = JSON.stringify(policy)
    await expect(getSdlcRoutingPolicy(context, signal)).resolves.toBe(policy)
    expect(JSON.stringify(policy)).toBe(original)
    expect(GET).toHaveBeenCalledWith('/api/v1/projects/{project_id}/sdlc/routing-policy', {
      params: { path: { project_id: task } },
      signal,
    })
  })
  it('accepts v2 with omitted package fields without fabricating legacy pins', async () => {
    const policy = singleTaskRoutingPolicy(task)
    const original = JSON.stringify(policy)
    GET.mockResolvedValue(success(policy))
    await expect(getSdlcRoutingPolicy(context)).resolves.toBe(policy)
    expect(JSON.stringify(policy)).toBe(original)
    for (const route of Object.values(policy.routes)) {
      expect(route).not.toHaveProperty('package_commit')
      expect(route).not.toHaveProperty('package_manifest_sha256')
    }
  })
  it.each([
    ['agent_id', '00000000-0000-4000-8000-000000000001'],
    ['fleet_config_revision', 0],
    ['fleet_config_revision', 9007199254740992],
    ['namespace_id', '02'],
    ['workflow_id', '9223372036854775808'],
    ['package_commit', '4b9b4c9297a13fb28a6ba2039af2f7cb719f2f58'],
    ['package_commit', null],
    ['package_manifest_sha256', 'c'.repeat(64)],
    ['package_manifest_sha256', null],
    ['instruction_profile', null],
    ['instruction_profile', 'single-task-v2'],
    ['instruction_manifest_sha256', null],
    ['instruction_manifest_sha256', 'F'.repeat(64)],
    ['instruction_manifest_sha256', 'b'.repeat(64)],
    ['namespace_name', 'hermes-analyst'],
    ['workflow_key', 'hermes-sdlc:analyst'],
    ['profile', 'hermes-sdlc-analyst'],
    ['workflow_catalog_version', 3],
    ['workflow_catalog_sha256', 'b'.repeat(64)],
    ['runtime_ready', true],
  ])('rejects malformed/mixed profile route %s=%s', async (field, value) => {
    const policy = singleTaskRoutingPolicy(task)
    GET.mockResolvedValue(
      success({
        ...policy,
        routes: {
          ...policy.routes,
          analyst: { ...policy.routes.analyst, [field]: value },
        },
      }),
    )
    await expect(getSdlcRoutingPolicy(context)).rejects.toMatchObject({ status: 409 })
  })
  it.each([
    { family: 'v1', makePolicy: routingPolicy },
    { family: 'v2', makePolicy: singleTaskRoutingPolicy },
  ])('requires both source identity fields for $family', async ({ makePolicy }) => {
    const policy = makePolicy(task)
    const fields =
      policy.contract_version === 1
        ? ['package_commit', 'package_manifest_sha256']
        : ['instruction_profile', 'instruction_manifest_sha256']
    for (const field of fields) {
      const route = Object.fromEntries(
        Object.entries(policy.routes.analyst).filter(([key]) => key !== field),
      )
      GET.mockResolvedValue(success({ ...policy, routes: { ...policy.routes, analyst: route } }))
      await expect(getSdlcRoutingPolicy(context)).rejects.toMatchObject({ status: 409 })
    }
  })
  it.each(['instruction_profile', 'instruction_manifest_sha256'])(
    'rejects instruction fields even null on a legacy route: %s',
    async (field) => {
      const policy = routingPolicy(task)
      for (const value of [null, 'single-task-v1']) {
        GET.mockResolvedValue(
          success({
            ...policy,
            routes: {
              ...policy.routes,
              analyst: { ...policy.routes.analyst, [field]: value },
            },
          }),
        )
        await expect(getSdlcRoutingPolicy(context)).rejects.toMatchObject({ status: 409 })
      }
    },
  )
  it('rejects a legacy role in a v2 policy and a profile role in a v1 policy', async () => {
    const legacy = routingPolicy(task)
    const single = singleTaskRoutingPolicy(task)
    for (const policy of [
      { ...single, routes: { ...single.routes, developer: legacy.routes.developer } },
      { ...legacy, routes: { ...legacy.routes, developer: single.routes.developer } },
    ]) {
      GET.mockResolvedValue(success(policy))
      await expect(getSdlcRoutingPolicy(context)).rejects.toMatchObject({ status: 409 })
    }
  })
  it.each([401, 403, 409, 503])('does not treat policy HTTP %i as absent', async (status) => {
    GET.mockResolvedValue({ response: { status } })
    await expect(getSdlcRoutingPolicy(context)).rejects.toMatchObject({ status })
  })
  it('keeps missing policy distinct from invalid or admitted policy', async () => {
    GET.mockResolvedValue({ response: { status: 404 } })
    await expect(getSdlcRoutingPolicy(context)).resolves.toBeNull()
  })
  it.each([
    ['contract_version', 2],
    ['version', 0],
    ['version', 9007199254740992],
    ['project_id', confirmation],
    ['tracker_instance_id', 'foreign'],
    ['native_ready', true],
    ['dispatch_allowed', true],
    ['verification', 'verified'],
  ])('rejects malformed policy %s=%s', async (field, value) => {
    GET.mockResolvedValue(success({ ...routingPolicy(task), [field]: value }))
    await expect(getSdlcRoutingPolicy(context)).rejects.toMatchObject({ status: 409 })
  })
  it.each([
    ['agent_id', '00000000-0000-4000-8000-000000000001'],
    ['namespace_id', '1'],
    ['workflow_id', '11'],
    ['namespace_id', '02'],
    ['workflow_id', '9223372036854775808'],
    ['fleet_config_revision', 0],
    ['package_commit', 'HEAD'],
    ['workflow_catalog_version', 2],
    ['package_manifest_sha256', 'f'.repeat(64)],
    ['workflow_catalog_sha256', 'f'.repeat(64)],
    ['namespace_name', 'hermes-architect'],
    ['workflow_key', 'hermes-sdlc:architect'],
    ['profile', 'hermes-sdlc-quality'],
  ])('rejects malformed Analyst reference %s', async (field, value) => {
    const policy = routingPolicy(task)
    policy.routes.analyst = { ...policy.routes.analyst, [field]: value }
    GET.mockResolvedValue(success(policy))
    await expect(getSdlcRoutingPolicy(context)).rejects.toMatchObject({ status: 409 })
  })
  it('rejects missing roles without using incomplete preparation', async () => {
    const policy = routingPolicy(task)
    GET.mockResolvedValue(success({ ...policy, routes: { analyst: policy.routes.analyst } }))
    await expect(getSdlcRoutingPolicy(context)).rejects.toMatchObject({ status: 409 })
  })
  it('uses typed metadata query parameters and rejects a legacy projection', async () => {
    const page = {
      contract_version: 1,
      projection: 'metadata_v1',
      after: '0',
      next_after: '0',
      has_more: false,
      events: [],
    }
    GET.mockResolvedValue(success(page))
    await expect(getSdlcMetadata(task, '0')).resolves.toEqual(page)
    expect(GET.mock.lastCall?.[1].params).toEqual({
      path: { id: task },
      query: { projection: 'metadata_v1', after: '0', limit: 100, max_bytes: 65536 },
    })
    GET.mockResolvedValue(success({ events: [] }))
    await expect(getSdlcMetadata(task, '0')).rejects.toMatchObject({ status: 409 })
    GET.mockResolvedValue(success({ ...page, has_more: true }))
    await expect(getSdlcMetadata(task, '0')).rejects.toMatchObject({ status: 409 })
  })
})
