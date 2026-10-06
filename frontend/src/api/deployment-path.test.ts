import { afterEach, expect, it, vi } from 'vitest'

vi.mock('@/shared/auth/store', () => ({ useAuthStore: { getState: () => ({ token: null, logout: vi.fn() }) } }))

afterEach(() => { vi.unstubAllEnvs(); vi.unstubAllGlobals(); vi.resetModules() })

it('keeps API requests under the deployed section prefix', async () => {
  vi.resetModules()
  vi.stubEnv('BASE_URL', '/tracker/')
  vi.stubEnv('VITE_API_BASE_URL', undefined)
  const fetchMock = vi.fn().mockResolvedValue(new Response('{}', { status: 200, headers: { 'Content-Type': 'application/json' } }))
  vi.stubGlobal('fetch', fetchMock)
  // Node's Request lacks the browser's document-relative URL resolution.
  const NativeRequest = Request
  vi.stubGlobal('Request', class extends NativeRequest {
    constructor(input: RequestInfo | URL, init?: RequestInit) {
      super(typeof input === 'string' ? new URL(input, window.location.origin) : input, init)
    }
  })
  const client = await import('./client')
  await client.api.GET('/api/v1/health' as never)
  const input = fetchMock.mock.calls[0]![0] as string | Request
  const path = typeof input === 'string' ? input : new URL(input.url).pathname
  expect(path).toBe('/tracker/api/v1/health')
})
