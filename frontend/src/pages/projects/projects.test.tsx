import { afterEach, beforeEach, describe, it, expect, vi } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'

import { ProjectsPage } from './'
import { ThemeProvider } from '@pdlc/ui/lib'
import i18n from '@/shared/i18n/config'

const ownerAccess = vi.hoisted(() => ({ admin: true }))
vi.mock('@pdlc/ui/auth', async (original) => ({
  ...(await original<typeof import('@pdlc/ui/auth')>()),
  useApplicationAccess: () => ({
    permits: (_apps: readonly string[], administrative = false) =>
      !administrative || ownerAccess.admin,
  }),
}))

const useProjects = vi.hoisted(() => vi.fn())
const useCreateProject = vi.hoisted(() => vi.fn())
const useUpdateProject = vi.hoisted(() => vi.fn())
const useDeleteProject = vi.hoisted(() => vi.fn())
const namespaceCatalog = vi.hoisted(() => vi.fn())
const platformServices = vi.hoisted(() => vi.fn())

vi.mock('@/widgets/namespace-context', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/widgets/namespace-context')>()),
  useNamespaceCatalog: namespaceCatalog,
}))
vi.mock('@pdlc/ui/ui', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@pdlc/ui/ui')>()),
  usePlatformServices: platformServices,
}))

vi.mock('@/shared/api/hooks', () => ({
  useProjects,
  useCreateProject,
  useUpdateProject,
  useDeleteProject,
}))

const projects = [
  {
    id: 'p1',
    key: 'TT',
    name: 'Task Tracker',
    owner_id: 'u1',
    todo_count: 2,
    in_progress_count: 3,
    done_count: 1,
  },
  {
    id: 'p2',
    key: 'DOC',
    name: 'Knowledge Base',
    owner_id: 'u2',
    todo_count: 1,
    in_progress_count: 0,
    done_count: 4,
  },
]

function renderPage() {
  return render(
    <ThemeProvider>
      <MemoryRouter>
        <ProjectsPage />
      </MemoryRouter>
    </ThemeProvider>,
  )
}

beforeEach(() => {
  ownerAccess.admin = true
  vi.stubEnv('VITE_NAMESPACE_ENABLED', 'false')
  namespaceCatalog.mockReturnValue({ data: [], isPending: false, isError: false, isSuccess: true })
  platformServices.mockReturnValue({
    services: [{ key: 'admin-panel', ui_url: 'https://platform.test/custom-admin/' }],
  })
  useProjects.mockReturnValue({ data: projects, isLoading: false, error: null, refetch: vi.fn() })
  useCreateProject.mockReturnValue({ mutate: vi.fn(), isPending: false, error: null })
  useUpdateProject.mockReturnValue({ mutate: vi.fn(), isPending: false, error: null })
  useDeleteProject.mockReturnValue({
    mutate: vi.fn(),
    reset: vi.fn(),
    isPending: false,
    error: null,
  })
})

afterEach(async () => {
  await act(async () => {
    await i18n.changeLanguage('ru')
  })
  vi.unstubAllEnvs()
  vi.clearAllMocks()
})

describe('ProjectsPage', () => {
  it('renders compact project rows with distinct links and actions', () => {
    renderPage()
    expect(screen.getByRole('list').querySelectorAll('li')).toHaveLength(2)
    expect(screen.getByRole('link', { name: /Task Tracker/i })).toHaveAttribute(
      'href',
      '/projects/TT/board',
    )
    expect(
      screen.getByRole('button', { name: 'Действия с проектом Task Tracker' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: 'Действия с проектом Knowledge Base' }),
    ).toBeInTheDocument()
    expect(screen.getAllByText('К выполнению')).toHaveLength(2)
    for (const link of screen.getAllByRole('link')) {
      expect(link.querySelector('button')).toBeNull()
    }
  })

  it('keeps offset admission based on the unfiltered response rather than the visible names', () => {
    useProjects.mockReturnValue({
      data: Array.from({ length: 50 }, (_, index) => ({
        ...projects[0],
        id: `project-${index}`,
        key: `KEY-${index}`,
      })),
      isLoading: false,
      error: null,
      refetch: vi.fn(),
    })
    renderPage()
    expect(useProjects.mock.calls.every(([offset]) => offset === 0)).toBe(true)
    expect(screen.getByRole('button', { name: 'Назад' })).toBeDisabled()
    fireEvent.change(screen.getByRole('searchbox', { name: i18n.t('projects.search') }), {
      target: { value: 'no matching name' },
    })
    expect(screen.getByRole('button', { name: 'Далее' })).toBeEnabled()
    useProjects.mockReturnValue({
      data: [projects[1]],
      isLoading: false,
      error: null,
      refetch: vi.fn(),
    })
    fireEvent.click(screen.getByRole('button', { name: 'Далее' }))
    expect(useProjects).toHaveBeenLastCalledWith(50, 50, '', true, { allPages: false })
    expect(screen.getByRole('button', { name: 'Далее' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Назад' })).toBeEnabled()
    fireEvent.click(screen.getByRole('button', { name: 'Назад' }))
    expect(useProjects).toHaveBeenLastCalledWith(0, 50, '', true, { allPages: false })
  })

  it('shows a localized no-match state after filtering', async () => {
    await act(async () => {
      await i18n.changeLanguage('en')
    })
    renderPage()
    fireEvent.change(screen.getByRole('searchbox', { name: /search projects/i }), {
      target: { value: 'not found' },
    })
    expect(screen.getByText('No projects match your search.')).toBeInTheDocument()
    expect(screen.queryByRole('list')).not.toBeInTheDocument()
  })

  it('offers retry when projects fail to load', () => {
    const refetch = vi.fn()
    useProjects.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('offline'),
      refetch,
    })
    renderPage()
    expect(screen.getByText('offline')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /повторить|retry/i }))
    expect(refetch).toHaveBeenCalledOnce()
  })

  it('keeps the confirmation open while deletion runs or fails', async () => {
    const remove = {
      mutate: vi.fn(),
      reset: vi.fn(),
      isPending: false,
      error: null as Error | null,
    }
    useDeleteProject.mockReturnValue(remove)
    const view = renderPage()

    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'Действия с проектом Task Tracker' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Удалить' }))
    expect(screen.getByText(/Проект «Task Tracker»/)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Удалить' }))
    expect(remove.mutate).toHaveBeenCalledWith(
      'TT',
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    )
    expect(screen.getByRole('alertdialog')).toBeInTheDocument()

    remove.isPending = true
    view.rerender(
      <ThemeProvider>
        <MemoryRouter>
          <ProjectsPage />
        </MemoryRouter>
      </ThemeProvider>,
    )
    expect(screen.getByRole('button', { name: 'Отмена' })).toBeDisabled()
    expect(screen.getByRole('button', { name: /загрузка/i })).toBeDisabled()

    remove.isPending = false
    remove.error = new Error('Удаление не удалось')
    view.rerender(
      <ThemeProvider>
        <MemoryRouter>
          <ProjectsPage />
        </MemoryRouter>
      </ThemeProvider>,
    )
    expect(screen.getByRole('alert')).toHaveTextContent('Не удалось удалить проект')
    expect(screen.getByRole('alertdialog')).toBeInTheDocument()
  })
})

const managedNamespace = {
  registry_instance_id: '11111111-1111-4111-8111-111111111111',
  namespace_id: '22222222-2222-4222-8222-222222222222',
}
function boundProject(state = 'active') {
  return {
    binding: {
      resource: { resource_id: 'p1' },
      namespace: managedNamespace,
      state,
    },
  }
}

it.each(['active'])('uses lifecycle management for a %s shared project', async (state) => {
  vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
  namespaceCatalog.mockReturnValue({
    data: [boundProject(state)],
    isPending: false,
    isError: false,
    isSuccess: true,
  })
  renderPage()
  const user = userEvent.setup()
  await user.click(screen.getByRole('button', { name: 'Действия с проектом Task Tracker' }))
  expect(screen.queryByRole('menuitem', { name: 'Удалить' })).not.toBeInTheDocument()
  const action = screen.getByRole('menuitem', { name: 'Управление проектом' })
  const url = new URL(action.getAttribute('href')!)
  expect(url.pathname).toBe(`/custom-admin/namespaces/${managedNamespace.namespace_id}`)
  expect(url.searchParams.get('registry_instance_id')).toBe(managedNamespace.registry_instance_id)
  expect(url.searchParams.get('namespace_id')).toBe(managedNamespace.namespace_id)
  expect(screen.queryByText('В архиве')).not.toBeInTheDocument()
  expect(useDeleteProject().mutate).not.toHaveBeenCalled()
})

it('keeps deletion for an unbound project beside a shared project', async () => {
  vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
  namespaceCatalog.mockReturnValue({
    data: [boundProject()],
    isPending: false,
    isError: false,
    isSuccess: true,
  })
  renderPage()
  const user = userEvent.setup()
  await user.click(screen.getByRole('button', { name: 'Действия с проектом Knowledge Base' }))
  await user.click(screen.getByRole('menuitem', { name: 'Удалить' }))
  fireEvent.click(screen.getByRole('button', { name: 'Удалить' }))
  expect(useDeleteProject().mutate).toHaveBeenCalledWith('DOC', expect.any(Object))
})

it.each([
  { data: undefined, isPending: true, isError: false },
  { data: undefined, isPending: false, isError: true },
  { data: [boundProject(), boundProject()], isPending: false, isError: false, isSuccess: true },
])('does not offer deletion when project ownership is unresolved: %j', async (query) => {
  vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
  namespaceCatalog.mockReturnValue({
    ...query,
    isSuccess: !query.isPending && !query.isError,
    error: query.isError ? new Error('catalog unavailable') : null,
  })
  renderPage()
  if (query.isPending || query.isError) {
    expect(screen.queryByRole('button', { name: 'Действия с проектом Task Tracker' })).toBeNull()
    expect(screen.getByRole(query.isPending ? 'status' : 'alert')).toBeInTheDocument()
  } else {
    await userEvent.click(screen.getByRole('button', { name: 'Действия с проектом Task Tracker' }))
  }
  expect(screen.queryByRole('menuitem', { name: 'Удалить' })).not.toBeInTheDocument()
})

it('keeps a shared project protected when lifecycle management is unavailable', async () => {
  vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
  namespaceCatalog.mockReturnValue({
    data: [boundProject()],
    isPending: false,
    isError: false,
    isSuccess: true,
  })
  platformServices.mockReturnValue({ services: [] })
  renderPage()
  await userEvent.click(screen.getByRole('button', { name: 'Действия с проектом Task Tracker' }))
  expect(screen.queryByRole('menuitem', { name: 'Удалить' })).not.toBeInTheDocument()
  expect(screen.getByRole('menuitem', { name: 'Управление проектом недоступно' })).toHaveAttribute(
    'aria-disabled',
    'true',
  )
})

it('hides an archived shared row without hiding its active standalone neighbor', () => {
  vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
  namespaceCatalog.mockReturnValue({
    data: [boundProject('archived')],
    isPending: false,
    isError: false,
    isSuccess: true,
  })
  renderPage()
  expect(screen.queryByRole('link', { name: /Task Tracker/i })).toBeNull()
  expect(screen.getByRole('link', { name: /Knowledge Base/i })).toBeInTheDocument()
})

it('preserves ordinary project navigation while hiding structure changes', async () => {
  ownerAccess.admin = false
  renderPage()
  expect(screen.getByRole('link', { name: /Task Tracker/i })).toHaveAttribute(
    'href',
    '/projects/TT/board',
  )
  expect(screen.queryByRole('button', { name: 'Создать проект' })).not.toBeInTheDocument()
  const user = userEvent.setup()
  await user.click(screen.getByRole('button', { name: 'Действия с проектом Task Tracker' }))
  expect(await screen.findByRole('menuitem', { name: 'Корзина' })).toBeInTheDocument()
  expect(screen.queryByRole('menuitem', { name: 'Удалить' })).not.toBeInTheDocument()
  expect(screen.queryByRole('menuitem', { name: 'Настройки' })).not.toBeInTheDocument()
})
