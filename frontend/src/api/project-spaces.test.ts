import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  createProjectInSpaces,
  getProjectSpaces,
  replaceProjectSpaces,
  projectSpaceKeys,
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
describe('canonical project-space client', () => {
  beforeEach(() => Object.values(methods).forEach((fn) => fn.mockReset()))
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
