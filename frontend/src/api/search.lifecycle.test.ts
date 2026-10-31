import { afterEach, describe, expect, it, vi } from 'vitest'
import { searchIssues } from './search'

const wire = vi.hoisted(() => {
  vi.stubEnv('VITE_API_BASE_URL', 'https://tracker.example.test/tracker')
  const fetch = vi.fn()
  vi.stubGlobal('fetch', fetch)
  return fetch
})
afterEach(() => wire.mockReset())
describe('lifecycle search transport', () => {
  it('sends stage and operational status separately with server pagination and the mounted prefix', async () => {
    const state = { stage: 'Analysis', status: 'Ready', revision: 9 }
    wire.mockResolvedValue(
      new Response(
        JSON.stringify({ issues: [{ id: 'task', lifecycle: state, status: 'Ready' }] }),
        { headers: { 'content-type': 'application/json' } },
      ),
    )
    const issues = await searchIssues({
      lifecycle_stage: 'Analysis',
      lifecycle_status: 'Ready',
      limit: 25,
      offset: 50,
    })
    const url = new URL((wire.mock.calls[0]?.[0] as Request).url)
    expect(url.pathname).toBe('/tracker/api/v1/search')
    expect(Object.fromEntries(url.searchParams)).toEqual({
      lifecycle_stage: 'Analysis',
      lifecycle_status: 'Ready',
      limit: '25',
      offset: '50',
    })
    expect(issues[0]?.lifecycle).toEqual(state)
  })
  it('does not silently retry a rejected filter or return an empty successful page', async () => {
    wire.mockResolvedValue(
      new Response(JSON.stringify({ error: 'invalid filter' }), {
        status: 400,
        headers: { 'content-type': 'application/json' },
      }),
    )
    await expect(searchIssues({ lifecycle_status: 'Ready' })).rejects.toMatchObject({ status: 400 })
    expect(wire).toHaveBeenCalledTimes(1)
  })
})
