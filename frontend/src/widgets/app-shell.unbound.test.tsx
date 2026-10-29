import { applicationAccessFixture } from '@/test/application-access'
import { render, screen, within, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter, Route, Routes, useLocation } from 'react-router'
import { ThemeProvider } from '@pdlc/ui/lib'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { AppShell } from './app-shell'

const fixture = vi.hoisted(() => ({ key: 'LOCAL', name: 'Independent project' }))
vi.mock('@pdlc/ui/auth', async (original) => ({
  ...(await original<object>()),
  useApplicationAccess: () =>
    applicationAccessFixture({ role: 'admin', applications: ['task-tracker'] }),
}))
vi.mock('@pdlc/ui/ui', async (original) => ({
  ...(await original<object>()),
  usePlatformServices: () => ({ source: 'runtime', services: [] }),
}))
vi.mock('@/api/project', () => ({ getProject: async (key: string) => ({ key, id: 'local' }) }))
vi.mock('@/shared/api/hooks', () => ({
  useCurrentUser: () => ({ data: { email: 'operator@example.test' } }),
  useLogout: () => ({ mutate: vi.fn() }),
  useNotifications: () => ({ data: { notifications: [], unread_count: 0 } }),
  useMarkNotificationRead: () => ({ mutate: vi.fn() }),
  useMarkAllNotificationsRead: () => ({ mutate: vi.fn() }),
  useIssue: () => ({ data: undefined }),
  useProjects: () => ({
    data: [
      { id: 'local', key: fixture.key, name: fixture.name },
      { id: 'bound', key: 'BOUND', name: 'Bound project' },
      { id: 'old', key: 'OLD', name: 'Archived project' },
    ],
    isPending: false,
    isError: false,
  }),
}))
vi.mock('@/shared/api/useTrackerEvents', () => ({ useTrackerEvents: vi.fn() }))
const registry = '506a8476-3868-4581-af70-6d22c93ced2f'
const namespace = '0ee31fca-2533-44bf-b54e-a7759fe80b14'
const search = `?registry_instance_id=${registry}&namespace_id=${namespace}`
const contexts = [
  {
    label: 'Bound project',
    resource_key: 'BOUND',
    binding: {
      state: 'active',
      resource: { instance_id: registry, resource_id: 'bound' },
      namespace: { registry_instance_id: registry, namespace_id: namespace },
    },
  },
  {
    label: 'Archived project',
    resource_key: 'OLD',
    binding: {
      state: 'archived',
      resource: { instance_id: registry, resource_id: 'old' },
      namespace: {
        registry_instance_id: registry,
        namespace_id: 'a93e0210-d3f6-4936-b465-90da8312f871',
      },
    },
  },
]
function Location() {
  const location = useLocation()
  return <output aria-label="Current URL">{location.pathname + location.search}</output>
}
function mount(path: string) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  render(
    <QueryClientProvider client={client}>
      <ThemeProvider>
        <MemoryRouter initialEntries={[path]}>
          <Routes>
            <Route element={<AppShell />}>
              <Route path="*" element={<Location />} />
            </Route>
          </Routes>
        </MemoryRouter>
      </ThemeProvider>
    </QueryClientProvider>,
  )
}
beforeEach(() => {
  vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
  vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'false')
  fixture.key = 'LOCAL'
  fixture.name = 'Independent project'
  localStorage.clear()
  vi.stubGlobal(
    'fetch',
    vi.fn(
      async (input: string) =>
        new Response(
          JSON.stringify(
            new URL(input, 'http://tracker.test').pathname.endsWith(namespace)
              ? contexts[0]
              : contexts,
          ),
          { status: 200 },
        ),
    ),
  )
})
afterEach(() => {
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
})

it.each([
  ['LOCAL', 'Independent project'],
  ['CUSTOM', 'Независимый проект'],
])(
  'shows %s without a namespace in both navigation controls and clears a foreign context',
  async (key, name) => {
    fixture.key = key
    fixture.name = name
    const user = userEvent.setup()
    mount(`/projects/BOUND/board${search}&project_scope=all`)
    const navigation = await screen.findByRole('navigation', { name: 'Навигация проекта' })
    const standalone = await within(navigation).findByRole('region', { name: `${name} · ${key}` })
    expect(within(navigation).getAllByRole('region')).toHaveLength(2)
    expect(within(navigation).queryByText('Archived project')).not.toBeInTheDocument()
    const board = within(standalone).getByRole('link', { name: 'Доска' })
    expect(board).toHaveAttribute('href', `/projects/${key}/board?project_scope=all`)
    expect(
      within(standalone).getByRole('link', { name: `Настройки проекта: ${name} · ${key}` }),
    ).toHaveAttribute('href', `/projects/${key}/settings?project_scope=all`)
    await user.click(screen.getByRole('button', { name: 'Namespace' }))
    await user.click(await screen.findByRole('menuitemradio', { name: `${name} · ${key}` }))
    await waitFor(() =>
      expect(screen.getByLabelText('Current URL')).toHaveTextContent(`/projects/${key}/board`),
    )
    expect(screen.getByLabelText('Current URL').textContent).not.toContain('namespace_id')
  },
)

it('keeps an explicitly selected namespace scoped to its own project', async () => {
  mount(`/namespace${search}`)
  const nav = await screen.findByRole('navigation', { name: 'Навигация проекта' })
  expect(within(nav).getAllByRole('region')).toHaveLength(1)
  expect(within(nav).getByRole('region', { name: 'Bound project · BOUND' })).toBeInTheDocument()
})

it('shows projects with an empty namespace catalogue and selects standalone context without inventing a ref', async () => {
  vi.stubGlobal(
    'fetch',
    vi.fn(async () => new Response('[]', { status: 200 })),
  )
  mount('/projects/LOCAL/board')
  const nav = await screen.findByRole('navigation', { name: 'Навигация проекта' })
  expect(
    within(nav).getByRole('region', { name: 'Independent project · LOCAL' }),
  ).toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'Namespace' })).toHaveTextContent(
    'Independent project · LOCAL',
  )
})
