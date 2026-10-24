import { afterEach, expect, it, vi } from 'vitest'
import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { useUpdateProject } from './hooks'
import { SpaceMutationScopeError } from './useScopedMutation'
import { acceptSpaceSession, useSpaceSession } from '@/shared/auth/space-session'
import { useAuthStore } from '@/shared/auth/store'
import type { Project } from '@/api/project'
const update = vi.hoisted(() => vi.fn())
vi.mock('@/api/project', async (original) => ({
  ...(await original<typeof import('@/api/project')>()),
  updateProject: update,
}))
function login(subject: string) {
  useAuthStore
    .getState()
    .setAuth({ token: subject, userId: subject, email: 'fixture@example.test' })
  acceptSpaceSession(
    {
      accessToken: subject,
      subject,
      email: 'fixture@example.test',
      name: subject,
      returnTo: '/',
      expiresAt: Date.now() + 60_000,
    },
    { id: subject },
  )
}
afterEach(() => {
  cleanup()
  vi.unstubAllEnvs()
  useAuthStore.getState().logout()
  useSpaceSession.setState({ session: null })
})
it('the actual update-project hook cannot overwrite a replacement actor project cache', async () => {
  vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'true')
  login('A')
  const qc = new QueryClient({ defaultOptions: { mutations: { retry: false, gcTime: 0 } } })
  let complete: (value: Project) => void = () => {}
  update.mockImplementation(
    () =>
      new Promise<Project>((resolve) => {
        complete = resolve
      }),
  )
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  )
  const { result } = renderHook(() => useUpdateProject('QA'), { wrapper })
  act(() => result.current.mutate({ name: 'A title' }))
  await waitFor(() => expect(update).toHaveBeenCalledTimes(1))
  await act(async () => {
    login('B')
    void qc
      .fetchQuery<Project>({
        queryKey: ['project-info', 'QA'],
        queryFn: () => new Promise(() => {}),
        retry: false,
      })
      .catch(() => undefined)
    complete({
      id: 'A project',
      key: 'QA',
      name: 'A title',
      owner_id: 'A',
      owner_name: 'A',
      todo_count: 0,
      in_progress_count: 0,
      done_count: 0,
    })
  })
  await waitFor(() => expect(result.current.error).toBeInstanceOf(SpaceMutationScopeError))
  expect(qc.getQueryData(['project-info', 'QA'])).toBeUndefined()
  expect(qc.getQueryState(['project-info', 'QA'])?.status).toBe('pending')
  expect(update.mock.calls[0]).toEqual(['QA', { name: 'A title' }])
  qc.clear()
})
