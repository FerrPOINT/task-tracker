import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { ApiError, InvalidResponseError } from '@pdlc/ui/lib'
import { resourceErrorState } from '@pdlc/ui/ui'

const recheck = vi.hoisted(() => vi.fn())
vi.mock('@pdlc/ui/platform', async (original) => ({
  ...(await original<typeof import('@pdlc/ui/platform')>()),
  getPlatformProduct: () => ({ recheckAuthorization: recheck }),
}))
const NativeRequest = Request
const transport = vi.fn()
let fields: typeof import('./label') & typeof import('./link')
beforeEach(async () => {
  recheck.mockClear()
  transport.mockReset()
  vi.stubGlobal('fetch', transport)
  vi.stubGlobal(
    'Request',
    class extends NativeRequest {
      constructor(input: RequestInfo | URL, init?: RequestInit) {
        super(typeof input === 'string' ? new URL(input, window.location.origin) : input, init)
      }
    },
  )
  fields = { ...(await import('./label')), ...(await import('./link')) }
})
afterEach(() => vi.unstubAllGlobals())

const readers = [
  ['listProjectLabels', 'labels', '/api/v1/projects/TEST/labels'],
  ['listIssueLabels', 'labels', '/api/v1/issues/TEST/labels'],
  ['listIssueLinks', 'links', '/api/v1/issues/TEST/links'],
] as const

it.each(readers)(
  '%s preserves read status for the Base resource state',
  async (method, _key, path) => {
    for (const status of [401, 403, 404, 503]) {
      transport.mockResolvedValue(Response.json({ message: 'server-only-detail' }, { status }))
      const failure = await fields[method]('TEST').catch((error: unknown) => error)
      expect(failure).toBeInstanceOf(ApiError)
      expect((failure as ApiError).status).toBe(status)
      expect((failure as ApiError).message).not.toContain('server-only-detail')
      expect(resourceErrorState(failure, undefined, { hasData: true }).kind).toBe(
        status === 404 ? 'not-found' : status === 503 ? 'partial-error' : 'permission-denied',
      )
      const request = transport.mock.lastCall![0] as Request
      expect(request.method).toBe('GET')
      expect(new URL(request.url).pathname).toBe(path)
    }
    expect(transport).toHaveBeenCalledTimes(4)
    expect(recheck).toHaveBeenCalledOnce()
  },
)

it.each(readers)(
  '%s distinguishes confirmed emptiness from malformed success',
  async (method, key) => {
    transport.mockResolvedValue(Response.json({ [key]: [] }))
    await expect(fields[method]('TEST')).resolves.toEqual([])
    for (const invalid of [null, {}, { [key]: null }, { [key]: {} }]) {
      transport.mockResolvedValue(Response.json(invalid))
      const failure = await fields[method]('TEST').catch((error: unknown) => error)
      expect(failure).toBeInstanceOf(InvalidResponseError)
      expect(resourceErrorState(failure, vi.fn(), { hasData: true }).kind).toBe('error')
    }
  },
)
