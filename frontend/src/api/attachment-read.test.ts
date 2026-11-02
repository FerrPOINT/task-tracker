import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { ApiError, InvalidResponseError } from '@pdlc/ui/lib'
import { resourceErrorState } from '@pdlc/ui/ui'
import type { Attachment } from './attachment'
let listAttachments: typeof import('./attachment').listAttachments

const recheck = vi.hoisted(() => vi.fn())
vi.mock('@pdlc/ui/platform', async (original) => ({
  ...(await original<typeof import('@pdlc/ui/platform')>()),
  getPlatformProduct: () => ({ recheckAuthorization: recheck }),
}))
const NativeRequest = Request
const transport = vi.fn()
beforeEach(async () => {
  recheck.mockClear()
  transport.mockReset()
  vi.stubGlobal('fetch', transport)
  // Match browser document-relative Request resolution, as in deployment-path.test.ts.
  vi.stubGlobal(
    'Request',
    class extends NativeRequest {
      constructor(input: RequestInfo | URL, init?: RequestInit) {
        super(typeof input === 'string' ? new URL(input, window.location.origin) : input, init)
      }
    },
  )
  // openapi-fetch captures Request/fetch when the product client is constructed.
  ;({ listAttachments } = await import('./attachment'))
})
afterEach(() => vi.unstubAllGlobals())

it.each([
  [401, 'permission-denied'],
  [403, 'permission-denied'],
  [404, 'not-found'],
  [503, 'error'],
] as const)(
  'preserves HTTP %s from the real OpenAPI client for Base resource classification',
  async (status, kind) => {
    transport.mockResolvedValue(
      Response.json({ error: { message: 'server-only-detail' } }, { status }),
    )
    const failure = await listAttachments('one/two').catch((error) => error)
    expect(failure).toBeInstanceOf(ApiError)
    expect(failure.status).toBe(status)
    expect(failure.message).toBe('Failed to load attachments')
    expect(resourceErrorState(failure).kind).toBe(kind)
    expect(transport).toHaveBeenCalledOnce()
    const request = transport.mock.calls[0]![0] as Request
    expect(request.method).toBe('GET')
    expect(new URL(request.url).pathname).toBe('/api/v1/issues/one%2Ftwo/attachments')
    expect(recheck).toHaveBeenCalledTimes(status === 401 ? 1 : 0)
  },
)

const confirmedAttachment: Attachment = {
  id: '00000000-0000-4000-8000-000000000011',
  issue_id: '00000000-0000-4000-8000-000000000012',
  author_id: '00000000-0000-4000-8000-000000000013',
  file_name: 'known.txt',
  content_type: 'text/plain',
  size_bytes: 4,
  created_at: '2026-10-24T00:00:00Z',
}
it.each([{ attachments: [] }, { attachments: [confirmedAttachment] }])(
  'accepts only confirmed attachment arrays: %j',
  async ({ attachments }) => {
    transport.mockResolvedValue(Response.json({ attachments }))
    await expect(listAttachments('valid')).resolves.toEqual(attachments)
    expect(transport).toHaveBeenCalledOnce()
  },
)

it.each([{}, { attachments: null }, { attachments: {} }])(
  'rejects a malformed successful body without claiming emptiness: %j',
  async (body) => {
    transport.mockResolvedValue(Response.json(body))
    await expect(listAttachments('invalid')).rejects.toBeInstanceOf(InvalidResponseError)
  },
)
