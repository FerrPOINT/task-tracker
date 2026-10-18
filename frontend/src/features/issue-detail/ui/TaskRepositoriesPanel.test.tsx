import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'
import { TaskRepositoriesPanel } from './TaskRepositoriesPanel'
const mocks = vi.hoisted(() => ({ get: vi.fn() }))
vi.mock('@pdlc/ui/ui', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@pdlc/ui/ui')>()),
  usePlatformServices: () => ({ services: [] }),
}))
vi.mock('@/widgets/namespace-context', () => ({
  useNamespaceContext: () => ({
    ref: { registry_instance_id: 'registry', namespace_id: 'namespace' },
    query: {
      data: {
        resource_key: 'PROJECT',
        binding: { state: 'active', resource: { instance_id: 'tracker' } },
      },
    },
  }),
}))
vi.mock('@/api/client', () => ({ api: { GET: mocks.get } }))
afterEach(() => {
  cleanup()
  mocks.get.mockReset()
})
it.each(['first-task', 'second-task'])(
  'reads project links and retains evidence scope for %s',
  async (taskId) => {
    mocks.get.mockImplementation(async (path: string) => ({
      data: path.endsWith('/repositories')
        ? [
            {
              forge_instance_id: 'forge',
              repository_id: 'repo',
              public_name: 'group/project-repo',
              kind: 'hosted',
            },
          ]
        : [],
    }))
    render(
      <MemoryRouter>
        <QueryClientProvider
          client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
        >
          <TaskRepositoriesPanel taskId={taskId} projectKey="PROJECT" />
        </QueryClientProvider>
      </MemoryRouter>,
    )
    await screen.findByText('group/project-repo')
    expect(screen.getByRole('heading', { name: 'Репозитории проекта' })).toBeVisible()
    await waitFor(() =>
      expect(mocks.get).toHaveBeenCalledWith(
        '/api/v1/projects/{project_key}/repositories',
        expect.objectContaining({ params: { path: { project_key: 'PROJECT' } } }),
      ),
    )
    expect(mocks.get).toHaveBeenCalledWith(
      '/api/v1/tasks/{id}/delivery-evidence',
      expect.objectContaining({ params: { path: { id: taskId }, query: { offset: 0 } } }),
    )
    expect(mocks.get.mock.calls.some(([path]) => path === '/api/v1/tasks/{id}/repositories')).toBe(
      false,
    )
  },
)
