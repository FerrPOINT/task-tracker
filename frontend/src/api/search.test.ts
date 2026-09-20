import { describe, expect, it, vi } from 'vitest'

const get = vi.hoisted(() => vi.fn())
vi.mock('./client', () => ({ api: { GET: get } }))

import { SearchRequestError, searchIssues } from './search'

describe('searchIssues', () => {
  it('passes bounded pagination to the API', async () => {
    get.mockResolvedValueOnce({ data: { issues: [] }, response: { status: 200 } })

    await searchIssues({ q: 'test', limit: 26, offset: 25 })

    expect(get).toHaveBeenCalledWith('/api/v1/search', {
      params: { query: { q: 'test', limit: 26, offset: 25 } },
    })
  })

  it('retains the HTTP status for query validation', async () => {
    get.mockResolvedValueOnce({ data: undefined, response: { status: 400 } })

    await expect(searchIssues({ jql: 'invalid' })).rejects.toEqual(new SearchRequestError(400))
  })
})
