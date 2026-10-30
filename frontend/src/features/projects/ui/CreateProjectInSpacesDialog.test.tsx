import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router'
import type { Project } from '@/api/project'
import { SpaceRequestError, type VisibleSpaceCatalog } from '@/api/space-catalog'
import { acceptSpaceSession, useSpaceSession } from '@/shared/auth/space-session'
import { SpaceSessionBoundary } from '@/shared/auth/space-session-boundary'
import { ssoConfig, useAuthStore } from '@/shared/auth/store'
import { CreateProjectInSpacesDialog } from './CreateProjectInSpacesDialog'

const api = vi.hoisted(() => ({ create: vi.fn(), lookup: vi.fn(), catalog: vi.fn() }))
vi.mock('@/api/project-spaces', async (original) => ({
  ...(await original<typeof import('@/api/project-spaces')>()),
  createProjectInSpaces: api.create,
}))
vi.mock('@/api/project', async (original) => ({
  ...(await original<typeof import('@/api/project')>()),
  getProject: api.lookup,
}))
vi.mock('@/api/space-catalog', async (original) => ({
  ...(await original<typeof import('@/api/space-catalog')>()),
  getVisibleSpaceCatalog: api.catalog,
}))
const registry = '00000000-0000-4000-8000-000000000001'
const a = { registry_instance_id: registry, space_id: '00000000-0000-4000-8000-000000000002' }
const b = { ...a, space_id: '00000000-0000-4000-8000-000000000003' }
const identity = { issuer: ssoConfig.issuer, subject: 'central-a' }
const key =
  'pdlc:command:tracker:create-project-v2:' + JSON.stringify([identity.issuer, identity.subject])
const project: Project = {
  id: '00000000-0000-4000-8000-000000000004',
  key: '-',
  name: 'Проект',
  owner_id: 'local-a',
  owner_name: 'Fixture',
  todo_count: 0,
  in_progress_count: 0,
  done_count: 0,
}
const clients: QueryClient[] = []
function catalog(spaces = [a, b]): VisibleSpaceCatalog {
  const now = Math.floor(Date.now() / 1000)
  return {
    schema_version: 2,
    identity,
    policy_revision: 1,
    validity: { observed_at: now, valid_until: now + 30 },
    total: spaces.length,
    limit: 10,
    offset: 0,
    has_more: false,
    spaces: spaces.map((space, i) => ({
      space,
      revision: 1,
      state: 'active',
      metadata: {
        slug: 's' + i,
        name: i ? 'B' : 'A',
        description: '',
        responsible_subject: identity.subject,
      },
    })),
  }
}
function mount(onCreated = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } })
  clients.push(client)
  const onOpenChange = vi.fn()
  const result = render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <SpaceSessionBoundary enabled>
          <CreateProjectInSpacesDialog
            open
            onOpenChange={onOpenChange}
            onCreated={onCreated}
            navigateOnSuccess={false}
          />
        </SpaceSessionBoundary>
      </MemoryRouter>
    </QueryClientProvider>,
  )
  return { ...result, onOpenChange, onCreated, client }
}
async function fill() {
  const user = userEvent.setup()
  await waitFor(() => expect(screen.getByRole('textbox', { name: 'Ключ проекта' })).toBeEnabled())
  const input = screen.getByRole('textbox', { name: 'Ключ проекта' })
  await user.type(input, '-')
  await user.type(screen.getByRole('textbox', { name: 'Название проекта' }), 'Проект')
  return user
}
beforeEach(() => {
  vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'true')
  Object.values(api).forEach((f) => f.mockReset())
  sessionStorage.clear()
  useAuthStore.getState().logout()
  useAuthStore
    .getState()
    .setAuth({ token: 'fixture-a', userId: 'local-a', email: 'a@example.test' })
  acceptSpaceSession(
    {
      accessToken: 'fixture-a',
      subject: identity.subject,
      email: 'a@example.test',
      name: 'Fixture',
      returnTo: '/',
      expiresAt: Date.now() + 60_000,
    },
    { id: 'local-a' },
  )
  api.catalog.mockImplementation(async () => catalog())
})
afterEach(() => {
  cleanup()
  clients.splice(0).forEach((c) => c.clear())
  vi.unstubAllEnvs()
  useSpaceSession.setState({ session: null })
})
describe('explicit-space project creation', () => {
  it('associates key and name validation with their fields and clears it after correction', async () => {
    mount()
    const user = userEvent.setup()
    const keyField = await screen.findByRole('textbox', { name: 'Ключ проекта' })
    const nameField = screen.getByRole('textbox', { name: 'Название проекта' })
    await waitFor(() => expect(keyField).toBeEnabled())
    await user.type(keyField, '?')
    await screen.findByText('Ключ: от 1 до 10 латинских букв, цифр или дефисов.')
    expect(keyField).toHaveAttribute('aria-invalid', 'true')
    expect(keyField).toHaveAccessibleDescription(
      'Ключ: от 1 до 10 латинских букв, цифр или дефисов.',
    )
    await user.type(nameField, 'x')
    await user.clear(nameField)
    await screen.findByText('Укажите название до 200 символов.')
    expect(nameField).toHaveAttribute('aria-invalid', 'true')
    expect(nameField).toHaveAccessibleDescription('Укажите название до 200 символов.')
    await user.clear(keyField)
    await user.type(keyField, 'VALID')
    await user.type(nameField, 'Проект')
    await waitFor(() => {
      expect(keyField).not.toHaveAttribute('aria-invalid')
      expect(nameField).not.toHaveAttribute('aria-invalid')
    })
    expect(keyField).not.toHaveAttribute('aria-describedby')
    expect(nameField).not.toHaveAttribute('aria-describedby')
    expect(screen.getByRole('button', { name: 'Создать проект' })).toBeDisabled()
    expect(api.create).not.toHaveBeenCalled()
  })

  it('requires an explicit nonempty selection and captures the original valid hyphen key before POST', async () => {
    let captured: string | null = null
    api.create.mockImplementation(async () => {
      captured = sessionStorage.getItem(key)
      return project
    })
    const { onCreated } = mount()
    const user = await fill()
    expect(screen.getByRole('button', { name: 'Создать проект' })).toBeDisabled()
    expect(screen.getByText('Выбрано: 0')).toBeInTheDocument()
    await user.click(screen.getByRole('checkbox', { name: /^A / }))
    await user.click(screen.getByRole('button', { name: 'Создать проект' }))
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith(project))
    expect(JSON.parse(captured!)).toEqual({
      key: '-',
      name: 'Проект',
      description: null,
      spaces: [a],
    })
    expect(api.create).toHaveBeenCalledTimes(1)
    expect(sessionStorage.getItem(key)).toBeNull()
  })
  it('retains a lost outcome across remount and never invents a different key or performs a write during lookup', async () => {
    api.create.mockRejectedValue(new SpaceRequestError(503))
    api.lookup.mockRejectedValue(new SpaceRequestError(404))
    const first = mount()
    const user = await fill()
    await user.click(screen.getByRole('checkbox', { name: /^B / }))
    await user.click(screen.getByRole('button', { name: 'Создать проект' }))
    await screen.findByText(/Создание не подтверждено/)
    const stored = sessionStorage.getItem(key)
    expect(stored).not.toBeNull()
    first.unmount()
    mount()
    await user.click(await screen.findByRole('button', { name: 'Проверить проект по ключу' }))
    await screen.findByText(/Найденный проект не подтверждён/)
    expect(api.lookup).toHaveBeenCalledWith('-')
    expect(api.create).toHaveBeenCalledTimes(1)
    expect(sessionStorage.getItem(key)).toBe(stored)
    const retry = screen.getByRole('button', { name: 'Повторить исходный запрос' })
    await waitFor(() => expect(retry).toBeEnabled())
    await user.click(retry)
    await waitFor(() => expect(api.create).toHaveBeenCalledTimes(2))
    expect(api.create.mock.calls[1]![0]).toEqual(JSON.parse(stored!))
  })
  it('does not call an existing project a successful creation or clear the journal without explicit selection', async () => {
    const request = { key: '-', name: 'Проект', description: null, spaces: [a] }
    sessionStorage.setItem(key, JSON.stringify(request))
    api.lookup.mockResolvedValue({ ...project, name: 'Существующий проект', owner_id: 'another' })
    const { onCreated } = mount()
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: 'Проверить проект по ключу' }))
    expect(await screen.findByText(/Найден проект: - · Существующий проект/)).toBeInTheDocument()
    expect(onCreated).not.toHaveBeenCalled()
    expect(api.create).not.toHaveBeenCalled()
    expect(sessionStorage.getItem(key)).toBe(JSON.stringify(request))
    expect(screen.getByRole('link', { name: 'Открыть найденный проект' })).toHaveAttribute(
      'href',
      '/projects/-/board',
    )
    await user.click(screen.getByRole('button', { name: 'Использовать найденный проект' }))
    expect(onCreated).toHaveBeenCalledWith(expect.objectContaining({ name: 'Существующий проект' }))
    expect(sessionStorage.getItem(key)).toBeNull()
  })
  it('keeps a malformed journal and blocks submission', async () => {
    const saved = '{"key":"QA","spaces":[]}'
    sessionStorage.setItem(key, saved)
    mount()
    expect(await screen.findByText(/Журнал создания повреждён/)).toBeInTheDocument()
    expect(api.create).not.toHaveBeenCalled()
    expect(sessionStorage.getItem(key)).toBe(saved)
  })
  it('does not issue a project request when the own-user catalog is empty', async () => {
    api.catalog.mockImplementation(async () => catalog([]))
    mount()
    const user = await fill()
    expect(screen.getByRole('button', { name: 'Создать проект' })).toBeDisabled()
    expect(api.create).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: 'Создать проект' }))
    expect(api.create).not.toHaveBeenCalled()
  })
  it('shows the catalog error without mounting actionable controls before a policy can be confirmed', async () => {
    api.catalog.mockRejectedValue(new SpaceRequestError(503))
    mount()
    expect(
      await screen.findByText('Не удалось подтвердить доступные пространства.'),
    ).toBeInTheDocument()
    expect(screen.queryByRole('textbox', { name: 'Ключ проекта' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Создать проект' })).not.toBeInTheDocument()
    expect(api.create).not.toHaveBeenCalled()
    expect(sessionStorage.getItem(key)).toBeNull()
  })
  it('requires an explicit warning acknowledgement to forget an unknown original request', async () => {
    sessionStorage.setItem(
      key,
      JSON.stringify({ key: '-', name: 'Проект', description: null, spaces: [a] }),
    )
    mount()
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: 'Отказаться от повторов' }))
    expect(screen.getByText(/Уже начавшееся создание не отменится/)).toBeInTheDocument()
    expect(sessionStorage.getItem(key)).not.toBeNull()
    await user.click(screen.getByRole('button', { name: 'Прекратить повторы' }))
    await waitFor(() => expect(sessionStorage.getItem(key)).toBeNull())
    expect(api.create).not.toHaveBeenCalled()
  })
  it('does not navigate or notify a replacement session after an earlier create completes', async () => {
    let resolve: (project: Project) => void = () => {}
    api.create.mockImplementation(
      () =>
        new Promise<Project>((done) => {
          resolve = done
        }),
    )
    const { onCreated, onOpenChange } = mount()
    const user = await fill()
    await user.click(screen.getByRole('checkbox', { name: /^A / }))
    await user.click(screen.getByRole('button', { name: 'Создать проект' }))
    await waitFor(() => expect(api.create).toHaveBeenCalledTimes(1))
    await act(async () => {
      useAuthStore.getState().logout()
      useAuthStore
        .getState()
        .setAuth({ token: 'fixture-b', userId: 'local-b', email: 'b@example.test' })
      acceptSpaceSession(
        {
          accessToken: 'fixture-b',
          subject: 'central-b',
          email: 'b@example.test',
          name: 'B',
          returnTo: '/',
          expiresAt: Date.now() + 60_000,
        },
        { id: 'local-b' },
      )
      resolve(project)
    })
    expect(onCreated).not.toHaveBeenCalled()
    expect(onOpenChange).not.toHaveBeenCalled()
  })
})
