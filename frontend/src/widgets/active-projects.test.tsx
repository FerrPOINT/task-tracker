import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { createElement, type ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useActiveProjects } from './active-projects'

vi.mock('@/shared/api/hooks', () => ({
  useProjects: () => ({
    data: [
      { id: 'one', key: 'ARCHIVED', name: 'Archived by name only' },
      { id: 'two', key: 'ACTIVE', name: 'Active by name only' },
      { id: 'local', key: 'LOCAL', name: 'Standalone' },
    ],
    isLoading: false,
    isPending: false,
    isError: false,
    error: null,
    refetch: vi.fn(),
  }),
}))
function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client }, children)
  return renderHook(() => useActiveProjects(), { wrapper })
}
afterEach(() => {
  vi.unstubAllGlobals()
  vi.unstubAllEnvs()
})
describe('working project list', () => {
  it.each(['one', 'two'])(
    'filters archived resource %s by binding identity, preserves standalone and observes restore',
    async (archived) => {
      vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
      let state = 'archived'
      vi.stubGlobal(
        'fetch',
        vi.fn(
          async () =>
            new Response(
              JSON.stringify([
                {
                  binding: { state, resource: { resource_id: archived } },
                  label: 'Changed name',
                  resource_key: 'OTHER',
                },
              ]),
              { status: 200 },
            ),
        ),
      )
      const { result } = mount()
      expect(result.current.data).toBeUndefined()
      await waitFor(() => expect(result.current.data).toHaveLength(2))
      expect(result.current.data?.map((p) => p.id)).not.toContain(archived)
      expect(result.current.data?.map((p) => p.id)).toContain('local')
      expect(result.current.unfilteredCount).toBe(3)
      state = 'active'
      await act(() => result.current.refetch())
      await waitFor(() => expect(result.current.data).toHaveLength(3))
    },
  )
  it('preserves standalone mode without consulting the registry', () => {
    vi.stubEnv('VITE_NAMESPACE_ENABLED', 'false')
    const fetch = vi.fn()
    vi.stubGlobal('fetch', fetch)
    const { result } = mount()
    expect(result.current.data).toHaveLength(3)
    expect(fetch).not.toHaveBeenCalled()
  })
  it('shows catalog failure instead of briefly displaying an archived project', async () => {
    vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('', { status: 503 })),
    )
    const { result } = mount()
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(result.current.data).toBeUndefined()
  })
})
