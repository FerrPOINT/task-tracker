import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Routes, Route } from 'react-router'

import { ProjectTrashPage } from './'
import { ThemeProvider } from '@/shared/lib/theme'

const mockRestore = vi.hoisted(() => vi.fn())
const mockPurge = vi.hoisted(() => vi.fn())
const mockTrash = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', () => ({
  useTrash: (...args: unknown[]) => mockTrash(...args),
  useRestoreIssue: () => ({
    mutate: mockRestore,
    isPending: false,
  }),
  usePurgeIssue: () => ({
    mutate: mockPurge,
    isPending: false,
  }),
}))

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter initialEntries={['/projects/TT/trash']}>
        <Routes>
          <Route path="/projects/:projectKey/trash" element={children} />
        </Routes>
      </MemoryRouter>
    </ThemeProvider>
  )
}

describe('ProjectTrashPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockTrash.mockReturnValue({
      data: [
        {
          id: 'i1',
          key: 'TT-1',
          summary: 'Deleted task',
          issue_type: 'Task',
          priority: 'High',
          status: 'todo',
          status_id: 'todo',
          project_key: 'TT',
          project_name: 'Task Tracker',
          description: '',
          labels: [],
          reporter_id: 'u1',
        },
      ],
      isLoading: false,
      error: null,
    })
  })

  it('renders loading state', () => {
    mockTrash.mockReturnValue({
      data: [],
      isLoading: true,
      error: null,
    })
    render(wrapper(<ProjectTrashPage />))
    expect(screen.getByText(/загрузка/i)).toBeInTheDocument()
  })

  it('renders trash list', async () => {
    render(wrapper(<ProjectTrashPage />))
    await waitFor(() => expect(screen.getByText('Deleted task')).toBeInTheDocument())
    expect(screen.getByText('TT-1')).toBeInTheDocument()
  })

  it('restores an issue', async () => {
    render(wrapper(<ProjectTrashPage />))
    await waitFor(() => expect(screen.getByText('Deleted task')).toBeInTheDocument())
    const restoreButton = screen.getByRole('button', { name: /восстановить/i })
    await userEvent.click(restoreButton)
    expect(mockRestore).toHaveBeenCalledWith('i1')
  })

  it('purges an issue', async () => {
    render(wrapper(<ProjectTrashPage />))
    await waitFor(() => expect(screen.getByText('Deleted task')).toBeInTheDocument())
    const purgeButton = screen.getByRole('button', { name: /удалить навсегда/i })
    await userEvent.click(purgeButton)
    const confirmButton = screen.getByRole('button', { name: /подтвердить/i })
    await userEvent.click(confirmButton)
    expect(mockPurge).toHaveBeenCalledWith('i1')
  })
})
