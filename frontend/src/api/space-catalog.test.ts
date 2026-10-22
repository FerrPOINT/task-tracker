import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  getVisibleSpaceCatalog,
  spaceCatalogKeys,
  catalogIsCurrent,
  type VisibleSpaceCatalog,
} from './space-catalog'
const GET = vi.hoisted(() => vi.fn())
vi.mock('./client', () => ({ api: { GET } }))
const id = { issuer: 'https://auth.example.test', subject: '00000000-0000-4000-8000-000000000001' }
const registry = '00000000-0000-4000-8000-000000000002'
function page(): VisibleSpaceCatalog {
  const now = Math.floor(Date.now() / 1000)
  return {
    schema_version: 2,
    identity: id,
    policy_revision: 1,
    validity: { observed_at: now, valid_until: now + 40 },
    limit: 10,
    offset: 0,
    total: 2,
    has_more: false,
    spaces: [3, 4].map((i) => ({
      space: {
        registry_instance_id: registry,
        space_id: '00000000-0000-4000-8000-00000000000' + i,
      },
      revision: 1,
      state: 'active',
      metadata: {
        slug: 'space-' + i,
        name: 'Одинаковое имя',
        description: '',
        responsible_subject: id.subject,
      },
    })),
  }
}
function reply(value: VisibleSpaceCatalog) {
  GET.mockResolvedValue({ data: value, response: new Response(null, { status: 200 }) })
}
describe('original-actor catalog client', () => {
  beforeEach(() => GET.mockReset())
  it('preserves equal names as distinct IDs and forwards cancellation and bounded query', async () => {
    const value = page()
    reply(value)
    const signal = new AbortController().signal
    expect(await getVisibleSpaceCatalog(id, {}, signal)).toEqual(value)
    expect(GET).toHaveBeenCalledWith('/api/v2/spaces/visible', {
      params: { query: { limit: 10, offset: 0, q: '' } },
      signal,
    })
  })
  it('accepts missing policy as an empty catalog without a default selection', async () => {
    const value = page()
    value.policy_revision = 0
    value.total = 0
    value.spaces = []
    reply(value)
    expect((await getVisibleSpaceCatalog(id)).spaces).toEqual([])
  })
  it.each([0, 11, 1.5])('rejects out-of-contract limit %s before HTTP', async (limit) => {
    await expect(getVisibleSpaceCatalog(id, { limit })).rejects.toThrow()
    expect(GET).not.toHaveBeenCalled()
  })
  it('validates Unicode length and rejects unknown request fields', async () => {
    reply(page())
    await getVisibleSpaceCatalog(id, { q: 'я'.repeat(200) })
    await expect(getVisibleSpaceCatalog(id, { q: 'я'.repeat(201) })).rejects.toThrow()
    await expect(getVisibleSpaceCatalog(id, { ...{}, unexpected: 1 } as never)).rejects.toThrow()
    expect(GET).toHaveBeenCalledTimes(1)
  })
  it.each([
    (v: VisibleSpaceCatalog) => {
      v.identity = { ...id, subject: 'another-user' }
    },
    (v: VisibleSpaceCatalog) => {
      v.identity = { ...id, issuer: 'https://foreign.example.test' }
    },
    (v: VisibleSpaceCatalog) => {
      v.validity.valid_until = Math.floor(Date.now() / 1000)
    },
    (v: VisibleSpaceCatalog) => {
      v.validity.valid_until = v.validity.observed_at + 61
    },
    (v: VisibleSpaceCatalog) => {
      v.validity.observed_at += 50
      v.validity.valid_until += 50
    },
    (v: VisibleSpaceCatalog) => {
      v.spaces = [v.spaces[0]!, v.spaces[0]!]
    },
    (v: VisibleSpaceCatalog) => {
      v.spaces.pop()
    },
    (v: VisibleSpaceCatalog) => {
      v.has_more = true
    },
    (v: VisibleSpaceCatalog) => {
      v.offset = 10
    },
    (v: VisibleSpaceCatalog) => {
      v.limit = 5
    },
    (v: VisibleSpaceCatalog) => {
      v.policy_revision = 0
    },
  ])('rejects incoherent identity, window or page %#', async (mutate) => {
    const value = page()
    mutate(value)
    reply(value)
    await expect(getVisibleSpaceCatalog(id)).rejects.toMatchObject({
      status: 502,
      kind: 'invalid_response',
    })
  })
  it.each([401, 403, 412, 503])(
    'keeps HTTP %s explicit without a fallback or retry',
    async (status) => {
      GET.mockResolvedValue({ error: {}, response: new Response(null, { status }) })
      await expect(getVisibleSpaceCatalog(id)).rejects.toMatchObject({ status })
      expect(GET).toHaveBeenCalledTimes(1)
    },
  )
  it('separates actor/query keys and does not extend an expired cached page', () => {
    const value = page()
    expect(catalogIsCurrent(value, id)).toBe(true)
    value.validity.valid_until = Math.floor(Date.now() / 1000)
    expect(catalogIsCurrent(value, id)).toBe(false)
    expect(spaceCatalogKeys.page(id)).not.toEqual(
      spaceCatalogKeys.page({ ...id, subject: 'other' }),
    )
    expect(spaceCatalogKeys.page(id, { offset: 10 })).not.toEqual(spaceCatalogKeys.page(id))
  })
})
