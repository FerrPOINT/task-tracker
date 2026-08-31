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

  it('retries a consumed request body after refreshing the access token', async () => {
    let middleware: {
      onRequest: (args: { request: Request }) => Request
      onResponse: (args: {
        request: Request
        response: Response
        options: { fetch: (request: Request) => Promise<Response> }
      }) => Promise<Response>
    }
    createClient.mockReturnValueOnce({
      use: vi.fn((next) => {
        middleware = next
      }),
    })
    authState.token = 'old-token'
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const url = input instanceof Request ? input.url : String(input)
      if (url.endsWith('/api/v1/auth/refresh')) {
        return new Response(
          JSON.stringify({
            access_token: 'new-token',
            user_id: 'u1',
            email: 'demo@example.test',
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        )
      }
      return new Response('{}', { status: 200 })
    })
    vi.stubGlobal('fetch', fetchMock)

    await import('./client')
    const request = middleware!.onRequest({
      request: new Request('https://api.example.test/api/v1/issues', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ summary: 'Retry me' }),
      }),
    })
    await request.text()

    await middleware!.onResponse({
      request,
      response: new Response(null, { status: 401 }),
      options: { fetch: fetchMock },
    })

    expect(fetchMock).toHaveBeenCalledTimes(2)
    const retryCall = fetchMock.mock.calls[1]
    if (!retryCall) throw new Error('missing retry request')
    const retry = retryCall[0] as Request
    expect(retry.headers.get('Authorization')).toBe('Bearer new-token')
    await expect(retry.json()).resolves.toEqual({ summary: 'Retry me' })
  })
})
