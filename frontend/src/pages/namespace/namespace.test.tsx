import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, useLocation } from 'react-router'
import { beforeEach, expect, it, vi } from 'vitest'
import { NamespacePage } from './'

const context = vi.hoisted(() => vi.fn())
vi.mock('@/widgets/namespace-context', () => ({ useNamespaceContext: context }))
const ref = {
  registry_instance_id: '11111111-1111-4111-8111-111111111111',
  namespace_id: '22222222-2222-4222-8222-222222222222',
}
function Location() {
  const location = useLocation()
  return <output aria-label="Current location">{location.pathname + location.search}</output>
}
function mount() {
  return render(
    <MemoryRouter initialEntries={['/?project_scope=all']}>
      <NamespacePage />
      <Location />
    </MemoryRouter>,
  )
}
beforeEach(() => {
  vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
  context.mockReturnValue({
    ref,
    malformed: false,
    query: {
      data: {
        label: 'Same project name',
        resource_key: 'SECOND',
        binding: { state: 'active' },
      },
    },
  })
})

it.each(['active', 'archived'])(
  'keeps %s resource links native and bound to the exact namespace',
  async (state) => {
    context.mockReturnValue({
      ref,
      malformed: false,
      query: {
        data: {
          label: 'Same project name',
          resource_key: 'SECOND',
          binding: { state },
        },
      },
    })
    mount()
    const nav = screen.getByRole('navigation', { name: 'Разделы проекта' })
    expect(nav).toHaveClass('base-section-navigation')
    const links = within(nav).getAllByRole('link')
    expect(links).toHaveLength(state === 'active' ? 3 : 2)
    for (const link of links) {
      expect(link).toHaveClass('base-section-navigation-item')
      const url = new URL(link.getAttribute('href')!, 'https://test.invalid')
      expect(url.searchParams.get('registry_instance_id')).toBe(ref.registry_instance_id)
      expect(url.searchParams.get('namespace_id')).toBe(ref.namespace_id)
      expect(url.searchParams.get('project_scope')).toBe('all')
    }
    expect(screen.queryByRole('link', { name: 'Создать задачу' }) !== null).toBe(state === 'active')
    const user = userEvent.setup()
    await user.tab()
    expect(links[0]).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(screen.getByLabelText('Current location').textContent).toMatch(
      /^\/projects\/SECOND\/board\?/,
    )
  },
)

it('offers the shared text link to the catalog only when no namespace was selected', () => {
  context.mockReturnValue({ ref: null, malformed: false, query: {} })
  mount()
  expect(screen.getByRole('link', { name: 'Каталог проектов Tracker' })).toHaveClass(
    'base-text-link',
  )
  expect(screen.queryByRole('navigation', { name: 'Разделы проекта' })).toBeNull()
})

it('does not replace an unavailable binding with project navigation', () => {
  context.mockReturnValue({ ref, malformed: false, query: { isError: true } })
  mount()
  expect(screen.getByRole('alert')).toHaveTextContent('Другой проект не выбран')
  expect(screen.queryByRole('link')).toBeNull()
})
