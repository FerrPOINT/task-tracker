import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, fireEvent, render, screen, waitFor, cleanup } from '@testing-library/react'
import { useState } from 'react'
import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query'
import type { SsoSession } from '@pdlc/ui/sso'
import { useAuthStore, ssoConfig } from './store'
import {
  acceptSpaceSession,
  observeSpacePolicy,
  isCurrentSpaceActor,
  useSpaceSession,
} from './space-session'
import { SpaceSessionBoundary } from './space-session-boundary'

const clients: QueryClient[] = []
function login(subject = 'central-a', local = 'local-a', token = 'fixture-a', admin = false) {
  const session: SsoSession = {
    accessToken: token,
    expiresAt: Date.now() + 60_000,
    subject,
    email: 'fixture@example.test',
    name: 'Fixture',
    returnTo: '/',
  }
  useAuthStore.getState().setAuth({ token, userId: local, email: session.email })
  acceptSpaceSession(session, { id: local, is_system_admin: admin })
  return session
}
function Draft() {
  const session = useSpaceSession((state) => state.session)
  const [draft, setDraft] = useState('')
  return (
    <>
      <output data-testid="actor">{session?.identity.subject ?? 'anonymous'}</output>
      <input aria-label="Draft" value={draft} onChange={(event) => setDraft(event.target.value)} />
    </>
  )
}
function mount(children = <Draft />, enabled = true) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } })
  clients.push(client)
  const view = render(
    <QueryClientProvider client={client}>
      <SpaceSessionBoundary enabled={enabled}>{children}</SpaceSessionBoundary>
    </QueryClientProvider>,
  )
  return { ...view, client }
}
beforeEach(() => {
  useAuthStore.getState().logout()
  useSpaceSession.setState({ session: null })
})
afterEach(() => {
  cleanup()
  clients.splice(0).forEach((client) => client.clear())
  vi.useRealTimers()
})
describe('central/local identity and cache boundary', () => {
  it('keeps central subject separate from the existing local UUID and never persists it', () => {
    login()
    expect(useSpaceSession.getState().session).toMatchObject({
      identity: { issuer: ssoConfig.issuer, subject: 'central-a' },
      userId: 'local-a',
    })
    expect(localStorage.getItem('task-tracker-auth')).not.toContain('central-a')
    expect(localStorage.getItem('task-tracker-auth')).not.toContain('fixture-a')
  })
  it('clears the proof on logout or a different local profile', () => {
    login()
    useAuthStore.getState().logout()
    expect(useSpaceSession.getState().session).toBeNull()
    login()
    useAuthStore.getState().setUser({ userId: 'local-b' })
    expect(useSpaceSession.getState().session).toBeNull()
  })
  it('rejects a mismatched or already expired SSO application', () => {
    const sso = login()
    acceptSpaceSession({ ...sso, accessToken: 'different-token' }, { id: 'local-a' })
    expect(useSpaceSession.getState().session).toBeNull()
    acceptSpaceSession({ ...sso, expiresAt: Date.now() - 1 }, { id: 'local-a' })
    expect(useSpaceSession.getState().session).toBeNull()
  })
  it('preserves the draft and cache during a token refresh for the same identity', () => {
    login()
    const { client } = mount()
    client.setQueryData(['private-data'], 'A')
    fireEvent.change(screen.getByLabelText('Draft'), { target: { value: 'unsaved' } })
    act(() => {
      login('central-a', 'local-a', 'refreshed')
    })
    expect(screen.getByLabelText('Draft')).toHaveValue('unsaved')
    expect(client.getQueryData(['private-data'])).toBe('A')
  })
  it('clears incompatible cache and drafts on actor or confirmed role change', () => {
    login()
    const { client } = mount()
    client.setQueryData(['private-data'], 'A')
    fireEvent.change(screen.getByLabelText('Draft'), { target: { value: 'unsaved' } })
    act(() => {
      login('central-b', 'local-b', 'fixture-b')
    })
    expect(screen.getByTestId('actor')).toHaveTextContent('central-b')
    expect(client.getQueryData(['private-data'])).toBeUndefined()
    expect(screen.getByLabelText('Draft')).toHaveValue('')
    client.setQueryData(['private-data'], 'B')
    fireEvent.change(screen.getByLabelText('Draft'), { target: { value: 'another draft' } })
    act(() => {
      login('central-b', 'local-b', 'admin-token', true)
    })
    expect(client.getQueryData(['private-data'])).toBeUndefined()
    expect(screen.getByLabelText('Draft')).toHaveValue('')
  })
  it('does not restore an old request into the new actor cache when cancellation is ignored', async () => {
    login()
    let resolveOld!: (v: string) => void
    const read = vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise<string>((resolve) => {
            resolveOld = resolve
          }),
      )
      .mockResolvedValue('B')
    function Read() {
      const query = useQuery({ queryKey: ['private-data'], queryFn: read })
      return <output data-testid="value">{query.data ?? 'pending'}</output>
    }
    const { client } = mount(<Read />)
    await waitFor(() => expect(read).toHaveBeenCalledTimes(1))
    act(() => {
      login('central-b', 'local-b', 'fixture-b')
    })
    await screen.findByText('B')
    await act(async () => {
      resolveOld('A')
    })
    expect(screen.getByTestId('value')).toHaveTextContent('B')
    expect(client.getQueryData(['private-data'])).toBe('B')
  })
  it('expires the in-memory context at the original token deadline', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-10-20T00:00:00Z'))
    login()
    mount()
    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_001)
    })
    expect(screen.getByTestId('actor')).toHaveTextContent('anonymous')
    expect(useSpaceSession.getState().session).toBeNull()
  })
  it('leaves legacy cache and draft behavior behind the disabled cutover flag', () => {
    login()
    const { client } = mount(<Draft />, false)
    client.setQueryData(['legacy'], 'A')
    fireEvent.change(screen.getByLabelText('Draft'), { target: { value: 'legacy draft' } })
    act(() => {
      login('central-b', 'local-b', 'fixture-b')
    })
    expect(screen.getByLabelText('Draft')).toHaveValue('legacy draft')
    expect(client.getQueryData(['legacy'])).toBe('A')
  })
})

describe('monotonic policy revision', () => {
  it('clears unversioned cache at first confirmation and again after a policy change', () => {
    login()
    const { client } = mount()
    const identity = useSpaceSession.getState().session!.identity
    client.setQueryData(['private-data'], 'A')
    fireEvent.change(screen.getByLabelText('Draft'), { target: { value: 'draft' } })
    act(() => observeSpacePolicy(identity, 1))
    expect(client.getQueryData(['private-data'])).toBeUndefined()
    expect(screen.getByLabelText('Draft')).toHaveValue('')
    client.setQueryData(['private-data'], 'current-policy-data')
    fireEvent.change(screen.getByLabelText('Draft'), { target: { value: 'new draft' } })
    act(() => observeSpacePolicy(identity, 2))
    expect(client.getQueryData(['private-data'])).toBeUndefined()
    expect(screen.getByLabelText('Draft')).toHaveValue('')
    expect(useSpaceSession.getState().session?.policyRevision).toBe(2)
    expect(useSpaceSession.getState().session?.policyEpoch).toBe(2)
  })
  it('never accepts a lower revision or a different actor and preserves the revision on token refresh', () => {
    login()
    const identity = useSpaceSession.getState().session!.identity
    observeSpacePolicy(identity, 4)
    observeSpacePolicy(identity, 1)
    observeSpacePolicy({ ...identity, subject: 'other' }, 9)
    login('central-a', 'local-a', 'refresh')
    expect(useSpaceSession.getState().session).toMatchObject({ policyRevision: 4, policyEpoch: 1 })
  })
})

it('fences delayed commands by actor/role/policy while permitting a fresh token for the same context', () => {
  login()
  const original = useSpaceSession.getState().session!
  expect(isCurrentSpaceActor(original)).toBe(true)
  login('central-a', 'local-a', 'refresh')
  expect(isCurrentSpaceActor(original)).toBe(true)
  observeSpacePolicy(original.identity, 1)
  expect(isCurrentSpaceActor(original)).toBe(false)
  const current = useSpaceSession.getState().session!
  login('central-b', 'local-b', 'fixture-b')
  expect(isCurrentSpaceActor(current)).toBe(false)
})
