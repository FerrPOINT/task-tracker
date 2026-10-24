import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { MutationCache, QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { useScopedMutation, SpaceMutationScopeError } from './useScopedMutation'
import {
  acceptSpaceSession,
  observeSpaceContext,
  observeSpacePolicy,
  useSpaceSession,
} from '@/shared/auth/space-session'
import { useAuthStore } from '@/shared/auth/store'
const clients: QueryClient[] = []
function login(subject = 'a', token = subject) {
  useAuthStore
    .getState()
    .setAuth({ token, userId: 'local-' + subject, email: 'fixture@example.test' })
  acceptSpaceSession(
    {
      accessToken: token,
      subject: 'central-' + subject,
      email: 'fixture@example.test',
      name: subject,
      returnTo: '/',
      expiresAt: Date.now() + 60_000,
    },
    { id: 'local-' + subject },
  )
}
function client(mutationCache?: MutationCache) {
  const result = new QueryClient({
    mutationCache,
    defaultOptions: {
      queries: { retry: false, gcTime: 60_000 },
      mutations: { retry: false, gcTime: 0 },
    },
  })
  clients.push(result)
  return result
}
function wrapper(qc: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  )
}
function pending<T>() {
  let resolve: (value: T) => void = () => {},
    reject: (error: Error) => void = () => {}
  const promise = new Promise<T>((yes, no) => {
    resolve = yes
    reject = no
  })
  return { promise, resolve, reject }
}
beforeEach(() => {
  vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'true')
  useAuthStore.getState().logout()
  login()
})
afterEach(() => {
  cleanup()
  clients.splice(0).forEach((qc) => qc.clear())
  useSpaceSession.setState({ session: null })
  vi.unstubAllEnvs()
})
describe('mutation initiating context', () => {
  it('keeps raw payload, callbacks and result variables while the original context remains valid', async () => {
    const rpc = vi.fn(async (value: { name: string }) => ({ ...value, saved: true }))
    const onSuccess = vi.fn(),
      perCall = vi.fn(),
      qc = client()
    const { result } = renderHook(() => useScopedMutation({ mutationFn: rpc, onSuccess }), {
      wrapper: wrapper(qc),
    })
    act(() => result.current.mutate({ name: 'name' }, { onSuccess: perCall }))
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(rpc.mock.calls[0]![0]).toEqual({ name: 'name' })
    expect(result.current.variables).toEqual({ name: 'name' })
    expect(onSuccess.mock.calls[0]![1]).toEqual({ name: 'name' })
    expect(perCall.mock.calls[0]![1]).toEqual({ name: 'name' })
  })
  it('does not write an earlier successful response into a new actor cache or call either callback', async () => {
    const response = pending<string>(),
      rpc = vi.fn(() => response.promise),
      qc = client()
    const onSuccess = vi.fn((data: string) => qc.setQueryData(['private'], data)),
      perCall = vi.fn(),
      onSettled = vi.fn()
    const { result } = renderHook(
      () => useScopedMutation({ mutationFn: rpc, onSuccess, onSettled, retry: true }),
      { wrapper: wrapper(qc) },
    )
    act(() => result.current.mutate(undefined, { onSuccess: perCall }))
    await waitFor(() => expect(rpc).toHaveBeenCalledTimes(1))
    await act(async () => {
      login('b')
      qc.setQueryData(['private'], 'B data')
      response.resolve('A private response')
    })
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(result.current.error).toBeInstanceOf(SpaceMutationScopeError)
    expect(qc.getQueryData(['private'])).toBe('B data')
    expect(onSuccess).not.toHaveBeenCalled()
    expect(perCall).not.toHaveBeenCalled()
    expect(onSettled).not.toHaveBeenCalled()
    expect(rpc).toHaveBeenCalledTimes(1)
  })
  it('keeps the original actor while queued before dispatch, even when observer options rerender for B', async () => {
    const barrier = pending<void>(),
      rpc = vi.fn(async () => 'sent')
    const entered = vi.fn(() => barrier.promise)
    const qc = client(new MutationCache({ onMutate: entered }))
    const { result } = renderHook(() => useScopedMutation({ mutationFn: rpc }), {
      wrapper: wrapper(qc),
    })
    act(() => result.current.mutate(undefined))
    await waitFor(() => expect(entered).toHaveBeenCalled())
    await act(async () => {
      login('b')
      barrier.resolve()
    })
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(rpc).not.toHaveBeenCalled()
    expect(result.current.error).toBeInstanceOf(SpaceMutationScopeError)
  })
  it('does not expose an earlier server error or run rollback callbacks after a context change', async () => {
    const response = pending<string>(),
      onError = vi.fn(),
      perCall = vi.fn(),
      qc = client()
    const { result } = renderHook(
      () => useScopedMutation({ mutationFn: () => response.promise, onError }),
      { wrapper: wrapper(qc) },
    )
    act(() => result.current.mutate(undefined, { onError: perCall }))
    await waitFor(() => expect(result.current.isPending).toBe(true))
    await act(async () => {
      const identity = useSpaceSession.getState().session!.identity
      observeSpaceContext(identity, 'another-space')
      response.reject(new Error('A private server error'))
    })
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(result.current.error).toBeInstanceOf(SpaceMutationScopeError)
    expect(onError).not.toHaveBeenCalled()
    expect(perCall).not.toHaveBeenCalled()
  })
  it('preserves token refresh of the same initiating identity', async () => {
    const response = pending<string>(),
      onSuccess = vi.fn(),
      qc = client()
    const { result } = renderHook(
      () => useScopedMutation({ mutationFn: () => response.promise, onSuccess }),
      { wrapper: wrapper(qc) },
    )
    act(() => result.current.mutate(undefined))
    await waitFor(() => expect(result.current.isPending).toBe(true))
    await act(async () => {
      login('a', 'fresh-a')
      response.resolve('same actor')
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(onSuccess).toHaveBeenCalled()
  })
  it('rejects policy change while awaiting an onSuccess continuation', async () => {
    const barrier = pending<void>(),
      entered = vi.fn(() => barrier.promise),
      perCall = vi.fn(),
      qc = client()
    const { result } = renderHook(
      () => useScopedMutation({ mutationFn: async () => 'saved', onSuccess: entered }),
      { wrapper: wrapper(qc) },
    )
    act(() => result.current.mutate(undefined, { onSuccess: perCall }))
    await waitFor(() => expect(entered).toHaveBeenCalled())
    await act(async () => {
      observeSpacePolicy(useSpaceSession.getState().session!.identity, 2)
      barrier.resolve()
    })
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(result.current.data).toBeUndefined()
    expect(perCall).not.toHaveBeenCalled()
  })
  it('closes protected mutations without a central session', async () => {
    useAuthStore.getState().logout()
    const rpc = vi.fn(async () => 'sent'),
      qc = client()
    const { result } = renderHook(() => useScopedMutation({ mutationFn: rpc }), {
      wrapper: wrapper(qc),
    })
    act(() => result.current.mutate(undefined))
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(rpc).not.toHaveBeenCalled()
  })
  it('preserves the ordinary legacy mode without applying a space requirement', async () => {
    vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'false')
    useAuthStore.getState().logout()
    const rpc = vi.fn(async () => 'legacy'),
      qc = client()
    const { result } = renderHook(() => useScopedMutation({ mutationFn: rpc }), {
      wrapper: wrapper(qc),
    })
    act(() => result.current.mutate(undefined))
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toBe('legacy')
    expect(rpc).toHaveBeenCalledTimes(1)
  })
})
