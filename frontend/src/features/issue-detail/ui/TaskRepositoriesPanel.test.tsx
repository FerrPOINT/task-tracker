import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'
import { TaskRepositoriesPanel } from './TaskRepositoriesPanel'
const mocks = vi.hoisted(() => ({ get: vi.fn(), projectKey: 'PROJECT' }))
vi.mock('@pdlc/ui/ui', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@pdlc/ui/ui')>()),
  usePlatformServices: () => ({ services: [] }),
}))
vi.mock('@/widgets/namespace-context', () => ({
  useNamespaceContext: () => ({
    ref: { registry_instance_id: 'registry', namespace_id: 'namespace' },
    query: {
      data: {
        resource_key: mocks.projectKey,
        binding: { state: 'active', resource: { instance_id: 'tracker' } },
      },
    },
  }),
}))
vi.mock('@/api/client', () => ({ api: { GET: mocks.get } }))
afterEach(() => {
  cleanup()
  mocks.get.mockReset()
  mocks.projectKey = 'PROJECT'
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
        expect.objectContaining({
          params: { path: { project_key: 'PROJECT' }, query: { offset: 0 } },
        }),
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

it('pages all project links and resets the page when the owning project changes', async () => {
  mocks.get.mockImplementation(
    async (
      path: string,
      options: {
        params: { path: { project_key?: string }; query?: { offset: number } }
      },
    ) => {
      if (path !== '/api/v1/projects/{project_key}/repositories') return { data: [] }
      const { project_key } = options.params.path
      const offset = options.params.query?.offset ?? 0
      return {
        data: Array.from(
          { length: project_key === 'OTHER' ? 1 : offset === 0 ? 100 : 2 },
          (_, index) => ({
            forge_instance_id: 'forge',
            repository_id: `${project_key}-${offset + index}`,
            public_name: `${project_key}/repository-${offset + index}`,
            kind: 'hosted',
          }),
        ),
      }
    },
  )
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const view = (projectKey: string) => (
    <MemoryRouter>
      <QueryClientProvider client={client}>
        <TaskRepositoriesPanel taskId={`${projectKey}-task`} projectKey={projectKey} />
      </QueryClientProvider>
    </MemoryRouter>
  )
  const { rerender } = render(view('PROJECT'))
  await screen.findByText('PROJECT/repository-99')
  fireEvent.click(screen.getByRole('button', { name: 'Следующие связи' }))
  await screen.findByText('PROJECT/repository-101')
  expect(screen.queryByText('PROJECT/repository-0')).not.toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'Следующие связи' })).toBeDisabled()
  fireEvent.click(screen.getByRole('button', { name: 'Предыдущие связи' }))
  await screen.findByText('PROJECT/repository-0')
  await waitFor(() => expect(screen.getByRole('button', { name: 'Следующие связи' })).toBeEnabled())
  fireEvent.click(screen.getByRole('button', { name: 'Следующие связи' }))
  await screen.findByText('PROJECT/repository-101')
  mocks.projectKey = 'OTHER'
  rerender(view('OTHER'))
  await screen.findByText('OTHER/repository-0')
  expect(
    mocks.get.mock.calls
      .filter(([path]) => path === '/api/v1/projects/{project_key}/repositories')
      .filter(([, options]) => options.params.path.project_key === 'OTHER')
      .every(([, options]) => options.params.query.offset === 0),
  ).toBe(true)
  expect(screen.queryByRole('button', { name: 'Следующие связи' })).not.toBeInTheDocument()
})

it('can return to the first page when loading a later page fails', async () => {
  mocks.get.mockImplementation(
    async (
      path: string,
      options: {
        params: { query?: { offset: number } }
      },
    ) => {
      if (path !== '/api/v1/projects/{project_key}/repositories') return { data: [] }
      if (options.params.query?.offset) return { error: { message: 'Unavailable' } }
      return {
        data: Array.from({ length: 100 }, (_, index) => ({
          forge_instance_id: 'forge',
          repository_id: `${index}`,
          public_name: `repo-${index}`,
          kind: 'hosted',
        })),
      }
    },
  )
  render(
    <MemoryRouter>
      <QueryClientProvider
        client={
          new QueryClient({
            defaultOptions: { queries: { retry: false } },
          })
        }
      >
        <TaskRepositoriesPanel taskId="task" projectKey="PROJECT" />
      </QueryClientProvider>
    </MemoryRouter>,
  )
  await screen.findByText('repo-99')
  fireEvent.click(screen.getByRole('button', { name: 'Следующие связи' }))
  await screen.findByRole('alert')
  fireEvent.click(screen.getByRole('button', { name: 'Предыдущие связи' }))
  await screen.findByText('repo-0')
})
