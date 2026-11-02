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
let reports: typeof import('./reports')
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
  reports = await import('./reports')
})
afterEach(() => vi.unstubAllGlobals())

const cases = [
  ['getVelocityReport', 'velocity', 'project_id', 'sprints'],
  ['getBurndownReport', 'burndown', 'sprint_id', 'points'],
  ['getCumulativeFlowReport', 'cumulative-flow', 'project_id', 'points'],
  ['getControlChartReport', 'control-chart', 'project_id', 'points'],
] as const

it.each(cases)(
  '%s preserves HTTP status through the real OpenAPI client',
  async (method, path, parameter) => {
    for (const status of [401, 403, 404, 503]) {
      transport.mockResolvedValue(
        Response.json({ error: { message: 'server-only-detail' } }, { status }),
      )
      const failure = await reports[method]('report/owner').catch((error: unknown) => error)
      expect(failure).toBeInstanceOf(ApiError)
      expect((failure as ApiError).status).toBe(status)
      expect((failure as ApiError).message).not.toContain('server-only-detail')
      expect(resourceErrorState(failure, undefined, { hasData: true }).kind).toBe(
        status === 404 ? 'not-found' : status === 503 ? 'partial-error' : 'permission-denied',
      )
      const request = transport.mock.lastCall![0] as Request
      expect(request.method).toBe('GET')
      expect(new URL(request.url).pathname).toBe(`/api/v1/reports/${path}`)
      expect(new URL(request.url).searchParams.get(parameter)).toBe('report/owner')
    }
    expect(transport).toHaveBeenCalledTimes(4)
    expect(recheck).toHaveBeenCalledOnce()
  },
)

it.each(cases)(
  '%s distinguishes a confirmed empty report from a malformed successful body',
  async (method, _path, _parameter, list) => {
    const body = { [list]: [] }
    transport.mockResolvedValue(Response.json(body))
    await expect(reports[method]('valid')).resolves.toEqual(body)
    for (const invalid of [null, {}, { [list]: null }, { [list]: {} }]) {
      transport.mockResolvedValue(Response.json(invalid))
      await expect(reports[method]('invalid')).rejects.toBeInstanceOf(InvalidResponseError)
    }
    expect(transport).toHaveBeenCalledTimes(5)
  },
)
