import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'
import type { ReactNode } from 'react'
import { afterEach, expect, it, vi } from 'vitest'
import { useCreateProject, useProjects } from './hooks'
import { createProject, listProjects, type Project } from '@/api/project'

vi.mock('@/api/project', async (original) => ({
  ...(await original<object>()),
  listProjects: vi.fn(),
  createProject: vi.fn(),
}))
afterEach(() => {
  vi.clearAllMocks()
  vi.unstubAllEnvs()
})
function project(id: number): Project {
  return {
    id: String(id),
    key: `P${id}`,
    name: `Project ${id}`,
    owner_id: 'owner',
    owner_name: 'Owner',
    description: null,
    todo_count: 0,
    in_progress_count: 0,
    done_count: 0,
  }
}
function wrapper(client: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>
      <MemoryRouter>{children}</MemoryRouter>
    </QueryClientProvider>
  )
}
it('loads all navigation pages and observes a newly created project through normal invalidation', async () => {
  vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'false')
  let projects = Array.from({ length: 201 }, (_, id) => project(id))
  vi.mocked(listProjects).mockImplementation(async (offset = 0, limit = 100) =>
    projects.slice(offset, offset + limit),
  )
  vi.mocked(createProject).mockImplementation(async () => {
    const saved = project(999)
    projects = [saved, ...projects]
    return saved
  })
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const { result } = renderHook(
    () => ({
      catalog: useProjects(0, 100, '', false, { allPages: true }),
      create: useCreateProject({ navigateOnSuccess: false }),
    }),
    { wrapper: wrapper(client) },
  )
  await waitFor(() => expect(result.current.catalog.data).toHaveLength(201))
  expect(vi.mocked(listProjects).mock.calls.map(([offset]) => offset)).toEqual([0, 100, 200])
  await act(async () => {
    await result.current.create.mutateAsync({ key: 'P999', name: 'New project' })
  })
  await waitFor(() => expect(result.current.catalog.data?.[0]?.id).toBe('999'))
  expect(result.current.catalog.data).toHaveLength(202)
})
it('keeps ordinary paginated project reads bounded', async () => {
  vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'false')
  vi.mocked(listProjects).mockResolvedValue(Array.from({ length: 100 }, (_, id) => project(id)))
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const { result } = renderHook(() => useProjects(), { wrapper: wrapper(client) })
  await waitFor(() => expect(result.current.data).toHaveLength(100))
  expect(listProjects).toHaveBeenCalledTimes(1)
})
