import { act, render } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { describe, expect, it, vi, afterEach } from 'vitest'
import { useTrackerEvents } from './useTrackerEvents'
import { useAuthStore } from '@/shared/auth/store'

let lastUrl = ''

class FakeEventSource {
  static latest: FakeEventSource | undefined
  private listeners = new Map<string, (event: MessageEvent) => void>()

  constructor(url: string) {
    // The stream URL carries the access token; keep it for assertions.
    lastUrl = url
    FakeEventSource.latest = this
  }

  addEventListener(name: string, callback: (event: MessageEvent) => void) {
    this.listeners.set(name, callback)
  }

  close() {}

  emit(type: string, payload: object) {
    this.listeners.get('tracker')?.({ data: JSON.stringify({ type, ...payload }) } as MessageEvent)
  }
}

function Subscriber() {
  useTrackerEvents()
  return null
}

describe('useTrackerEvents', () => {
  afterEach(() => {
    useAuthStore.getState().logout()
    FakeEventSource.latest = undefined
    lastUrl = ''
    vi.unstubAllGlobals()
  })

  it('subscribes with the access token in the stream URL', () => {
    vi.stubGlobal('EventSource', FakeEventSource)
    useAuthStore.setState({ token: 'test.token+/=' })
    const client = new QueryClient()
    render(
      <QueryClientProvider client={client}>
        <Subscriber />
      </QueryClientProvider>,
    )
    expect(lastUrl).toBe('/api/v1/events?access_token=test.token%2B%2F%3D')
  })

  it('invalidates worklogs and issue detail when a worklog SSE event arrives', () => {
    vi.stubGlobal('EventSource', FakeEventSource)
    useAuthStore.setState({ token: 'test-token' })
    const client = new QueryClient()
    const invalidate = vi.spyOn(client, 'invalidateQueries')

    render(
      <QueryClientProvider client={client}>
        <Subscriber />
      </QueryClientProvider>,
    )

    act(() => {
      FakeEventSource.latest?.emit('worklog_logged', { issue_id: 'issue-1', project_key: 'TT' })
    })

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['worklogs', 'issue-1'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue', 'issue-1'] })
  })

  it('invalidates issue collection caches when an issue SSE event arrives', () => {
    vi.stubGlobal('EventSource', FakeEventSource)
    useAuthStore.setState({ token: 'test-token' })
    const client = new QueryClient()
    const invalidate = vi.spyOn(client, 'invalidateQueries')

    render(
      <QueryClientProvider client={client}>
        <Subscriber />
      </QueryClientProvider>,
    )

    act(() => {
      FakeEventSource.latest?.emit('issue_moved', { issue_id: 'issue-1', project_key: 'TT' })
    })

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['projects'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['dashboard'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['search'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['project', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['backlog', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue', 'issue-1'] })
  })
})
