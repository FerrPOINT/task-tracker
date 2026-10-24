import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

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
  beforeEach(() => {
    getState.mockReset()
    refreshAccessToken.mockReset()
  })
  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
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

  it('refreshes once and retries only within the confirmed same local identity', async () => {
    let state = { token: 'old-token', userId: 'same-user' }
    getState.mockImplementation(() => state)
    refreshAccessToken.mockImplementation(async () => {
      state = { token: 'new-token', userId: 'same-user' }
      return true
    })
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 401 }))
      .mockResolvedValueOnce(new Response('{"issues":[]}', { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)
    await fetchIssueExport('TT', 'json')
    expect(refreshAccessToken).toHaveBeenCalledOnce()
    expect(fetchMock).toHaveBeenCalledTimes(2)
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://tracker.test/api/v1/export/json',
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: 'Bearer new-token' }),
      }),
    )
  })
  it('ignores an old 401 after another identity has become current', async () => {
    let state = { token: 'a', userId: 'A' },
      complete: (response: Response) => void = () => {}
    getState.mockImplementation(() => state)
    const fetchMock = vi.fn(
      () =>
        new Promise<Response>((resolve) => {
          complete = resolve
        }),
    )
    vi.stubGlobal('fetch', fetchMock)
    const checked = expect(fetchIssueExport('TT', 'csv')).rejects.toThrow()
    state = { token: 'b', userId: 'B' }
    complete(new Response(null, { status: 401 }))
    await checked
    expect(refreshAccessToken).not.toHaveBeenCalled()
    expect(fetchMock).toHaveBeenCalledTimes(1)
  })
  it('does not retry if identity changes while refresh is awaited', async () => {
    let state = { token: 'a', userId: 'A' }
    getState.mockImplementation(() => state)
    refreshAccessToken.mockImplementation(async () => {
      state = { token: 'b', userId: 'B' }
      return true
    })
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 401 }))
    vi.stubGlobal('fetch', fetchMock)
    await expect(fetchIssueExport('TT', 'json')).rejects.toThrow()
    expect(refreshAccessToken).toHaveBeenCalledOnce()
    expect(fetchMock).toHaveBeenCalledTimes(1)
  })
  it('does not refresh or repeat after the initiating resource context retires', async () => {
    let active = true,
      complete: (response: Response) => void = () => {}
    getState.mockReturnValue({ token: 'a', userId: 'A' })
    const fetchMock = vi.fn(
      () =>
        new Promise<Response>((resolve) => {
          complete = resolve
        }),
    )
    vi.stubGlobal('fetch', fetchMock)
    const checked = expect(
      fetchIssueExport('TT', 'csv', { isCurrent: () => active }),
    ).rejects.toThrow('context changed')
    active = false
    complete(new Response(null, { status: 401 }))
    await checked
    expect(refreshAccessToken).not.toHaveBeenCalled()
    expect(fetchMock).toHaveBeenCalledTimes(1)
  })
})
