import { afterEach, describe, expect, it, vi } from 'vitest'

const refreshAccessToken = vi.hoisted(() => vi.fn())
const getState = vi.hoisted(() => vi.fn())

vi.mock('./client', () => ({
  apiBaseUrl: 'http://tracker.test',
  refreshAccessToken,
}))

vi.mock('@/shared/auth/store', () => ({
  useAuthStore: { getState },
}))

import { fetchIssueExport } from './export'

describe('fetchIssueExport', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    vi.clearAllMocks()
  })

  it('downloads a CSV export with the authenticated project request', async () => {
    getState.mockReturnValue({ token: 'access-token' })
    const fetchMock = vi.fn().mockResolvedValue(
      new Response('key,summary\nTT-1,Export me\n', {
        status: 200,
        headers: {
          'content-disposition': 'attachment; filename="issues.csv"',
          'content-type': 'text/csv; charset=utf-8',
        },
      }),
    )
    vi.stubGlobal('fetch', fetchMock)

    const exportFile = await fetchIssueExport('TT', 'csv')

    expect(fetchMock).toHaveBeenCalledWith(
      'http://tracker.test/api/v1/export/csv',
      expect.objectContaining({
        method: 'POST',
        headers: expect.objectContaining({ Authorization: 'Bearer access-token' }),
        body: JSON.stringify({ project_key: 'TT' }),
      }),
    )
    expect(exportFile.filename).toBe('issues.csv')
    expect(exportFile.blob).toBeTruthy()
  })

  it('refreshes once before retrying an expired export request', async () => {
    getState.mockReturnValueOnce({ token: 'old-token' }).mockReturnValue({ token: 'new-token' })
    refreshAccessToken.mockResolvedValue(true)
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 401 }))
      .mockResolvedValueOnce(new Response('{"issues":[]}', { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)

    await fetchIssueExport('TT', 'json')

    expect(refreshAccessToken).toHaveBeenCalledOnce()
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://tracker.test/api/v1/export/json',
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: 'Bearer new-token' }),
      }),
    )
  })
})
