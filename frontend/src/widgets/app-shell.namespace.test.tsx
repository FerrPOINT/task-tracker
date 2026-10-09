import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter, Route, Routes, useLocation } from 'react-router'
import { ThemeProvider } from '@sdlc/ui/lib'
import { AppShell } from './app-shell'
import { ProjectLink, useProjectNavigate } from '@/shared/lib/project-navigation'
import type { ResourceContext } from './namespace-context'

vi.mock('@/shared/api/hooks', () => ({
  useCurrentUser: () => ({ data: { email: 'operator@example.test' } }),
  useLogout: () => ({ mutate: vi.fn() }),
  useNotifications: () => ({ data: { notifications: [], unread_count: 0 } }),
  useMarkNotificationRead: () => ({ mutate: vi.fn() }),
  useMarkAllNotificationsRead: () => ({ mutate: vi.fn() }),
  useIssue: () => ({ data: undefined }),
  useProjects: () => ({ data: [{ id: 'unrelated', key: 'WRONG', name: 'Wrong fallback' }] }),
}))
vi.mock('@/shared/api/useTrackerEvents', () => ({ useTrackerEvents: vi.fn() }))

const registry = '506a8476-3868-4581-af70-6d22c93ced2f'
const refs = ['0ee31fca-2533-44bf-b54e-a7759fe80b14', 'a93e0210-d3f6-4936-b465-90da8312f871']
const contexts: ResourceContext[] = refs.map((namespace_id, index) => ({
  label: 'Одинаковый проект',
  resource_key: index ? 'B' : 'A',
  binding: {
    schema_version: 1,
    operation_id: namespace_id,
    generation: 1,
    state: 'active',
    drained: true,
    namespace: { registry_instance_id: registry, namespace_id },
    resource: { instance_id: registry, resource_id: namespace_id, kind: 'tracker_project' },
  },
}))
const selected = (index: number) => `${registry}/${refs[index]}`
const search = (index: number) => `?registry_instance_id=${registry}&namespace_id=${refs[index]}`
function Location() {
  const location = useLocation()
  const navigate = useProjectNavigate()
  return (
    <>
      <output aria-label="Current URL">{location.pathname + location.search}</output>
      <ProjectLink to="/issues/task-id?tab=worklog">Task detail</ProjectLink>
      <button onClick={() => navigate(-1)}>Back</button>
      <button onClick={() => navigate(1)}>Forward</button>
      <button onClick={() => navigate('/projects/B/backlog')}>Navigate backlog</button>
    </>
  )
}
function mount(path = '/namespace') {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(
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

describe('project sidebar and Namespace selection', () => {
  beforeEach(() => {
    vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
    window.localStorage.removeItem('tt-sidebar-collapsed')
    window.localStorage.removeItem('tt-project-nav-collapsed:v1')
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => {
        const path = new URL(url, 'http://tracker.test')
        const entry = contexts.find((item) =>
          path.pathname.endsWith(`/${item.binding.namespace.namespace_id}`),
        )
        return new Response(JSON.stringify(entry ?? contexts), { status: 200 })
      }),
    )
  })
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.unstubAllEnvs()
  })

  it('opens every catalog project, collapses each independently and links its own Namespace', async () => {
    const user = userEvent.setup()
    mount()
    const navigation = await screen.findByRole('navigation', {
      name: 'Навигация проекта',
    })
    const a = within(navigation).getByRole('region', {
      name: 'Одинаковый проект · A',
    })
    const b = within(navigation).getByRole('region', {
      name: 'Одинаковый проект · B',
    })
    expect(within(a).getByRole('link', { name: 'Доска' })).toHaveAttribute(
      'href',
      `/projects/A/board${search(0)}&project_scope=all`,
    )
    expect(within(b).getByRole('link', { name: 'Доска' })).toHaveAttribute(
      'href',
      `/projects/B/board${search(1)}&project_scope=all`,
    )
    const toggle = within(a).getByRole('button', { name: 'Одинаковый проект · A' })
    await user.click(toggle)
    expect(toggle).toHaveAttribute('aria-expanded', 'false')
    expect(within(a).queryByRole('link', { name: 'Доска' })).not.toBeInTheDocument()
    expect(within(b).getByRole('link', { name: 'Бэклог' })).toBeVisible()
    await user.click(within(b).getByRole('link', { name: 'Доска' }))
    await waitFor(() => expect(screen.getByRole('combobox', { name: 'Namespace' })).toHaveValue(''))
    expect(screen.getByRole('region', { name: 'Одинаковый проект · A' })).toBeInTheDocument()
    expect(toggle).toHaveAttribute('aria-expanded', 'false')
    expect(screen.getByLabelText('Current URL')).toHaveTextContent(
      `/projects/B/board${search(1)}&project_scope=all`,
    )
    await user.click(screen.getByRole('link', { name: 'Task detail' }))
    expect(screen.getByLabelText('Current URL')).toHaveTextContent('tab=worklog&project_scope=all')
    expect(screen.getByRole('combobox', { name: 'Namespace' })).toHaveValue('')
    expect(toggle).toHaveAttribute('aria-expanded', 'false')
    await user.click(screen.getByRole('button', { name: 'Navigate backlog' }))
    expect(screen.getByLabelText('Current URL')).toHaveTextContent(
      '/projects/B/backlog?project_scope=all',
    )
    expect(screen.getAllByRole('region')).toHaveLength(2)
  })

  it('updates navigation and task creation on top selection, restores all and preserves a selected collapsed group', async () => {
    const user = userEvent.setup()
    mount()
    await screen.findByRole('button', { name: 'Одинаковый проект · A' })
    await user.click(screen.getByRole('button', { name: 'Одинаковый проект · A' }))
    const picker = screen.getByRole('combobox', { name: 'Namespace' })
    await user.selectOptions(picker, selected(0))
    await waitFor(() =>
      expect(
        screen.queryByRole('region', { name: 'Одинаковый проект · B' }),
      ).not.toBeInTheDocument(),
    )
    expect(screen.getByRole('button', { name: 'Одинаковый проект · A' })).toHaveAttribute(
      'aria-expanded',
      'false',
    )
    expect(
      within(screen.getByRole('banner')).getByRole('link', { name: 'Создать' }),
    ).toHaveAttribute(
      'href',
      `/issues/create?project_key=A&registry_instance_id=${registry}&namespace_id=${refs[0]}`,
    )
    await user.selectOptions(picker, selected(1))
    await screen.findByRole('button', { name: 'Одинаковый проект · B' })
    expect(screen.queryByRole('region', { name: 'Одинаковый проект · A' })).not.toBeInTheDocument()
    await user.selectOptions(picker, '')
    await screen.findByRole('button', { name: 'Одинаковый проект · A' })
    expect(screen.getByRole('button', { name: 'Одинаковый проект · B' })).toHaveAttribute(
      'aria-expanded',
      'true',
    )
  })

  it('uses the selected binding even on a foreign project route', async () => {
    mount(`/projects/WRONG/board${search(0)}`)
    await screen.findByRole('button', { name: 'Одинаковый проект · A' })
    expect(screen.queryByText('Wrong fallback')).not.toBeInTheDocument()
    expect(screen.getByRole('alert')).toHaveTextContent('Ресурс не подтверждён')
  })

  it('keeps an unavailable binding selected without showing a different project', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(
        async (url: string) =>
          new Response(JSON.stringify(url.includes('/namespace-contexts/') ? {} : contexts), {
            status: url.includes('/namespace-contexts/') ? 404 : 200,
          }),
      ),
    )
    mount(`/projects/A/board${search(0)}`)
    await screen.findByRole('alert')
    expect(screen.getByRole('combobox', { name: 'Namespace' })).toHaveValue(selected(0))
    expect(screen.getByRole('combobox', { name: 'Namespace' })).toBeDisabled()
    expect(screen.queryByRole('navigation', { name: 'Навигация проекта' })).not.toBeInTheDocument()
    expect(screen.queryByText('Wrong fallback')).not.toBeInTheDocument()
  })

  it('keeps project controls in the mobile drawer and closes after navigation', async () => {
    const user = userEvent.setup()
    mount()
    await screen.findByRole('button', { name: 'Одинаковый проект · A' })
    await user.click(screen.getByRole('button', { name: 'Открыть меню' }))
    const dialog = await screen.findByRole('dialog')
    const b = within(dialog).getByRole('region', { name: 'Одинаковый проект · B' })
    await user.click(within(b).getByRole('button', { name: 'Одинаковый проект · B' }))
    expect(within(b).queryByRole('link', { name: 'Доска' })).not.toBeInTheDocument()
    await user.click(within(b).getByRole('button', { name: 'Одинаковый проект · B' }))
    await user.click(within(b).getByRole('link', { name: 'Бэклог' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(screen.getByRole('combobox', { name: 'Namespace' })).toHaveValue('')
    expect(screen.getAllByRole('region')).toHaveLength(2)
  })

  it('restores the filter through history and a copied all-project resource URL', async () => {
    const user = userEvent.setup()
    mount(`/projects/B/trash${search(1)}&project_scope=all`)
    await screen.findByRole('button', { name: 'Одинаковый проект · A' })
    const picker = screen.getByRole('combobox', { name: 'Namespace' })
    expect(picker).toHaveValue('')
    await user.selectOptions(picker, selected(0))
    await waitFor(() => expect(screen.getAllByRole('region')).toHaveLength(1))
    await user.click(screen.getByRole('button', { name: 'Back' }))
    await waitFor(() => expect(screen.getAllByRole('region')).toHaveLength(2))
    expect(picker).toHaveValue('')
    await user.click(screen.getByRole('button', { name: 'Forward' }))
    await waitFor(() => expect(screen.getAllByRole('region')).toHaveLength(1))
    expect(picker).toHaveValue(selected(0))
  })

  it('keeps all projects visible while rejecting a foreign resource binding', async () => {
    mount(`/projects/WRONG/board${search(0)}&project_scope=all`)
    await screen.findByRole('button', { name: 'Одинаковый проект · B' })
    expect(screen.getByRole('combobox', { name: 'Namespace' })).toHaveValue('')
    expect(screen.getByRole('alert')).toHaveTextContent('Ресурс не подтверждён')
    expect(screen.queryByText('Wrong fallback')).not.toBeInTheDocument()
  })

  it('remembers independent groups across remounts, including expansion and selected-project URLs', async () => {
    const user = userEvent.setup()
    const first = mount()
    await screen.findByRole('button', { name: 'Одинаковый проект · A' })
    await user.click(screen.getByRole('button', { name: 'Одинаковый проект · A' }))
    first.unmount()
    const second = mount(`/projects/A/trash${search(0)}`)
    const a = await screen.findByRole('button', { name: 'Одинаковый проект · A' })
    expect(a).toHaveAttribute('aria-expanded', 'false')
    await user.click(a)
    second.unmount()
    mount(`/projects/B/trash${search(1)}&project_scope=all`)
    expect(await screen.findByRole('button', { name: 'Одинаковый проект · A' })).toHaveAttribute(
      'aria-expanded',
      'true',
    )
    expect(screen.getByRole('button', { name: 'Одинаковый проект · B' })).toHaveAttribute(
      'aria-expanded',
      'true',
    )
  })

  it('restores collapsed state by stable identity despite identical project names', async () => {
    window.localStorage.setItem(
      'tt-project-nav-collapsed:v1',
      JSON.stringify([`${registry}/${refs[1]}`]),
    )
    mount()
    expect(await screen.findByRole('button', { name: 'Одинаковый проект · A' })).toHaveAttribute(
      'aria-expanded',
      'true',
    )
    expect(screen.getByRole('button', { name: 'Одинаковый проект · B' })).toHaveAttribute(
      'aria-expanded',
      'false',
    )
  })

  it('ignores malformed saved preferences without hiding the project catalog', async () => {
    window.localStorage.setItem('tt-project-nav-collapsed:v1', '{broken')
    mount()
    expect(await screen.findByRole('button', { name: 'Одинаковый проект · A' })).toHaveAttribute(
      'aria-expanded',
      'true',
    )
    expect(screen.getByRole('button', { name: 'Одинаковый проект · B' })).toHaveAttribute(
      'aria-expanded',
      'true',
    )
  })

  it('uses one Base row/list contract for project headings and nested links in both navigation surfaces', async () => {
    const user = userEvent.setup()
    mount()
    const heading = await screen.findByRole('button', { name: 'Одинаковый проект · A' })
    expect(heading).toHaveClass('base-sidebar-item')
    expect(heading.querySelector('svg')).not.toBeNull()
    const list = document.getElementById(heading.getAttribute('aria-controls')!)!
    expect(list).toHaveClass('base-sidebar-list')
    expect(list).not.toHaveAttribute('hidden')
    for (const link of within(list).getAllByRole('link'))
      expect(link).toHaveClass('base-sidebar-item')
    await user.click(heading)
    expect(list).toHaveAttribute('hidden')
    await user.click(screen.getByRole('button', { name: 'Открыть меню' }))
    const dialog = await screen.findByRole('dialog')
    const mobileHeading = within(dialog).getByRole('button', { name: 'Одинаковый проект · A' })
    const mobileList = document.getElementById(mobileHeading.getAttribute('aria-controls')!)!
    expect(mobileHeading).toHaveClass('base-sidebar-item')
    expect(mobileList).toHaveAttribute('hidden')
    await user.click(mobileHeading)
    expect(mobileList).not.toHaveAttribute('hidden')
    expect(within(mobileList).getByRole('link', { name: 'Доска' })).toHaveClass('base-sidebar-item')
  })

  it('reads the whole paginated catalog for both the picker and sidebar', async () => {
    const first = Array.from({ length: 100 }, (_, index) => ({
      ...contexts[0],
      resource_key: `P${index}`,
      binding: {
        ...contexts[0]!.binding,
        namespace: {
          registry_instance_id: registry,
          namespace_id: `11111111-1111-4111-8111-${String(index).padStart(12, '0')}`,
        },
        resource: {
          ...contexts[0]!.binding.resource,
          resource_id: `22222222-2222-4222-8222-${String(index).padStart(12, '0')}`,
        },
      },
    }))
    vi.stubGlobal(
      'fetch',
      vi.fn(
        async (url: string) =>
          new Response(
            JSON.stringify(
              new URL(url, 'http://tracker.test').searchParams.get('offset') === '0'
                ? first
                : [contexts[1]],
            ),
          ),
      ),
    )
    mount()
    await screen.findByRole('button', { name: 'Одинаковый проект · B' })
    expect(screen.getAllByRole('region')).toHaveLength(101)
    expect(
      within(screen.getByRole('combobox', { name: 'Namespace' })).getAllByRole('option'),
    ).toHaveLength(102)
  })
})
