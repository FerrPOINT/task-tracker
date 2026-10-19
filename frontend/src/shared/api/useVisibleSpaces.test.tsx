import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useVisibleSpaces } from './useVisibleSpaces'
import { acceptSpaceSession, useSpaceSession } from '@/shared/auth/space-session'
import { ssoConfig, useAuthStore } from '@/shared/auth/store'
import type { VisibleSpaceCatalog } from '@/api/space-catalog'

const read = vi.hoisted(() => vi.fn())
vi.mock('@/api/space-catalog', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/api/space-catalog')>()),
  getVisibleSpaceCatalog: read,
}))
const clients: QueryClient[] = []
function login(subject = 'central-a') {
  useAuthStore
    .getState()
    .setAuth({ token: subject, userId: 'local-' + subject, email: 'fixture@example.test' })
  acceptSpaceSession(
    {
      accessToken: subject,
      subject,
      email: 'fixture@example.test',
      name: 'Fixture',
      returnTo: '/',
      expiresAt: Date.now() + 60_000,
    },
    { id: 'local-' + subject },
  )
}
function page(subject = 'central-a'): VisibleSpaceCatalog {
  const now = Math.floor(Date.now() / 1000)
  return {
    schema_version: 2,
    identity: { issuer: ssoConfig.issuer, subject },
    policy_revision: 1,
    validity: { observed_at: now, valid_until: now + 2 },
    spaces: [],
    total: 0,
    offset: 0,
    limit: 10,
    has_more: false,
  }
}
function Probe() {
  const query = useVisibleSpaces()
  return (
    <>
      <output data-testid="catalog">
        {query.data?.identity.subject ?? (query.unavailable ? 'unavailable' : 'pending')}
      </output>
      <button onClick={() => void query.refetch()}>Retry</button>
    </>
  )
}
function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } })
  clients.push(client)
  return render(
    <QueryClientProvider client={client}>
      <Probe />
    </QueryClientProvider>,
  )
}
beforeEach(() => {
  read.mockReset()
  useAuthStore.getState().logout()
  useSpaceSession.setState({ session: null })
})
afterEach(() => {
  cleanup()
  clients.splice(0).forEach((client) => client.clear())
  vi.useRealTimers()
})
describe('visible catalog original validity', () => {
  it('does not query without a current SSO context', () => {
    mount()
    expect(read).not.toHaveBeenCalled()
    expect(screen.getByTestId('catalog')).toHaveTextContent('unavailable')
  })
  it('hides expired data while revalidation remains unanswered', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-10-20T00:00:00Z'))
    login()
    read.mockResolvedValueOnce(page()).mockImplementation(() => new Promise(() => {}))
    mount()
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1)
    })
    expect(screen.getByTestId('catalog')).toHaveTextContent('central-a')
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2100)
    })
    expect(screen.getByTestId('catalog')).toHaveTextContent('unavailable')
    expect(read).toHaveBeenCalledTimes(2)
  })
  it('does not expose a late previous actor response', async () => {
    login()
    let resolveOld!: (v: VisibleSpaceCatalog) => void
    read
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveOld = resolve
          }),
      )
      .mockImplementation(async (identity) => page(identity.subject))
    mount()
    await waitFor(() => expect(read).toHaveBeenCalledTimes(1))
    act(() => {
      login('central-b')
    })
    await waitFor(() => expect(screen.getByTestId('catalog')).toHaveTextContent('central-b'))
    await act(async () => {
      resolveOld(page('central-a'))
    })
    expect(screen.getByTestId('catalog')).toHaveTextContent('central-b')
  })
  it('does not retain cached positive data after a failed fresh read', async () => {
    login()
    read.mockResolvedValueOnce(page()).mockRejectedValue(new Error('unavailable'))
    mount()
    await waitFor(() => expect(screen.getByTestId('catalog')).toHaveTextContent('central-a'))
    await act(async () => {
      screen.getByText('Retry').click()
    })
    await waitFor(() => expect(screen.getByTestId('catalog')).toHaveTextContent('unavailable'))
    expect(read).toHaveBeenCalledTimes(2)
  })
})
