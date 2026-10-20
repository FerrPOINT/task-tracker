import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { createMemoryRouter, RouterProvider } from 'react-router'
import { ThemeProvider } from '@pdlc/ui/lib'
import { AppShell } from './app-shell'
import { SpaceSessionBoundary } from '@/shared/auth/space-session-boundary'
import { acceptSpaceSession, useSpaceSession } from '@/shared/auth/space-session'
import { ssoConfig, useAuthStore } from '@/shared/auth/store'
import { SpaceRequestError, type VisibleSpaceCatalog } from '@/api/space-catalog'
import type { VisibleProjectCatalog, VisibleProjectQuery } from '@/api/project-catalog'

const calls = vi.hoisted(() => ({ spaces: vi.fn(), projects: vi.fn() }))
vi.mock('@/api/space-catalog', async (original) => ({
  ...(await original<typeof import('@/api/space-catalog')>()),
  getVisibleSpaceCatalog: calls.spaces,
}))
vi.mock('@/api/project-catalog', async (original) => ({
  ...(await original<typeof import('@/api/project-catalog')>()),
  getVisibleProjectCatalog: calls.projects,
}))
vi.mock('@/shared/api/useTrackerEvents', () => ({ useTrackerEvents: () => {} }))
vi.mock('@/shared/api/hooks', () => ({
  useProjects: () => ({ data: [] }),
  useIssue: () => ({ data: undefined }),
  useCurrentUser: () => ({ data: { id: 'local-a', display_name: 'A', is_system_admin: false } }),
  useNotifications: () => ({ data: { notifications: [], unread_count: 0 } }),
  useMarkNotificationRead: () => ({ mutate: vi.fn() }),
  useMarkAllNotificationsRead: () => ({ mutate: vi.fn() }),
  useLogout: () => ({ mutate: vi.fn() }),
}))
const registry = '00000000-0000-4000-8000-000000000001'
const space = { registry_instance_id: registry, space_id: '00000000-0000-4000-8000-000000000002' }
const identity = { issuer: ssoConfig.issuer, subject: 'central-a' }
const clients: QueryClient[] = []
function validity() {
  const now = Math.floor(Date.now() / 1000)
  return { observed_at: now, valid_until: now + 30 }
}
function mount(initial: string) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } })
  clients.push(client)
  const router = createMemoryRouter(
    [
      {
        element: <AppShell />,
        children: [
          { path: '/projects/QA/board', element: <h1>Verified project body</h1> },
          {
            path: '/admin',
            handle: { spaceAdmission: 'independent' },
            element: <h1>Global admin body</h1>,
          },
        ],
      },
    ],
    { initialEntries: [initial] },
  )
  return render(
    <QueryClientProvider client={client}>
      <ThemeProvider>
        <SpaceSessionBoundary enabled>
          <RouterProvider router={router} />
        </SpaceSessionBoundary>
      </ThemeProvider>
    </QueryClientProvider>,
  )
}
beforeEach(() => {
  vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'true')
  vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
  calls.spaces.mockReset()
  calls.projects.mockReset()
  useAuthStore.getState().logout()
  useAuthStore
    .getState()
    .setAuth({ token: 'fixture-a', userId: 'local-a', email: 'a@example.test' })
  acceptSpaceSession(
    {
      accessToken: 'fixture-a',
      subject: identity.subject,
      email: 'a@example.test',
      name: 'A',
      returnTo: '/',
      expiresAt: Date.now() + 60_000,
    },
    { id: 'local-a' },
  )
  calls.spaces.mockImplementation(
    async () =>
      ({
        schema_version: 2,
        identity,
        policy_revision: 1,
        validity: validity(),
        limit: 10,
        offset: 0,
        total: 1,
        has_more: false,
        spaces: [
          {
            space,
            revision: 1,
            state: 'active',
            metadata: {
              slug: 'a',
              name: 'A',
              description: '',
              responsible_subject: identity.subject,
            },
          },
        ],
      }) satisfies VisibleSpaceCatalog,
  )
  calls.projects.mockImplementation(
    async (_identity, query: VisibleProjectQuery) =>
      ({
        schema_version: 2,
        tracker_instance_id: registry,
        identity,
        policy_revision: 1,
        validity: validity(),
        space: query.space ?? null,
        total: 1,
        limit: query.limit ?? 50,
        offset: 0,
        has_more: false,
        projects: [
          {
            id: '00000000-0000-4000-8000-000000000003',
            key: 'QA',
            name: 'Project',
            owner_id: registry,
            owner_name: 'Owner',
            todo_count: 0,
            in_progress_count: 0,
            done_count: 0,
          },
        ],
      }) satisfies VisibleProjectCatalog,
  )
})
afterEach(() => {
  cleanup()
  clients.splice(0).forEach((c) => c.clear())
  vi.unstubAllEnvs()
  vi.restoreAllMocks()
  useSpaceSession.setState({ session: null })
})
it('serves the new paired context with legacy namespace mode also configured, without a legacy reader request', async () => {
  const fetch = vi.spyOn(globalThis, 'fetch')
  mount('/projects/QA/board?' + new URLSearchParams(space))
  expect(await screen.findByRole('heading', { name: 'Verified project body' })).toBeInTheDocument()
  await waitFor(() => expect(calls.projects).toHaveBeenCalled())
  expect(screen.queryByText('Некорректная ссылка на проект.')).not.toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'Пространство: A' })).toBeInTheDocument()
  expect(fetch).not.toHaveBeenCalled()
})
it('does not apply project catalog outage as the authorization rule for the global admin route', async () => {
  calls.projects.mockRejectedValue(new SpaceRequestError(503))
  calls.spaces.mockRejectedValue(new SpaceRequestError(503))
  mount('/admin')
  expect(await screen.findByRole('heading', { name: 'Global admin body' })).toBeInTheDocument()
})
