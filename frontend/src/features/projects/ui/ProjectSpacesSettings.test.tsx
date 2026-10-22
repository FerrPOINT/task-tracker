import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Project } from '@/api/project'
import { SpaceRequestError, type VisibleSpaceCatalog } from '@/api/space-catalog'
import {
  projectSpaceKeys,
  type ConfirmedProjectSpaces,
  type ProjectSpaceManagement,
} from '@/api/project-spaces'
import {
  acceptSpaceSession,
  observeSpacePolicy,
  useSpaceSession,
} from '@/shared/auth/space-session'
import { ssoConfig, useAuthStore } from '@/shared/auth/store'
import { SpaceSessionBoundary } from '@/shared/auth/space-session-boundary'
import { ProjectSpacesSettings } from './ProjectSpacesSettings'

const owner = vi.hoisted(() => ({
  get: vi.fn(),
  save: vi.fn(),
  projection: vi.fn(),
  reconcile: vi.fn(),
  catalog: vi.fn(),
  receipt: vi.fn(),
}))
vi.mock('@/api/project-spaces', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/api/project-spaces')>()),
  getProjectSpaceManagement: owner.get,
  replaceProjectSpaces: owner.save,
  getProjectSpaceOperationReceipt: owner.receipt,
  getProjectSpaceProjection: owner.projection,
  reconcileProjectSpaceProjection: owner.reconcile,
}))
vi.mock('@/api/space-catalog', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/api/space-catalog')>()),
  getVisibleSpaceCatalog: owner.catalog,
}))
const registry = '00000000-0000-4000-8000-000000000001'
const a = { registry_instance_id: registry, space_id: '00000000-0000-4000-8000-000000000002' }
const b = { ...a, space_id: '00000000-0000-4000-8000-000000000003' }
const hidden = { ...a, space_id: '00000000-0000-4000-8000-000000000004' }
const project: Project = {
  id: '00000000-0000-4000-8000-000000000005',
  key: 'QA',
  name: 'Проект',
  owner_id: 'local-a',
  owner_name: 'Владелец',
  todo_count: 0,
  in_progress_count: 0,
  done_count: 0,
}
const identity = { issuer: ssoConfig.issuer, subject: 'central-a' }
const journalKey =
  'pdlc:command:tracker:project-spaces-v2:' +
  JSON.stringify([identity.issuer, identity.subject, project.id])
const clients: QueryClient[] = []
function links(spaces = [a, hidden], revision = 3): ConfirmedProjectSpaces {
  const now = Math.floor(Date.now() / 1000)
  return {
    project: { tracker_instance_id: registry, project_id: project.id },
    revision,
    spaces,
    validity: { observed_at: now, valid_until: now + 30 },
  }
}
function view(canChange = true, spaces = [a, hidden], revision = 3): ProjectSpaceManagement {
  return { links: links(spaces, revision), canChange, policyRevision: 1 }
}
function catalog(): VisibleSpaceCatalog {
  const now = Math.floor(Date.now() / 1000)
  return {
    schema_version: 2,
    identity,
    policy_revision: 1,
    validity: { observed_at: now, valid_until: now + 30 },
    total: 2,
    limit: 10,
    offset: 0,
    has_more: false,
    spaces: [a, b].map((ref, i) => ({
      space: ref,
      revision: 1,
      state: 'active',
      metadata: {
        slug: 'space-' + i,
        name: 'Активное ' + (i ? 'B' : 'A'),
        description: '',
        responsible_subject: identity.subject,
      },
    })),
  }
}
function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } })
  clients.push(client)
  const result = render(
    <QueryClientProvider client={client}>
      <SpaceSessionBoundary enabled>
        <ProjectSpacesSettings project={project} />
      </SpaceSessionBoundary>
    </QueryClientProvider>,
  )
  return { ...result, client }
}
beforeEach(() => {
  vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'true')
  Object.values(owner).forEach((fn) => fn.mockReset())
  sessionStorage.clear()
  useAuthStore.getState().logout()
  useAuthStore
    .getState()
    .setAuth({ token: 'fixture-a', userId: 'local-a', email: 'fixture@example.test' })
  acceptSpaceSession(
    {
      accessToken: 'fixture-a',
      subject: identity.subject,
      email: 'fixture@example.test',
      name: 'Fixture',
      returnTo: '/',
      expiresAt: Date.now() + 60_000,
    },
    { id: 'local-a' },
  )
  owner.get.mockResolvedValue(view())
  owner.catalog.mockImplementation(async () => catalog())
  owner.projection.mockResolvedValue({
    operation_id: '00000000-0000-4000-8000-000000000006',
    project: links().project,
    revision: 3,
    confirmed: false,
    last_error: null,
  })
})
afterEach(() => {
  cleanup()
  clients.splice(0).forEach((client) => client.clear())
  useSpaceSession.setState({ session: null })
  vi.unstubAllEnvs()
})
describe('project M:N settings', () => {
  it('preserves an unavailable association and does not infer write authority from owner_id', async () => {
    owner.get.mockResolvedValue(view(false))
    mount()
    expect(await screen.findByText(/Связи доступны только для чтения/)).toBeInTheDocument()
    expect(screen.getByText(new RegExp(hidden.space_id))).toBeInTheDocument()
    expect(await screen.findByRole('checkbox', { name: /Активное B/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Сохранить связи' })).toBeDisabled()
    expect(owner.save).not.toHaveBeenCalled()
    expect(owner.catalog.mock.calls.every(([, query]) => query.limit === 10)).toBe(true)
  })
  it('keeps hidden IDs, captures before HTTP and waits for a separate Wiki confirmation', async () => {
    let capturedBeforeHttp: string | null = null
    owner.save.mockImplementation(async () => {
      capturedBeforeHttp = sessionStorage.getItem(journalKey)
      owner.get.mockResolvedValue(view(true, [a, hidden, b], 4))
      return links([a, hidden, b], 4)
    })
    const user = userEvent.setup()
    mount()
    await user.click(await screen.findByRole('checkbox', { name: /Активное B/ }))
    await user.click(screen.getByRole('button', { name: 'Сохранить связи' }))
    await waitFor(() => expect(owner.save).toHaveBeenCalledTimes(1))
    const command = owner.save.mock.calls[0]![1]
    expect(capturedBeforeHttp).not.toBeNull()
    expect(JSON.parse(capturedBeforeHttp!)).toEqual(command)
    await screen.findByText(/Сохранение подтверждено/)
    expect(command).toMatchObject({ expected_revision: 3, spaces: [a, hidden, b] })
    expect(command.operation_id).toMatch(/^[0-9a-f-]{36}$/)
    await waitFor(() => expect(sessionStorage.getItem(journalKey)).toBeNull())
    expect(screen.getByText(/Ожидаем подтверждение/)).toBeInTheDocument()
    expect(screen.queryByText('Wiki подтвердила текущие связи.')).not.toBeInTheDocument()
  })
  it('does not remove the last association', async () => {
    owner.get.mockResolvedValue(view(true, [a]))
    mount()
    expect(await screen.findByRole('checkbox', { name: /Активное A/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Убрать пространство ' + a.space_id })).toBeDisabled()
    expect(owner.save).not.toHaveBeenCalled()
  })
  it('retains the revision captured at first edit across background refresh', async () => {
    owner.save.mockRejectedValue(new SpaceRequestError(409))
    const user = userEvent.setup()
    const { client } = mount()
    await user.click(await screen.findByRole('checkbox', { name: /Активное B/ }))
    owner.get.mockResolvedValue(view(true, [a, hidden], 4))
    await act(async () => {
      await client.invalidateQueries({ queryKey: projectSpaceKeys.project(identity, project) })
    })
    await user.click(screen.getByRole('button', { name: 'Сохранить связи' }))
    await waitFor(() => expect(owner.save).toHaveBeenCalledTimes(1))
    expect(owner.save.mock.calls[0]![1].expected_revision).toBe(3)
    expect(sessionStorage.getItem(journalKey)).not.toBeNull()
    expect(
      await screen.findByRole('button', {
        name: 'Отменить это изменение и загрузить текущие связи',
      }),
    ).toBeEnabled()
  })
  it('restores an uncertain command after remount and retries exactly the same payload/key', async () => {
    owner.save.mockRejectedValueOnce(new SpaceRequestError(503)).mockImplementation(async () => {
      owner.get.mockResolvedValue(view(true, [a, hidden, b], 4))
      return links([a, hidden, b], 4)
    })
    const user = userEvent.setup()
    const first = mount()
    await user.click(await screen.findByRole('checkbox', { name: /Активное B/ }))
    await user.click(screen.getByRole('button', { name: 'Сохранить связи' }))
    await screen.findByText(/Исход изменения не подтверждён/)
    const stored = sessionStorage.getItem(journalKey)
    expect(stored).not.toBeNull()
    first.unmount()
    mount()
    const retry = await screen.findByRole('button', { name: 'Повторить исходным запросом' })
    await waitFor(() => expect(retry).toBeEnabled())
    await user.click(retry)
    await waitFor(() => expect(owner.save).toHaveBeenCalledTimes(2))
    expect(owner.save.mock.calls[1]![1]).toEqual(JSON.parse(stored!))
  })
  it('blocks a malformed saved command without deleting it', async () => {
    const invalid = '{"operation_id":"bad","spaces":[],"expected_revision":0}'
    sessionStorage.setItem(journalKey, invalid)
    mount()
    expect(await screen.findByText(/Журнал изменения связей повреждён/)).toBeInTheDocument()
    expect(sessionStorage.getItem(journalKey)).toBe(invalid)
    expect(owner.save).not.toHaveBeenCalled()
  })
  it('does not display an old or foreign Wiki confirmation as completion for this set', async () => {
    owner.projection.mockResolvedValue({
      operation_id: '00000000-0000-4000-8000-000000000006',
      project: {
        tracker_instance_id: '00000000-0000-4000-8000-000000000099',
        project_id: project.id,
      },
      revision: 2,
      confirmed: true,
      last_error: null,
    })
    mount()
    await screen.findByRole('checkbox', { name: /Активное B/ })
    expect(screen.queryByText('Wiki подтвердила текущие связи.')).not.toBeInTheDocument()
    expect(screen.getByText(/Ожидаем подтверждение/)).toBeInTheDocument()
  })
  it('hides a management response from an older admission policy revision', async () => {
    mount()
    await screen.findByRole('checkbox', { name: /Активное B/ })
    owner.catalog.mockImplementation(async () => ({ ...catalog(), policy_revision: 2 }))
    await act(async () => {
      observeSpacePolicy(identity, 2)
    })
    await screen.findByText('Не удалось подтвердить связи и доступ к проекту.')
    expect(screen.queryByRole('button', { name: 'Сохранить связи' })).not.toBeInTheDocument()
    expect(owner.save).not.toHaveBeenCalled()
  })
  it('does not mount v2 reads while its cutover flag is off', () => {
    vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'false')
    mount()
    expect(owner.get).not.toHaveBeenCalled()
    expect(owner.catalog).not.toHaveBeenCalled()
  })
})

it('resolves the original committed operation with read-only authority after archive', async () => {
  const command = {
    operation_id: '00000000-0000-4000-8000-000000000077',
    expected_revision: 3,
    spaces: [a, hidden],
  }
  sessionStorage.setItem(journalKey, JSON.stringify(command))
  owner.get.mockResolvedValue(view(false))
  owner.receipt.mockResolvedValue({
    schema_version: 1,
    operation_id: command.operation_id,
    project: links().project,
    applied: true,
    applied_revision: 3,
    change: command,
    validity: links().validity,
  })
  const user = userEvent.setup()
  mount()
  const check = await screen.findByRole('button', { name: 'Проверить сохранение' })
  await waitFor(() => expect(check).toBeEnabled())
  await user.click(check)
  await waitFor(() => expect(sessionStorage.getItem(journalKey)).toBeNull())
  expect(owner.save).not.toHaveBeenCalled()
  expect(owner.receipt).toHaveBeenCalledWith(project, links().project, command)
})
it('keeps unknown outcome after a missing read-only receipt without issuing a new write/key', async () => {
  const command = {
    operation_id: '00000000-0000-4000-8000-000000000077',
    expected_revision: 3,
    spaces: [a, hidden],
  }
  const stored = JSON.stringify(command)
  sessionStorage.setItem(journalKey, stored)
  owner.get.mockResolvedValue(view(false))
  owner.receipt.mockRejectedValue(new SpaceRequestError(404))
  const user = userEvent.setup()
  mount()
  await user.click(await screen.findByRole('button', { name: 'Проверить сохранение' }))
  await screen.findByText(/Квитанция не подтверждена/)
  expect(sessionStorage.getItem(journalKey)).toBe(stored)
  expect(owner.save).not.toHaveBeenCalled()
})
