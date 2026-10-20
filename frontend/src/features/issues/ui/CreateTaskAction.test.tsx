import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { CreateTaskAction, newPmChatUrl } from './CreateTaskAction'

const mocks = vi.hoisted(() => ({
  services: vi.fn(),
  product: vi.fn(),
  navigate: vi.fn(),
  error: vi.fn(),
}))
const wire = vi.hoisted(() => {
  vi.stubEnv('VITE_API_BASE_URL', 'https://tracker.example.test')
  const fetch = vi.fn()
  vi.stubGlobal('fetch', fetch)
  return fetch
})
vi.mock('@pdlc/ui/ui', async (original) => ({
  ...(await original<object>()),
  usePlatformServices: mocks.services,
}))
vi.mock('@pdlc/ui/platform', async (original) => ({
  ...(await original<object>()),
  getPlatformProduct: mocks.product,
  platformNavigate: mocks.navigate,
}))
vi.mock('sonner', () => ({ toast: { error: mocks.error } }))

const id = 'af51e677-f2f4-4af5-a4f3-5d8e35f0a719'
let requests: Request[]
beforeEach(() => {
  vi.clearAllMocks()
  mocks.services.mockReturnValue({
    source: 'runtime',
    services: [{ key: 'fleet-control', ui_url: 'https://fleet.example.test/ui/' }],
  })
  mocks.product.mockReturnValue(null)
  requests = []
  wire.mockImplementation(async (request: Request) => {
    requests.push(request)
    return new Response(JSON.stringify({ id, key: 'TT', name: 'Task Tracker' }), {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    })
  })
})
afterAll(() => {
  vi.unstubAllGlobals()
  vi.unstubAllEnvs()
})

describe('new task opens the existing empty PM route', () => {
  it('resolves project key through the production GET adapter, never creates an issue or chat', async () => {
    render(<CreateTaskAction projectKey="TT" />)
    expect(requests).toHaveLength(0)
    expect(mocks.navigate).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Создать' }))
    await waitFor(() =>
      expect(mocks.navigate).toHaveBeenCalledWith(
        `https://fleet.example.test/ui/chats/new?project=${id}`,
      ),
    )
    expect(requests).toHaveLength(1)
    expect(requests[0]!.method).toBe('GET')
    expect(new URL(requests[0]!.url).pathname).toBe('/api/v1/projects/TT')
    expect(requests[0]!.body).toBeNull()
    expect(mocks.error).not.toHaveBeenCalled()
  })

  it('opens an unscoped empty chat without preselecting a project or writing anything', async () => {
    render(<CreateTaskAction />)
    fireEvent.click(screen.getByRole('button', { name: 'Создать' }))
    await waitFor(() =>
      expect(mocks.navigate).toHaveBeenCalledWith('https://fleet.example.test/ui/chats/new'),
    )
    expect(requests).toHaveLength(0)
  })

  it.each(['fallback-unreachable', 'fallback-empty', 'fallback-invalid'])(
    'rejects display defaults when Base catalog source is %s',
    async (source) => {
      mocks.services.mockReturnValue({
        source,
        services: [{ key: 'fleet-control', ui_url: 'https://untrusted.test' }],
      })
      render(<CreateTaskAction projectKey="TT" />)
      fireEvent.click(screen.getByRole('button', { name: 'Создать' }))
      await waitFor(() =>
        expect(mocks.error).toHaveBeenCalledWith(expect.stringContaining('Fleet недоступен')),
      )
      expect(mocks.navigate).not.toHaveBeenCalled()
      expect(requests).toHaveLength(0)
      expect(screen.queryByRole('link')).not.toBeInTheDocument()
    },
  )

  it.each([{ services: [] }, { services: [{ key: 'fleet-control', ui_url: null }] }])(
    'reports missing Fleet UI mapping',
    async ({ services }) => {
      mocks.services.mockReturnValue({ source: 'runtime', services })
      render(<CreateTaskAction />)
      fireEvent.click(screen.getByRole('button', { name: 'Создать' }))
      await waitFor(() => expect(mocks.error).toHaveBeenCalled())
      expect(mocks.navigate).not.toHaveBeenCalled()
    },
  )

  it.each([{ id, key: 'WRONG' }, { id: 'TT', key: 'TT' }, { key: 'TT' }])(
    'does not substitute a key or another project for immutable mapping %j',
    async (project) => {
      wire.mockImplementation(async () => new Response(JSON.stringify(project), { status: 200 }))
      render(<CreateTaskAction projectKey="TT" />)
      fireEvent.click(screen.getByRole('button', { name: 'Создать' }))
      await waitFor(() => expect(mocks.error).toHaveBeenCalled())
      expect(mocks.navigate).not.toHaveBeenCalled()
    },
  )

  it('reports unavailable project mapping without a manual-form fallback', async () => {
    wire.mockImplementation(async () => new Response('{}', { status: 503 }))
    render(<CreateTaskAction projectKey="TT" />)
    fireEvent.click(screen.getByRole('button', { name: 'Создать' }))
    await waitFor(() => expect(mocks.error).toHaveBeenCalled())
    expect(mocks.navigate).not.toHaveBeenCalled()
    expect(screen.queryByRole('link')).not.toBeInTheDocument()
  })

  it('shows loading and ignores duplicate presses until the mapping GET resolves', async () => {
    let finish!: (value: Response) => void
    wire.mockImplementation(
      () =>
        new Promise<Response>((resolve) => {
          finish = resolve
        }),
    )
    render(<CreateTaskAction projectKey="TT" />)
    const button = screen.getByRole('button', { name: 'Создать' })
    fireEvent.click(button)
    fireEvent.click(button)
    expect(button).toBeDisabled()
    expect(button).toHaveAttribute('aria-busy', 'true')
    expect(button).toHaveAttribute('title', 'Загружаем проект…')
    await waitFor(() => expect(wire).toHaveBeenCalledTimes(1))
    await act(async () => finish(new Response(JSON.stringify({ id, key: 'TT' }), { status: 200 })))
    expect(mocks.navigate).toHaveBeenCalledTimes(1)
  })

  it('does not navigate using a mapping resolved after project context changed', async () => {
    let finish!: (value: Response) => void
    wire.mockImplementation(
      () =>
        new Promise<Response>((resolve) => {
          finish = resolve
        }),
    )
    const view = render(<CreateTaskAction projectKey="TT" />)
    fireEvent.click(screen.getByRole('button', { name: 'Создать' }))
    await waitFor(() => expect(wire).toHaveBeenCalledTimes(1))
    view.rerender(<CreateTaskAction projectKey="OTHER" />)
    await act(async () => finish(new Response(JSON.stringify({ id, key: 'TT' }), { status: 200 })))
    expect(mocks.navigate).not.toHaveBeenCalled()
  })

  it('holds pending or failed Namespace context instead of dropping the project', () => {
    const view = render(<CreateTaskAction contextPending />)
    const button = screen.getByRole('button', { name: 'Создать' })
    expect(button).toBeDisabled()
    view.rerender(<CreateTaskAction contextError="Контекст проекта недоступен" />)
    fireEvent.click(button)
    expect(mocks.error).toHaveBeenCalledWith('Контекст проекта недоступен')
    expect(mocks.navigate).not.toHaveBeenCalled()
  })

  it('uses the configured host Fleet destination and public navigation bridge', async () => {
    mocks.product.mockReturnValue({
      services: [{ key: 'fleet-control', origin: 'https://host-fleet.test/apps/fleet/' }],
    })
    render(<CreateTaskAction projectKey="TT" />)
    fireEvent.click(screen.getByRole('button', { name: 'Создать' }))
    await waitFor(() =>
      expect(mocks.navigate).toHaveBeenCalledWith(
        `https://host-fleet.test/apps/fleet/chats/new?project=${id}`,
      ),
    )
  })

  it('does not bypass the host service allowlist using a standalone catalog', () => {
    mocks.product.mockReturnValue({ services: [] })
    render(<CreateTaskAction />)
    fireEvent.click(screen.getByRole('button', { name: 'Создать' }))
    expect(mocks.navigate).not.toHaveBeenCalled()
    expect(mocks.error).toHaveBeenCalled()
  })
})

describe('trusted new-chat URL', () => {
  it.each([
    'javascript:alert(1)',
    'data:text/html,unsafe',
    'https://user:secret@fleet.test',
    '//foreign.test',
  ])('%s is rejected', (base) => {
    expect(() => newPmChatUrl(base, id)).toThrow()
  })
  it('never transports tokens, task IDs, filters or old fragments from the service URL', () => {
    const url = new URL(newPmChatUrl('https://fleet.test/?token=secret&task_id=old#draft', id))
    expect([...url.searchParams.keys()]).toEqual(['project'])
    expect(url.hash).toBe('')
    expect(url.pathname).toBe('/chats/new')
  })
})
