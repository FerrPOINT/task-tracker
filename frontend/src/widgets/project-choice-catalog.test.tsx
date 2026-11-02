import { createElement, type ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'
import { renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useProjectChoiceCatalog } from './project-choice-catalog'

const projects = vi.hoisted(() => vi.fn())
const details = vi.hoisted(() => vi.fn())
const exact = vi.hoisted(() => vi.fn())
vi.mock('@/shared/api/hooks', () => ({ useProjects: projects, useProjectDetails: details }))
vi.mock('@/shared/api/useVisibleProjects', () => ({ useVisibleProjects: exact }))

const project = (index: number) => ({
  id: `id-${index}`,
  key: `P${index}`,
  name: `Project ${index}`,
})
const page = (count: number) => ({
  data: Array.from({ length: count }, (_, index) => project(index)),
  catalog: undefined,
  isPending: false,
  isError: false,
  error: null,
  refetch: vi.fn(),
})
function mount(selected = '', offset = 0, search = '', route = '/search') {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } })
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(
      QueryClientProvider,
      { client },
      createElement(MemoryRouter, { initialEntries: [route] }, children),
    )
  return renderHook(() => useProjectChoiceCatalog(selected, offset, search), { wrapper })
}
beforeEach(() => {
  vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'false')
  vi.stubEnv('VITE_NAMESPACE_ENABLED', 'false')
  projects.mockReturnValue(page(21))
  details.mockReturnValue({ data: undefined, isPending: false, isError: false, refetch: vi.fn() })
  exact.mockReturnValue({
    data: undefined,
    isPending: false,
    isError: false,
    unavailable: false,
    refetch: vi.fn(),
  })
})
afterEach(() => {
  vi.clearAllMocks()
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
})

describe('project choice catalog', () => {
  it('requests a bounded sentinel and presents only the actual page without a fabricated total', () => {
    const { result } = mount()
    expect(projects).toHaveBeenLastCalledWith(0, 21, '', true, { allPages: false })
    expect(result.current.data).toHaveLength(20)
    expect(result.current.data?.some((value) => value.key === 'P20')).toBe(false)
    expect(result.current.hasMore).toBe(true)
    expect(result.current.catalog).toBeUndefined()
    expect(result.current.searchable).toBe(false)
    expect(details).toHaveBeenCalledWith('')
  })

  it.each([0, 19, 20])('does not advertise another legacy page for %i confirmed rows', (count) => {
    projects.mockReturnValue(page(count))
    expect(mount().result.current.hasMore).toBe(false)
  })

  it('resolves a selected key outside the page without replacing it with the first option', () => {
    details.mockReturnValue({ data: project(350), isError: false, isPending: false })
    const { result } = mount('P350', 100)
    expect(details).toHaveBeenCalledWith('P350')
    expect(result.current.selected?.key).toBe('P350')
    expect(result.current.selectedUnavailable).toBe(false)
    expect(projects).toHaveBeenLastCalledWith(100, 21, '', true, { allPages: false })
  })

  it('does not display an old detail after access to that selected key fails', () => {
    details.mockReturnValue({ data: project(350), isError: true, isPending: false })
    const { result } = mount('P350')
    expect(result.current.selected).toBeUndefined()
    expect(result.current.selectedUnavailable).toBe(true)
    expect(result.current.data?.[0]?.key).toBe('P0')
  })

  it('slices the raw page before archive filtering so the sentinel is not repeated on two pages', async () => {
    vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
    vi.stubGlobal(
      'fetch',
      vi.fn(
        async () =>
          new Response(
            JSON.stringify([{ binding: { state: 'archived', resource: { resource_id: 'id-0' } } }]),
            { status: 200 },
          ),
      ),
    )
    details.mockReturnValue({ data: project(0), isError: false, isPending: false })
    const { result } = mount('P0')
    expect(result.current.data).toBeUndefined()
    await waitFor(() => expect(result.current.data).toHaveLength(19))
    expect(result.current.data?.some((value) => value.key === 'P20')).toBe(false)
    expect(result.current.hasMore).toBe(true)
    expect(result.current.selected).toBeUndefined()
    expect(result.current.selectedUnavailable).toBe(true)
  })

  it('retains registry admission errors instead of exposing an unconfirmed legacy project', async () => {
    vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('', { status: 503 })),
    )
    details.mockReturnValue({ data: project(350), isError: false, isPending: false })
    const { result } = mount('P350')
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(result.current.data).toBeUndefined()
    expect(result.current.selected).toBeUndefined()
    expect(result.current.selectedPending).toBe(false)
  })

  it('uses the v2 counted catalog and exact admitted lookup with server search', () => {
    vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'true')
    projects.mockReturnValue({ ...page(21), catalog: { total: 47 } })
    exact.mockReturnValue({
      data: { projects: [project(350)] },
      isError: false,
      isPending: false,
      unavailable: false,
    })
    const { result } = mount('P350', 20, 'team')
    expect(projects).toHaveBeenLastCalledWith(20, 21, 'team', true, { allPages: false })
    expect(exact).toHaveBeenLastCalledWith(
      expect.objectContaining({ projectKey: 'P350', limit: 1, workingOnly: true }),
      true,
    )
    expect(details).toHaveBeenCalledWith('')
    expect(result.current.selected?.key).toBe('P350')
    expect(result.current.hasMore).toBe(true)
    expect(result.current.searchable).toBe(true)
  })

  it('does not substitute a neighboring v2 result for an unavailable explicit key', () => {
    vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'true')
    exact.mockReturnValue({
      data: { projects: [project(351)] },
      isError: false,
      isPending: false,
      unavailable: false,
    })
    const { result } = mount('P350')
    expect(result.current.selected).toBeUndefined()
    expect(result.current.selectedUnavailable).toBe(true)
  })

  it('reports an inactive v2 exact read as unavailable instead of waiting for a disabled query', () => {
    vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'true')
    exact.mockReturnValue({ data: undefined, unavailable: true, isPending: true, isError: false })
    const { result } = mount('P350')
    expect(result.current.selected).toBeUndefined()
    expect(result.current.selectedPending).toBe(false)
    expect(result.current.selectedUnavailable).toBe(true)
  })

  it('keeps an active exact read pending while its current result is being loaded', () => {
    vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'true')
    exact.mockReturnValue({ data: undefined, unavailable: false, isPending: true, isError: false })
    const { result } = mount('P350')
    expect(result.current.selected).toBeUndefined()
    expect(result.current.selectedPending).toBe(true)
    expect(result.current.selectedUnavailable).toBe(false)
  })
})
