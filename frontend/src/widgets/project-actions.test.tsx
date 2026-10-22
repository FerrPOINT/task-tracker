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
