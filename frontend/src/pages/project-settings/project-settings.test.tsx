import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router'
import { beforeEach, expect, it, vi } from 'vitest'
import { ProjectSettingsPage, ProjectGeneralSettingsPage } from './'

const state = vi.hoisted(() => ({
  query: vi.fn(),
  mutation: vi.fn(),
  mutate: vi.fn(),
  reset: vi.fn(),
}))
vi.mock('@/shared/api/hooks', () => ({
  useProjectDetails: state.query,
  useUpdateProject: state.mutation,
}))
vi.mock('@pdlc/ui/ui', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@pdlc/ui/ui')>()),
  ...(await (() => ({ toast: { success: vi.fn() } }))()),
}))
const project = {
  id: 'project-second',
  key: 'SECOND',
  name: 'Same name',
  description: 'Saved description',
}
function settingsView() {
  return (
    <MemoryRouter
      initialEntries={[
        '/projects/SECOND/settings?project_scope=all&registry_instance_id=11111111-1111-4111-8111-111111111111&namespace_id=22222222-2222-4222-8222-222222222222',
      ]}
    >
      <Routes>
        <Route path="/projects/:projectKey/settings" element={<ProjectSettingsPage />}>
          <Route index element={<ProjectGeneralSettingsPage />} />
          <Route path="custom-fields" element={<p>Custom fields outlet</p>} />
        </Route>
      </Routes>
    </MemoryRouter>
  )
}
function renderSettings() {
  return render(settingsView())
}
beforeEach(() => {
  vi.clearAllMocks()
  vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
  state.query.mockReturnValue({ data: project, isPending: false, isError: false })
  state.mutation.mockReturnValue({
    mutate: state.mutate,
    reset: state.reset,
    isPending: false,
    isError: false,
  })
})
it('shows properties and preserves project scope in both settings tabs', () => {
  renderSettings()
  expect(state.query).toHaveBeenCalledWith('SECOND')
  expect(screen.getByLabelText('Ключ')).toHaveAttribute('readonly')
  expect(screen.getByRole('button', { name: 'Сохранить' })).toBeDisabled()
  const href = screen.getByRole('link', { name: 'Кастомные поля' }).getAttribute('href')!
  const url = new URL(href, 'https://test.invalid')
  expect(url.pathname).toBe('/projects/SECOND/settings/custom-fields')
  expect(url.searchParams.get('namespace_id')).toBe('22222222-2222-4222-8222-222222222222')
  expect(url.searchParams.get('project_scope')).toBe('all')
})
it('submits editable properties, clears an empty description, and cancels a draft', async () => {
  renderSettings()
  const user = userEvent.setup()
  await user.clear(screen.getByRole('textbox', { name: 'Название' }))
  await user.type(screen.getByRole('textbox', { name: 'Название' }), 'New name')
  await user.clear(screen.getByLabelText('Описание'))
  await user.click(screen.getByRole('button', { name: 'Сохранить' }))
  expect(state.mutate).toHaveBeenCalledWith(
    { name: 'New name', description: null },
    expect.any(Object),
  )
  await user.click(screen.getByRole('button', { name: 'Отмена' }))
  expect(screen.getByRole('textbox', { name: 'Название' })).toHaveValue('Same name')
  expect(screen.getByLabelText('Описание')).toHaveValue('Saved description')
})
it('retains a draft after a failed save or background project refresh', async () => {
  const view = renderSettings()
  await userEvent.type(screen.getByRole('textbox', { name: 'Название' }), ' draft')
  state.mutation.mockReturnValue({
    mutate: state.mutate,
    reset: state.reset,
    isPending: false,
    isError: true,
  })
  state.query.mockReturnValue({
    data: { ...project, name: 'Background name' },
    isPending: false,
    isError: false,
  })
  view.rerender(settingsView())
  expect(screen.getByRole('alert')).toHaveTextContent('Не удалось сохранить настройки')
  expect(screen.getByRole('textbox', { name: 'Название' })).toHaveValue('Same name draft')
})
it('offers a retry on project load failure instead of an editable empty form', async () => {
  const refetch = vi.fn()
  state.query.mockReturnValue({ isError: true, isPending: false, refetch })
  renderSettings()
  await userEvent.click(screen.getByRole('button', { name: /повторить/i }))
  expect(refetch).toHaveBeenCalledOnce()
  expect(screen.queryByRole('button', { name: 'Сохранить' })).not.toBeInTheDocument()
})

it('saves only the edited description after another user renames the project', async () => {
  const view = renderSettings()
  await userEvent.type(screen.getByLabelText('Описание'), ' local edit')
  state.query.mockReturnValue({
    data: { ...project, name: 'Remote name' },
    isPending: false,
    isError: false,
  })
  view.rerender(settingsView())
  expect(screen.getByRole('textbox', { name: 'Название' })).toHaveValue('Remote name')
  await userEvent.click(screen.getByRole('button', { name: 'Сохранить' }))
  expect(state.mutate).toHaveBeenCalledWith(
    { description: 'Saved description local edit' },
    expect.any(Object),
  )
})

it('saves only the edited name after another user changes the description', async () => {
  const view = renderSettings()
  await userEvent.type(screen.getByRole('textbox', { name: 'Название' }), ' local edit')
  state.query.mockReturnValue({
    data: { ...project, description: 'Remote description' },
    isPending: false,
    isError: false,
  })
  view.rerender(settingsView())
  expect(screen.getByLabelText('Описание')).toHaveValue('Remote description')
  await userEvent.click(screen.getByRole('button', { name: 'Сохранить' }))
  expect(state.mutate).toHaveBeenCalledWith({ name: 'Same name local edit' }, expect.any(Object))
})

it('keeps a pristine form pristine when the background query changes', () => {
  const view = renderSettings()
  state.query.mockReturnValue({
    data: { ...project, name: 'Remote name' },
    isPending: false,
    isError: false,
  })
  view.rerender(settingsView())
  expect(screen.getByRole('textbox', { name: 'Название' })).toHaveValue('Remote name')
  expect(screen.getByRole('button', { name: 'Сохранить' })).toBeDisabled()
})

it('keeps route navigation and selection accessible through Base section items', async () => {
  renderSettings()
  const user = userEvent.setup()
  const general = screen.getByRole('link', { name: 'Общие' })
  const fields = screen.getByRole('link', { name: 'Кастомные поля' })
  expect(general).toHaveClass('base-section-navigation-item')
  expect(fields).toHaveClass('base-section-navigation-item')
  expect(general).toHaveAttribute('aria-current', 'page')
  expect(fields).not.toHaveAttribute('aria-current')
  await user.tab()
  expect(general).toHaveFocus()
  await user.tab()
  expect(fields).toHaveFocus()
  await user.keyboard('{Enter}')
  expect(screen.getByText('Custom fields outlet')).toBeVisible()
  expect(fields).toHaveAttribute('aria-current', 'page')
  expect(general).not.toHaveAttribute('aria-current')
  expect(state.mutate).not.toHaveBeenCalled()
})
