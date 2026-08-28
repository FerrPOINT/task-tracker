import { beforeEach, describe, expect, it, vi } from 'vitest'
import { downloadAttachment } from './attachment'
import { refreshAccessToken } from './client'
import { useAuthStore } from '@/shared/auth/store'
import type { Attachment } from './attachment'

vi.mock('./client', () => ({
  api: {},
  apiBaseUrl: '',
  refreshAccessToken: vi.fn(),
}))

const attachment: Attachment = {
  id: 'a1',
  issue_id: 'i1',
  author_id: 'u1',
  file_name: 'report.txt',
  content_type: 'text/plain',
  size_bytes: 6,
  created_at: '2026-08-01T10:00:00Z',
}

beforeEach(() => {
  vi.restoreAllMocks()
  document.body.innerHTML = ''
  useAuthStore.setState({
    token: 'old-token',
    userId: 'u1',
    email: 'user@example.com',
    username: null,
    displayName: null,
  })
  Object.defineProperty(URL, 'createObjectURL', {
    configurable: true,
    value: vi.fn(() => 'blob:test-download'),
  })
  Object.defineProperty(URL, 'revokeObjectURL', {
    configurable: true,
    value: vi.fn(),
  })
  vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {})
})

describe('attachment api', () => {
  it('refreshes access token and retries download after 401', async () => {
    vi.mocked(refreshAccessToken).mockImplementation(async () => {
      useAuthStore.setState({ token: 'new-token' })
      return true
    })
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 401 }))
      .mockResolvedValueOnce(new Response(new Blob(['report']), { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)

    await downloadAttachment(attachment)

    expect(refreshAccessToken).toHaveBeenCalledTimes(1)
    expect(fetchMock).toHaveBeenCalledTimes(2)
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      '/api/v1/attachments/a1/download',
      expect.objectContaining({
        headers: { Authorization: 'Bearer new-token' },
      }),
    )
  })
})
