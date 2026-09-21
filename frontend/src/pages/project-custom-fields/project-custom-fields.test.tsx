import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Routes, Route } from 'react-router'
import { toast } from 'sonner'

import { ProjectCustomFieldsPage } from './'
import { ThemeProvider } from '@sdlc/ui/lib'

const mockCreate = vi.hoisted(() => vi.fn())
const mockDelete = vi.hoisted(() => vi.fn())
const mockFields = vi.hoisted(() => vi.fn())
const mockCreateState = vi.hoisted(() => vi.fn())
const mockDeleteState = vi.hoisted(() => vi.fn())

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

vi.mock('@/shared/api/hooks', () => ({
  useProjectCustomFields: (...args: unknown[]) => mockFields(...args),
  useCreateCustomField: () => mockCreateState(),
  useDeleteCustomField: () => mockDeleteState(),
}))

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter initialEntries={['/projects/TT/custom-fields']}>
        <Routes>
          <Route path="/projects/:projectKey/custom-fields" element={children} />
        </Routes>
      </MemoryRouter>
    </ThemeProvider>
  )
}

describe('ProjectCustomFieldsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockCreateState.mockReturnValue({
      mutate: mockCreate,
      reset: vi.fn(),
      isPending: false,
      error: null,
    })
    mockDeleteState.mockReturnValue({
      mutate: mockDelete,
      reset: vi.fn(),
      isPending: false,
      error: null,
    })
    mockFields.mockReturnValue({
      data: [
        {
          id: 'cf1',
          project_id: 'p1',
          name: 'Priority Label',
          field_type: 'text',
          options: [],
          is_required: false,
          created_at: '2024-01-01T00:00:00Z',
        },
      ],
      isLoading: false,
      error: null,
    })
  })

  it('renders loading state', () => {
    mockFields.mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
    })
    render(wrapper(<ProjectCustomFieldsPage />))
    expect(screen.getByText(/загрузка/i)).toBeInTheDocument()
  })

  it('renders field list', async () => {
    render(wrapper(<ProjectCustomFieldsPage />))
    await waitFor(() => expect(screen.getByText('Priority Label')).toBeInTheDocument())
  })

  it('shows a recoverable loading error instead of an empty state', async () => {
    const refetch = vi.fn()
    mockFields.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('500: failed to load'),
      refetch,
    })
    const user = userEvent.setup()
    render(wrapper(<ProjectCustomFieldsPage />))

    expect(screen.getByRole('alert')).toHaveTextContent('Не удалось загрузить поля.')
    await user.click(screen.getByRole('button', { name: /повторить/i }))
    expect(refetch).toHaveBeenCalledOnce()
  })

  it('creates a field', async () => {
    render(wrapper(<ProjectCustomFieldsPage />))
    const nameInput = screen.getByLabelText(/название/i)
    await userEvent.type(nameInput, 'New Field')
    const submit = screen.getByRole('button', { name: /добавить поле/i })
    await userEvent.click(submit)
    expect(mockCreate).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'New Field', field_type: 'text', options: [] }),
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    )
    mockCreate.mock.calls[0]?.[1].onSuccess()
    expect(toast.success).toHaveBeenCalledWith('Поле добавлено.')
  })

  it('keeps commas while typing and submits multiple select options', async () => {
    const user = userEvent.setup()
    render(wrapper(<ProjectCustomFieldsPage />))

    await user.type(screen.getByRole('textbox', { name: 'Название поля' }), '  Team  ')
    await user.selectOptions(screen.getByRole('combobox', { name: 'Тип поля' }), 'select')
    const optionsInput = screen.getByRole('textbox', { name: 'Варианты через запятую' })
    await user.type(optionsInput, 'Первый, Второй, Третий')
    expect(optionsInput).toHaveValue('Первый, Второй, Третий')
    await user.click(screen.getByRole('button', { name: 'Добавить поле' }))

    expect(mockCreate).toHaveBeenCalledWith(
      {
        name: 'Team',
        field_type: 'select',
        options: ['Первый', 'Второй', 'Третий'],
        is_required: false,
      },
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    )
  })

  it('explains a create failure without exposing the API error', () => {
    mockCreateState.mockReturnValue({
      mutate: mockCreate,
      reset: vi.fn(),
      isPending: false,
      error: new Error('500: failed to create'),
    })
    render(wrapper(<ProjectCustomFieldsPage />))
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Не удалось добавить поле. Проверьте данные и повторите попытку.',
    )
  })

  it('deletes a field', async () => {
    render(wrapper(<ProjectCustomFieldsPage />))
    await waitFor(() => expect(screen.getByText('Priority Label')).toBeInTheDocument())
    const deleteButton = screen.getByRole('button', { name: 'Удалить поле Priority Label' })
    await userEvent.click(deleteButton)
    expect(screen.getByRole('alertdialog')).toHaveTextContent('Priority Label')
    await userEvent.click(screen.getByRole('button', { name: /подтвердить/i }))
    expect(mockDelete).toHaveBeenCalledWith(
      'cf1',
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    )
    mockDelete.mock.calls[0]?.[1].onSuccess()
    expect(toast.success).toHaveBeenCalledWith('Поле «Priority Label» удалено.')
  })

  it('keeps a failed delete in the dialog and clears the error on close', async () => {
    const reset = vi.fn()
    mockDeleteState.mockReturnValue({
      mutate: mockDelete,
      reset,
      isPending: false,
      error: new Error('500: failed to delete'),
    })
    const user = userEvent.setup()
    render(wrapper(<ProjectCustomFieldsPage />))

    await user.click(screen.getByRole('button', { name: 'Удалить поле Priority Label' }))
    expect(reset).toHaveBeenCalledOnce()
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Не удалось удалить поле. Повторите попытку.',
    )
    await user.click(screen.getByRole('button', { name: /отмена/i }))
    expect(reset).toHaveBeenCalledTimes(2)
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
  })
})
