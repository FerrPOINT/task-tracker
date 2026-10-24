import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter, useLocation } from 'react-router'
import { SpaceContextBoundary, SpaceShellContext } from './space-context'
import { SpaceSessionBoundary } from '@/shared/auth/space-session-boundary'
import { acceptSpaceSession, useSpaceSession } from '@/shared/auth/space-session'
import { useAuthStore, ssoConfig } from '@/shared/auth/store'
import { SpaceRequestError, type VisibleSpaceCatalog } from '@/api/space-catalog'
import type { VisibleProjectCatalog, VisibleProjectQuery } from '@/api/project-catalog'

const read = vi.hoisted(() => ({ spaces: vi.fn(), projects: vi.fn() }))
vi.mock('@/api/space-catalog', async (original) => ({
  ...(await original<typeof import('@/api/space-catalog')>()),
  getVisibleSpaceCatalog: read.spaces,
}))
vi.mock('@/api/project-catalog', async (original) => ({
  ...(await original<typeof import('@/api/project-catalog')>()),
  getVisibleProjectCatalog: read.projects,
}))
const registry = '00000000-0000-4000-8000-000000000001'
const a = { registry_instance_id: registry, space_id: '00000000-0000-4000-8000-000000000002' }
const b = { ...a, space_id: '00000000-0000-4000-8000-000000000003' }
const identity = { issuer: ssoConfig.issuer, subject: 'central-a' }
const clients: QueryClient[] = []
function validity() {
  const now = Math.floor(Date.now() / 1000)
  return { observed_at: now, valid_until: now + 30 }
}
function spaces(): VisibleSpaceCatalog {
  return {
    schema_version: 2,
    identity,
    policy_revision: 1,
    validity: validity(),
    limit: 10,
    offset: 0,
    total: 2,
    has_more: false,
    spaces: [a, b].map((space, i) => ({
      space,
      revision: 1,
      state: 'active',
      metadata: {
        slug: i ? 'b' : 'a',
        name: 'Одинаковое',
        description: '',
        responsible_subject: identity.subject,
      },
    })),
  }
}
function projects(query: VisibleProjectQuery): VisibleProjectCatalog {
  const all =
    query.space?.space_id === b.space_id
      ? []
      : [
          {
            id: '00000000-0000-4000-8000-000000000004',
            key: 'QA',
            name: 'Проект',
            owner_id: '00000000-0000-4000-8000-000000000005',
            owner_name: 'Owner',
            todo_count: 0,
            in_progress_count: 0,
            done_count: 0,
          },
        ]
  const selected = query.projectKey
    ? all.filter((project) => project.key === query.projectKey)
    : all
  const limit = query.limit ?? 50,
    offset = query.offset ?? 0
  return {
    schema_version: 2,
    tracker_instance_id: registry,
    identity,
    policy_revision: 1,
    validity: validity(),
    space: query.space ?? null,
    total: selected.length,
    limit,
    offset,
    has_more: offset + limit < selected.length,
    projects: selected.slice(offset, offset + limit),
  }
}
function Location() {
  const location = useLocation()
  return <output data-testid="location">{location.pathname + location.search}</output>
}
function mount({
  initial = '/projects',
  header = false,
  projectKey,
  independent = false,
}: {
  initial?: string
  header?: boolean
  projectKey?: string
  independent?: boolean
} = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } })
  clients.push(client)
  return render(
    <MemoryRouter initialEntries={[initial]}>
      <QueryClientProvider client={client}>
        <SpaceSessionBoundary enabled>
          {header ? (
            <SpaceShellContext projectKey={projectKey} />
          ) : (
            <SpaceContextBoundary independent={independent} projectKey={projectKey}>
              <div data-testid="protected">Protected content</div>
            </SpaceContextBoundary>
          )}
          <Location />
        </SpaceSessionBoundary>
      </QueryClientProvider>
    </MemoryRouter>,
  )
}
beforeEach(() => {
  vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'true')
  Object.values(read).forEach((f) => f.mockReset())
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
  read.spaces.mockImplementation(async () => spaces())
  read.projects.mockImplementation(async (_identity, query) => projects(query))
})
afterEach(() => {
  cleanup()
  clients.splice(0).forEach((c) => c.clear())
  vi.unstubAllEnvs()
  useSpaceSession.setState({ session: null })
})
describe('separate explicit space and project context', () => {
  it('does not auto-select a space or project and preserves equal-name IDs when choosing', async () => {
    mount({ header: true })
    const user = userEvent.setup()
    const chooser = await screen.findByRole('button', {
      name: 'Пространство: Все доступные пространства',
    })
    await waitFor(() => expect(read.spaces).toHaveBeenCalled())
    await user.click(chooser)
    await user.click(await screen.findByRole('button', { name: new RegExp(a.space_id) }))
    expect(screen.getByTestId('location')).toHaveTextContent('/projects?' + new URLSearchParams(a))
    expect(screen.getByTestId('location')).not.toHaveTextContent('/board')
    await user.click(screen.getByRole('button', { name: 'Проект: Все проекты пространства' }))
    await user.click(await screen.findByRole('button', { name: 'Проект · QA' }))
    expect(screen.getByTestId('location')).toHaveTextContent(
      '/projects/QA/board?' + new URLSearchParams(a),
    )
  })
  it('allows a selected space with zero projects without selecting another one', async () => {
    mount({ header: true, initial: '/projects?' + new URLSearchParams(b) })
    await waitFor(() => expect(read.projects).toHaveBeenCalled())
    expect(screen.getByTestId('location')).toHaveTextContent('/projects?' + new URLSearchParams(b))
    expect(screen.queryByText('Проект · QA')).not.toBeInTheDocument()
  })
  it('rejects an ambiguous legacy/duplicate context before protected reads', async () => {
    mount({ initial: '/projects?' + new URLSearchParams(a) + '&space_id=' + a.space_id })
    expect(
      await screen.findByText(/Контекст пространства или проекта неоднозначен/),
    ).toBeInTheDocument()
    expect(read.projects).not.toHaveBeenCalled()
    expect(screen.queryByTestId('protected')).not.toBeInTheDocument()
  })
  it('checks the exact project within the explicit space without redirecting to another result', async () => {
    mount({ initial: '/projects/QA/board?' + new URLSearchParams(b), projectKey: 'QA' })
    expect(
      await screen.findByText('Проект недоступен в выбранном пространстве.'),
    ).toBeInTheDocument()
    expect(read.projects).toHaveBeenCalledWith(
      identity,
      expect.objectContaining({ space: b, projectKey: 'QA', limit: 1 }),
      expect.any(AbortSignal),
    )
    expect(screen.queryByTestId('protected')).not.toBeInTheDocument()
    expect(screen.getByTestId('location')).toHaveTextContent('/projects/QA/board')
  })
  it('keeps global administrative resources independent of the space catalog', async () => {
    read.projects.mockRejectedValue(new SpaceRequestError(503))
    mount({ initial: '/admin', independent: true })
    expect(await screen.findByTestId('protected')).toBeInTheDocument()
    expect(read.projects).not.toHaveBeenCalled()
  })
  it('closes the protected view on proof expiry while revalidation is unanswered', async () => {
    const now = Math.floor(Date.now() / 1000),
      until = now + 2
    read.projects.mockImplementation(async (_identity, query) => {
      if (Math.floor(Date.now() / 1000) >= until) return new Promise(() => {})
      return { ...projects(query), validity: { observed_at: now, valid_until: until } }
    })
    mount({ initial: '/projects/QA/board', projectKey: 'QA' })
    expect(await screen.findByTestId('protected')).toBeInTheDocument()
    await act(async () => {
      await new Promise((done) => setTimeout(done, 2100))
    })
    expect(screen.queryByTestId('protected')).not.toBeInTheDocument()
  })
})
