import { applicationAccessFixture } from '@/test/application-access'

const accessActor = vi.hoisted(() => ({
  role: 'admin' as 'user' | 'admin',
  applications: ['task-tracker'],
}))
vi.mock('@pdlc/ui/auth', async (original) => ({
  ...(await original<object>()),
  useApplicationAccess: () => applicationAccessFixture(accessActor),
}))
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import userEvent from '@testing-library/user-event'
import { ProjectActions, ProjectCardActionItems, ProjectCardLink } from './project-actions'
const catalog = vi.hoisted(() => vi.fn())
vi.mock('./namespace-context', () => ({ useNamespaceCatalog: catalog }))

beforeEach(() => {
  accessActor.role = 'admin'
})
afterEach(() => vi.unstubAllEnvs())
it.each(['pending', 'failed', 'ambiguous'] as const)(
  'keeps the project identity without inventing a board link when context is %s',
  (state) => {
    vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
    vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'false')
    const binding = {
      resource: { resource_id: 'second' },
      namespace: {
        registry_instance_id: '11111111-1111-4111-8111-111111111111',
        namespace_id: '22222222-2222-4222-8222-222222222222',
      },
    }
    catalog.mockReturnValue({
      isPending: state === 'pending',
      isError: state === 'failed',
      data: state === 'ambiguous' ? [{ binding }, { binding }] : undefined,
    })
    render(
      <MemoryRouter initialEntries={['/projects?project_scope=all']}>
        <ProjectCardLink project={{ id: 'second', key: 'SECOND', name: 'Same name' }}>
          <span>Same name · SECOND</span>
        </ProjectCardLink>
      </MemoryRouter>,
    )
    expect(screen.getByText('Same name · SECOND')).toBeInTheDocument()
    expect(screen.queryByRole('link')).not.toBeInTheDocument()
    expect(screen.getByText('Same name · SECOND').closest('[data-interactive="true"]')).toBeNull()
    if (state === 'pending') {
      expect(screen.getByText('Same name · SECOND').closest('[aria-busy="true"]')).not.toBeNull()
      expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    } else {
      expect(screen.getByRole('alert')).toHaveTextContent('Контекст проекта недоступен')
    }
  },
)

it.each(['admin', 'user'] as const)(
  'keeps delete permission and explicit selection for %s',
  async (role) => {
    vi.stubEnv('VITE_NAMESPACE_ENABLED', 'false')
    accessActor.role = role
    const onDelete = vi.fn()
    const { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } = await import('@pdlc/ui/ui')
    render(
      <MemoryRouter>
        <DropdownMenu>
          <DropdownMenuTrigger>Project actions</DropdownMenuTrigger>
          <DropdownMenuContent>
            <ProjectCardActionItems
              project={{ id: 'selected-project', key: 'SECOND', name: 'Same name' }}
              onDelete={onDelete}
            />
          </DropdownMenuContent>
        </DropdownMenu>
      </MemoryRouter>,
    )
    expect(onDelete).not.toHaveBeenCalled()
    await userEvent.click(screen.getByRole('button', { name: 'Project actions' }))
    expect(onDelete).not.toHaveBeenCalled()
    const deletion = screen.queryByRole('menuitem', { name: 'Удалить' })
    if (role === 'user') expect(deletion).not.toBeInTheDocument()
    else {
      expect(deletion).toBeInTheDocument()
      deletion!.focus()
      await userEvent.keyboard('{Enter}')
      expect(onDelete).toHaveBeenCalledTimes(1)
    }
  },
)
it('opens settings for the exact project without losing the all-projects scope', async () => {
  vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
  const namespace = {
    registry_instance_id: '11111111-1111-4111-8111-111111111111',
    namespace_id: '22222222-2222-4222-8222-222222222222',
  }
  render(
    <MemoryRouter initialEntries={['/projects?project_scope=all']}>
      <ProjectActions project={{ key: 'SECOND', name: 'Same name', namespace }} />
    </MemoryRouter>,
  )
  const href = screen
    .getByRole('link', { name: 'Настройки проекта: Same name · SECOND' })
    .getAttribute('href')!
  const url = new URL(href, 'https://test.invalid')
  expect(url.pathname).toBe('/projects/SECOND/settings')
  expect(url.searchParams.get('namespace_id')).toBe(namespace.namespace_id)
  expect(url.searchParams.get('project_scope')).toBe('all')
})
it('resolves card actions and its board by resource identity despite a different open project', async () => {
  vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
  const registry = '11111111-1111-4111-8111-111111111111'
  const source = '22222222-2222-4222-8222-222222222222'
  const destination = '33333333-3333-4333-8333-333333333333'
  catalog.mockReturnValue({
    data: [
      {
        binding: {
          resource: { resource_id: 'first' },
          namespace: { registry_instance_id: registry, namespace_id: source },
        },
      },
      {
        binding: {
          resource: { resource_id: 'second' },
          namespace: { registry_instance_id: registry, namespace_id: destination },
        },
      },
    ],
  })
  const project = { id: 'second', key: 'SECOND', name: 'Same name' }
  const { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } = await import('@pdlc/ui/ui')
  render(
    <MemoryRouter
      initialEntries={[
        `/projects?project_scope=all&registry_instance_id=${registry}&namespace_id=${source}`,
      ]}
    >
      <ProjectCardLink project={project}>Open board</ProjectCardLink>
      <DropdownMenu>
        <DropdownMenuTrigger>Actions</DropdownMenuTrigger>
        <DropdownMenuContent>
          <ProjectCardActionItems project={project} />
        </DropdownMenuContent>
      </DropdownMenu>
    </MemoryRouter>,
  )
  expect(
    new URL(
      screen.getByRole('link', { name: 'Open board' }).getAttribute('href')!,
      'https://test.invalid',
    ).searchParams.get('namespace_id'),
  ).toBe(destination)
  await userEvent.click(screen.getByRole('button', { name: 'Actions' }))
  const target = new URL(
    screen.getByRole('menuitem', { name: 'Корзина' }).getAttribute('href')!,
    'https://test.invalid',
  )
  expect(target.searchParams.get('namespace_id')).toBe(destination)
  expect(target.searchParams.get('project_scope')).toBe('all')
})

it('keeps the project board visible while hiding its settings from a user', () => {
  accessActor.role = 'user'
  render(
    <MemoryRouter>
      <ProjectCardLink project={{ id: 'second', key: 'SECOND', name: 'Same name' }}>
        Open board
      </ProjectCardLink>
      <ProjectActions project={{ key: 'SECOND', name: 'Same name' }} />
    </MemoryRouter>,
  )
  expect(screen.getByRole('link', { name: 'Open board' })).toBeInTheDocument()
  expect(screen.queryByRole('link', { name: /Настройки проекта/ })).not.toBeInTheDocument()
})
