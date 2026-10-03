import { beforeEach, describe, expect, it, vi } from 'vitest'
import { confirmRequirements, getSdlcMetadata, getSdlcSnapshot, SdlcRequestError } from './sdlc'
import type { AnalysisIntent, SdlcContext, RequirementsRevision } from './sdlc'

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

describe('SDLC generated API adapter', () => {
  beforeEach(() => {
    GET.mockReset()
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
