import { act, render } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { describe, expect, it, vi, afterEach } from 'vitest'
import { useTrackerEvents } from './useTrackerEvents'
import { useAuthStore } from '@/shared/auth/store'
import { connectAuthenticatedEventStream } from '@sdlc/ui/lib'

vi.mock('@sdlc/ui/lib', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@sdlc/ui/lib')>()),
  connectAuthenticatedEventStream: vi.fn(() => vi.fn()),
}))

function emit(type: string, payload: object) {
  const options = vi.mocked(connectAuthenticatedEventStream).mock.lastCall?.[0]
  options?.onEvent('tracker', { type, ...payload })
}

function Subscriber() {
  useTrackerEvents()
  return null
}

describe('useTrackerEvents', () => {
  afterEach(() => {
    useAuthStore.getState().logout()
    vi.mocked(connectAuthenticatedEventStream).mockClear()
    vi.unstubAllGlobals()
  })

  it('subscribes without exposing the access token in the stream URL', () => {
    useAuthStore.setState({ token: 'test.token+/=' })
    const client = new QueryClient()
    render(
      <QueryClientProvider client={client}>
        <Subscriber />
      </QueryClientProvider>,
    )
    expect(connectAuthenticatedEventStream).toHaveBeenCalledWith(
      expect.objectContaining({
        url: '/api/v1/events',
        token: 'test.token+/=',
      }),
    )
  })

  it('invalidates worklogs and issue detail when a worklog SSE event arrives', () => {
    useAuthStore.setState({ token: 'test-token' })
    const client = new QueryClient()
    const invalidate = vi.spyOn(client, 'invalidateQueries')

    render(
      <QueryClientProvider client={client}>
        <Subscriber />
      </QueryClientProvider>,
    )

    act(() => {
      emit('worklog_logged', { issue_id: 'issue-1', project_key: 'TT' })
    })

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['projects'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['dashboard'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['search'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['reports'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['project', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['backlog', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['worklogs', 'issue-1'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue', 'issue-1'] })
  })

  it('invalidates issue collection caches when an issue SSE event arrives', () => {
    useAuthStore.setState({ token: 'test-token' })
    const client = new QueryClient()
    const invalidate = vi.spyOn(client, 'invalidateQueries')

    render(
      <QueryClientProvider client={client}>
        <Subscriber />
      </QueryClientProvider>,
    )

    act(() => {
      emit('issue_moved', { issue_id: 'issue-1', project_key: 'TT' })
    })

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['projects'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['dashboard'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['search'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['reports'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['project', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['backlog', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue', 'issue-1'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue-labels', 'issue-1'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue-links', 'issue-1'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue-custom-fields', 'issue-1'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['attachments', 'issue-1'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue-votes', 'issue-1'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue-watchers', 'issue-1'] })
  })

  it('invalidates sprint-backed project caches when a sprint SSE event arrives', () => {
    useAuthStore.setState({ token: 'test-token' })
    const client = new QueryClient()
    const invalidate = vi.spyOn(client, 'invalidateQueries')

    render(
      <QueryClientProvider client={client}>
        <Subscriber />
      </QueryClientProvider>,
    )

    act(() => {
      emit('sprint_changed', { project_key: 'TT' })
    })

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['sprints'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['reports'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['project', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['backlog', 'TT'] })
  })
})
