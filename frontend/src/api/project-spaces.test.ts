import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  createProjectInSpaces,
  getProjectSpaces,
  replaceProjectSpaces,
  projectSpaceKeys,
  getProjectSpaceManagement,
  getProjectSpaceProjection,
  getProjectSpaceOperationReceipt,
  reconcileProjectSpaceProjection,
  type ConfirmedProjectSpaces,
} from './project-spaces'
const methods = vi.hoisted(() => ({ GET: vi.fn(), PUT: vi.fn(), POST: vi.fn() }))
vi.mock('./client', () => ({ api: methods }))
const registry = '00000000-0000-4000-8000-000000000001'
const a = { registry_instance_id: registry, space_id: '00000000-0000-4000-8000-000000000002' }
const hidden = { ...a, space_id: '00000000-0000-4000-8000-000000000003' }
const project = { key: 'QA', id: '00000000-0000-4000-8000-000000000004' }
const operation = '00000000-0000-4000-8000-000000000005'
function links(): ConfirmedProjectSpaces {
  const now = Math.floor(Date.now() / 1000)
  return {
    project: { tracker_instance_id: registry, project_id: project.id },
    revision: 5,
    spaces: [a, hidden],
    validity: { observed_at: now, valid_until: now + 20 },
  }
}
const command = { operation_id: operation, expected_revision: 4, spaces: [a, hidden] }
beforeEach(() => Object.values(methods).forEach((fn) => fn.mockReset()))
describe('canonical project-space client', () => {
  it('reads the complete association set and forwards cancellation without metadata fetching', async () => {
    const value = links()
    methods.GET.mockResolvedValue({ data: value, response: new Response() })
    const signal = new AbortController().signal
    expect(await getProjectSpaces(project, signal)).toEqual(value)
    expect(methods.GET).toHaveBeenCalledWith('/api/v2/projects/{project_key}/spaces', {
      params: { path: { project_key: 'QA' } },
      signal,
    })
    expect(methods.GET).toHaveBeenCalledTimes(1)
  })
  it('sends original operation/revision and retains hidden links', async () => {
    methods.PUT.mockResolvedValue({ data: links(), response: new Response() })
    await replaceProjectSpaces(project, command)
    expect(methods.PUT).toHaveBeenCalledWith('/api/v2/projects/{project_key}/spaces', {
      params: { path: { project_key: 'QA' } },
      body: command,
    })
  })
  it('returns a newer authoritative set on original-key replay rather than synthesizing the requested state', async () => {
    const value = links()
    value.revision = 7
    value.spaces = [hidden]
    methods.PUT.mockResolvedValue({ data: value, response: new Response() })
    expect(await replaceProjectSpaces(project, command)).toEqual(value)
  })
  it.each([{ spaces: [] }, { spaces: [a, a] }])(
    'rejects empty or duplicate set before sending',
    async ({ spaces }) => {
      await expect(replaceProjectSpaces(project, { ...command, spaces })).rejects.toThrow()
      expect(methods.PUT).not.toHaveBeenCalled()
    },
  )
  it('rejects an invalid operation, revision and injected primary space', async () => {
    await expect(
      replaceProjectSpaces(project, { ...command, operation_id: 'bad' }),
    ).rejects.toThrow()
    await expect(
      replaceProjectSpaces(project, { ...command, expected_revision: 0 }),
    ).rejects.toThrow()
    await expect(
      replaceProjectSpaces(project, { ...command, primary_space: a } as never),
    ).rejects.toThrow()
    expect(methods.PUT).not.toHaveBeenCalled()
  })
  it.each([403, 409, 503])(
    'does not retry uncertain or rejected mutation %s with a new key',
    async (status) => {
      methods.PUT.mockResolvedValue({ error: {}, response: new Response(null, { status }) })
      await expect(replaceProjectSpaces(project, command)).rejects.toMatchObject({ status })
      expect(methods.PUT).toHaveBeenCalledTimes(1)
    },
  )
  it.each([
    (v: ConfirmedProjectSpaces) => {
      v.project.project_id = a.space_id
    },
    (v: ConfirmedProjectSpaces) => {
      v.spaces = []
    },
    (v: ConfirmedProjectSpaces) => {
      v.validity.valid_until = Math.floor(Date.now() / 1000)
    },
    (v: ConfirmedProjectSpaces) => {
      v.validity.valid_until = v.validity.observed_at + 61
    },
  ])('rejects wrong project, empty associations and invalid window %#', async (mutate) => {
    const value = links()
    mutate(value)
    methods.GET.mockResolvedValue({ data: value, response: new Response() })
    await expect(getProjectSpaces(project)).rejects.toMatchObject({
      status: 502,
      kind: 'invalid_response',
    })
  })
  it('creates only through v2 with a nonempty explicit set', async () => {
    const input = { key: 'QA', name: 'Проект', spaces: [a], description: null }
    methods.POST.mockResolvedValue({
      data: { ...project, name: 'Проект' },
      response: new Response(null, { status: 201 }),
    })
    expect(await createProjectInSpaces(input)).toMatchObject(project)
    expect(methods.POST).toHaveBeenCalledWith('/api/v2/projects', { body: input })
    await expect(createProjectInSpaces({ ...input, spaces: [] })).rejects.toThrow()
    expect(methods.POST).toHaveBeenCalledTimes(1)
  })
  it('separates link caches by central actor and immutable project ID', () => {
    const identity = { issuer: 'https://auth.example.test', subject: 'actor' }
    expect(projectSpaceKeys.project(identity, project)).not.toEqual(
      projectSpaceKeys.project({ ...identity, subject: 'other' }, project),
    )
    expect(projectSpaceKeys.project(identity, project)).not.toEqual(
      projectSpaceKeys.project(identity, { ...project, id: a.space_id }),
    )
  })
})

describe('owner management hint and projection readback', () => {
  it.each([true, false])(
    'requires the exact server hint %s instead of inferring ownership',
    async (canChange) => {
      methods.GET.mockResolvedValue({
        data: links(),
        response: new Response(null, {
          headers: {
            'x-pdlc-project-spaces-can-change': String(canChange),
            'x-pdlc-admission-policy-revision': '1',
          },
        }),
      })
      expect((await getProjectSpaceManagement(project)).canChange).toBe(canChange)
    },
  )
  it.each([null, '1', 'admin'])('rejects missing or malformed management hint %s', async (hint) => {
    methods.GET.mockResolvedValue({
      data: links(),
      response: new Response(null, {
        headers: hint
          ? { 'x-pdlc-project-spaces-can-change': hint, 'x-pdlc-admission-policy-revision': '1' }
          : { 'x-pdlc-admission-policy-revision': '1' },
      }),
    })
    await expect(getProjectSpaceManagement(project)).rejects.toMatchObject({
      status: 502,
      kind: 'invalid_response',
    })
  })
  it('requires project identity and explicit owner confirmation in projection readback', async () => {
    const status = {
      operation_id: operation,
      project: links().project,
      revision: 5,
      confirmed: false,
      last_error: null,
    }
    methods.GET.mockResolvedValue({ data: status, response: new Response() })
    expect(await getProjectSpaceProjection(project)).toEqual(status)
    methods.POST.mockResolvedValue({
      data: { ...status, confirmed: true },
      response: new Response(),
    })
    expect((await reconcileProjectSpaceProjection(project)).confirmed).toBe(true)
    expect(methods.POST).toHaveBeenCalledWith(
      '/api/v2/projects/{project_key}/spaces/projection/reconcile',
      { params: { path: { project_key: project.key } } },
    )
    methods.GET.mockResolvedValue({
      data: { ...status, project: { ...status.project, project_id: a.space_id } },
      response: new Response(),
    })
    await expect(getProjectSpaceProjection(project)).rejects.toMatchObject({ status: 502 })
  })
})

describe('authored canonical receipt read', () => {
  function receipt() {
    return {
      schema_version: 1,
      project: links().project,
      operation_id: operation,
      applied: true,
      applied_revision: 5,
      change: { ...command, spaces: [...command.spaces] },
      validity: links().validity,
    }
  }
  it('uses a read-only fixed route and confirms the exact original command without PUT', async () => {
    methods.GET.mockResolvedValue({ data: receipt(), response: new Response() })
    const signal = new AbortController().signal
    const result = await getProjectSpaceOperationReceipt(project, links().project, command, signal)
    expect(result.applied).toBe(true)
    expect(methods.GET).toHaveBeenCalledWith(
      '/api/v2/projects/{project_key}/spaces/operations/{operation_id}',
      {
        params: { path: { project_key: 'QA', operation_id: operation } },
        signal,
      },
    )
    expect(methods.PUT).not.toHaveBeenCalled()
  })
  it.each([
    (value: ReturnType<typeof receipt>) => {
      value.applied = false
    },
    (value: ReturnType<typeof receipt>) => {
      value.change.expected_revision = 9
    },
    (value: ReturnType<typeof receipt>) => {
      value.project.tracker_instance_id = a.space_id
    },
    (value: ReturnType<typeof receipt>) => {
      value.change.spaces.reverse()
    },
    (value: ReturnType<typeof receipt>) => {
      value.validity.valid_until = Math.floor(Date.now() / 1000)
    },
    (value: ReturnType<typeof receipt>) => {
      value.operation_id = a.space_id
    },
  ])('rejects an unrelated, expired or incomplete receipt %#', async (mutate) => {
    const value = receipt()
    mutate(value)
    methods.GET.mockResolvedValue({ data: value, response: new Response() })
    await expect(
      getProjectSpaceOperationReceipt(project, links().project, command),
    ).rejects.toMatchObject({ status: 502, kind: 'invalid_response' })
  })
  it.each([404, 403, 503])(
    'keeps %s unresolved without guessing a new operation key',
    async (status) => {
      methods.GET.mockResolvedValue({ error: {}, response: new Response(null, { status }) })
      await expect(
        getProjectSpaceOperationReceipt(project, links().project, command),
      ).rejects.toMatchObject({ status })
      expect(methods.GET).toHaveBeenCalledTimes(1)
      expect(methods.PUT).not.toHaveBeenCalled()
    },
  )
})

it.each([null, '', '1.5', '-1', '9007199254740992'])(
  'rejects an absent or malformed policy revision header %s',
  async (value) => {
    const headers: Record<string, string> = { 'x-pdlc-project-spaces-can-change': 'true' }
    if (value !== null) headers['x-pdlc-admission-policy-revision'] = value
    methods.GET.mockResolvedValue({ data: links(), response: new Response(null, { headers }) })
    await expect(getProjectSpaceManagement(project)).rejects.toMatchObject({
      status: 502,
      kind: 'invalid_response',
    })
  },
)
