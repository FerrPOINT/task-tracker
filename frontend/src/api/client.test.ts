import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const createClient = vi.hoisted(() => vi.fn(() => ({ use: vi.fn() })))
const authState = vi.hoisted(() => ({
  token: null as string | null,
  logout: vi.fn(),
  setAuth: vi.fn((next: { token: string }) => {
    authState.token = next.token
  }),
}))

vi.mock('openapi-fetch', () => ({ default: createClient }))
vi.mock('@/shared/auth/store', () => ({
  useAuthStore: {
    getState: vi.fn(() => authState),
    setState: vi.fn((next: Partial<typeof authState>) => Object.assign(authState, next)),
  },
}))

describe('API client transport', () => {
  beforeEach(() => {
    vi.resetModules()
    createClient.mockReset()
    createClient.mockImplementation(() => ({ use: vi.fn() }))
    authState.token = null
    authState.logout.mockClear()
    authState.setAuth.mockClear()
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('includes HttpOnly cookies when the API is cross-origin', async () => {
    await import('./client')
    expect(createClient).toHaveBeenCalledWith(expect.objectContaining({ credentials: 'include' }))
  })

  it('does not retry a revoked central session through local refresh', async () => {
    let middleware: {
      onRequest: (args: { request: Request }) => Request
      onResponse: (args: {
        request: Request
        response: Response
        options: { fetch: (request: Request) => Promise<Response> }
      }) => Response
    }
    createClient.mockReturnValueOnce({
      use: vi.fn((next) => {
        middleware = next
      }),
    })
    authState.token = 'old-token'
    const fetchMock = vi.fn(async () => new Response('{}', { status: 200 }))
    vi.stubGlobal('fetch', fetchMock)
    const assign = vi.fn()
    vi.stubGlobal('window', { location: { assign } })

    await import('./client')
    const request = middleware!.onRequest({
      request: new Request('https://api.example.test/api/v1/issues', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ summary: 'Retry me' }),
      }),
    })
    middleware!.onResponse({
      request,
      response: new Response(null, { status: 401 }),
      options: { fetch: fetchMock },
    })

    expect(authState.logout).toHaveBeenCalledOnce()
    expect(assign).toHaveBeenCalledWith('/login')
    expect(fetchMock).not.toHaveBeenCalled()
  })
})
