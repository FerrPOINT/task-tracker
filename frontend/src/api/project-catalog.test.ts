import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  getVisibleProjectCatalog,
  visibleProjectKeys,
  type VisibleProjectCatalog,
} from './project-catalog'
const get = vi.hoisted(() => vi.fn())
vi.mock('./client', () => ({ api: { GET: get } }))
const registry = '00000000-0000-4000-8000-000000000001'
const space = { registry_instance_id: registry, space_id: '00000000-0000-4000-8000-000000000002' }
const identity = { issuer: 'https://auth.example.test', subject: 'central-a' }
function page(): VisibleProjectCatalog {
  const now = Math.floor(Date.now() / 1000)
  return {
    schema_version: 2,
    tracker_instance_id: registry,
    identity,
    policy_revision: 3,
    validity: { observed_at: now, valid_until: now + 20 },
    space,
    total: 2,
    limit: 1,
    offset: 0,
    has_more: true,
    projects: [
      {
        id: '00000000-0000-4000-8000-000000000003',
        key: '-',
        name: 'Одинаковое имя',
        owner_id: '00000000-0000-4000-8000-000000000004',
        owner_name: 'Owner',
        description: null,
        todo_count: 0,
        in_progress_count: 0,
        done_count: 0,
      },
    ],
  }
}
beforeEach(() => get.mockReset())
describe('canonical visible project catalog', () => {
  it('sends an explicit pair, literal search, exact key and cancellation to V2 only', async () => {
    const data = { ...page(), total: 1, has_more: false }
    get.mockResolvedValue({ data, response: new Response() })
    const signal = new AbortController().signal
    expect(
      await getVisibleProjectCatalog(
        identity,
        { space, limit: 1, q: ' %_ ', projectKey: '-' },
        signal,
      ),
    ).toEqual(data)
    expect(get).toHaveBeenCalledWith('/api/v2/projects', {
      signal,
      params: {
        query: {
          limit: 1,
          offset: 0,
          q: '%_',
          project_key: '-',
          registry_instance_id: registry,
          space_id: space.space_id,
        },
      },
    })
  })
  it('retains the true total for an empty page beyond the end', async () => {
    const data = { ...page(), offset: 99, projects: [], has_more: false }
    get.mockResolvedValue({ data, response: new Response() })
    expect((await getVisibleProjectCatalog(identity, { space, limit: 1, offset: 99 })).total).toBe(
      2,
    )
  })
  it('accepts equal names from distinct immutable projects without picking one', async () => {
    const data = page()
    data.limit = 2
    data.has_more = false
    data.projects.push({ ...data.projects[0]!, id: space.space_id, key: 'SECOND' })
    get.mockResolvedValue({ data, response: new Response() })
    expect((await getVisibleProjectCatalog(identity, { space, limit: 2 })).projects).toHaveLength(2)
  })
  it.each([
    (v: VisibleProjectCatalog) => {
      v.identity = { ...identity, subject: 'other' }
    },
    (v: VisibleProjectCatalog) => {
      v.space = { ...space, registry_instance_id: space.space_id }
    },
    (v: VisibleProjectCatalog) => {
      v.tracker_instance_id = '00000000-0000-0000-0000-000000000000'
    },
    (v: VisibleProjectCatalog) => {
      v.validity.valid_until = Math.floor(Date.now() / 1000)
    },
    (v: VisibleProjectCatalog) => {
      v.validity.valid_until = v.validity.observed_at + 61
    },
    (v: VisibleProjectCatalog) => {
      v.has_more = false
    },
    (v: VisibleProjectCatalog) => {
      v.projects = []
    },
    (v: VisibleProjectCatalog) => {
      v.offset = 1
    },
    (v: VisibleProjectCatalog) => {
      v.projects[0]!.todo_count = -1
    },
  ])('rejects wrong actor/context/instance, expiry and incoherent page %#', async (mutate) => {
    const data = page()
    mutate(data)
    get.mockResolvedValue({ data, response: new Response() })
    await expect(getVisibleProjectCatalog(identity, { space, limit: 1 })).rejects.toMatchObject({
      status: 502,
    })
  })
  it('rejects duplicate IDs and a response outside an exact key filter', async () => {
    const data = page()
    data.limit = 2
    data.has_more = false
    data.projects.push({ ...data.projects[0]! })
    get.mockResolvedValue({ data, response: new Response() })
    await expect(getVisibleProjectCatalog(identity, { space, limit: 2 })).rejects.toMatchObject({
      status: 502,
    })
    get.mockResolvedValue({
      data: { ...page(), total: 1, has_more: false },
      response: new Response(),
    })
    await expect(
      getVisibleProjectCatalog(identity, { space, limit: 1, projectKey: 'OTHER' }),
    ).rejects.toMatchObject({ status: 502 })
  })
  it.each([{ limit: 101 }, { offset: -1 }, { q: 'я'.repeat(201) }, { projectKey: 'bad key' }])(
    'rejects invalid query before HTTP %#',
    async (query) => {
      await expect(getVisibleProjectCatalog(identity, query)).rejects.toThrow()
      expect(get).not.toHaveBeenCalled()
    },
  )
  it.each([403, 503])('does not retry or fall back after %s', async (status) => {
    get.mockResolvedValue({ error: {}, response: new Response(null, { status }) })
    await expect(getVisibleProjectCatalog(identity)).rejects.toMatchObject({ status })
    expect(get).toHaveBeenCalledTimes(1)
  })
  it('separates actor, space, page, literal search and exact key in cache identities', () => {
    const initial = visibleProjectKeys.page(identity, { space })
    for (const query of [
      { space: null },
      { space, offset: 1 },
      { space, q: 'x' },
      { space, projectKey: '-' },
    ])
      expect(visibleProjectKeys.page(identity, query)).not.toEqual(initial)
    expect(visibleProjectKeys.page({ ...identity, subject: 'other' }, { space })).not.toEqual(
      initial,
    )
  })
})
