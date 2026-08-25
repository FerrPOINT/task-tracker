import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Routes, Route } from 'react-router'

import { ProjectCustomFieldsPage } from './'
import { ThemeProvider } from '@/shared/lib/theme'

const mockCreate = vi.hoisted(() => vi.fn())
const mockDelete = vi.hoisted(() => vi.fn())
const mockFields = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', () => ({
  useProjectCustomFields: (...args: unknown[]) => mockFields(...args),
  useCreateCustomField: () => ({
    mutate: mockCreate,
    isPending: false,
    error: null,
  }),
  useDeleteCustomField: () => ({
    mutate: mockDelete,
    isPending: false,
    error: null,
  }),
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

  it('creates a field', async () => {
    render(wrapper(<ProjectCustomFieldsPage />))
    const nameInput = screen.getByLabelText(/название/i)
    await userEvent.type(nameInput, 'New Field')
    const submit = screen.getByRole('button', { name: /добавить поле/i })
    await userEvent.click(submit)
    expect(mockCreate).toHaveBeenCalled()
  })

  it('deletes a field', async () => {
    render(wrapper(<ProjectCustomFieldsPage />))
    await waitFor(() => expect(screen.getByText('Priority Label')).toBeInTheDocument())
    const deleteButton = screen.getByRole('button', { name: /удалить/i })
    await userEvent.click(deleteButton)
    expect(mockDelete).toHaveBeenCalledWith('cf1')
  })
})