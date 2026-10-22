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

afterEach(() => vi.unstubAllGlobals())

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

it.each(['different-user-token', 'refreshed-session-token', null])(
  'does not invalidate a successor session after an old request returns401: %s',
  async (token) => {
    vi.resetModules()
    createClient.mockReset()
    authState.logout.mockClear()
    let middleware: {
      onRequest: (args: { request: Request }) => Request
      onResponse: (args: { request: Request; response: Response }) => Response
    }
    createClient.mockReturnValueOnce({
      use: vi.fn((value) => {
        middleware = value
      }),
    })
    authState.token = 'old-session-token'
    const assign = vi.fn()
    vi.stubGlobal('window', { location: { assign } })
    await import('./client')
    const request = middleware!.onRequest({
      request: new Request('https://api.example.test/api/v2/projects/QA/spaces'),
    })
    authState.token = token
    middleware!.onResponse({ request, response: new Response(null, { status: 401 }) })
    expect(authState.logout).not.toHaveBeenCalled()
    expect(assign).not.toHaveBeenCalled()
    vi.unstubAllGlobals()
  },
)

it('rejects a streamed body after an application change with the same token', async () => {
  vi.resetModules()
  createClient.mockReset()
  let middleware!: {
    onRequest: (args: { request: Request }) => Request
    onResponse: (args: { request: Request; response: Response }) => Response
  }
  createClient.mockReturnValueOnce({
    use: vi.fn((value) => {
      middleware = value
    }),
  })
  authState.token = 'same-token'
  await import('./client')
  const { clearPlatformPrivateQueries } = await import('@pdlc/ui/platform')
  const request = middleware.onRequest({
    request: new Request('https://api.example.test/api/v1/issues'),
  })
  let source!: ReadableStreamDefaultController<Uint8Array>
  const response = middleware.onResponse({
    request,
    response: new Response(
      new ReadableStream<Uint8Array>({
        start(controller) {
          source = controller
        },
      }),
    ),
  })
  const reading = response.json()
  const rejection = expect(reading).rejects.toMatchObject({
    status: 403,
    code: 'AUTH_CONTEXT_CHANGED',
  })
  clearPlatformPrivateQueries()
  source.enqueue(new TextEncoder().encode('{"private":"old"}'))
  source.close()
  await rejection
})
