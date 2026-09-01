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

function trashIssue(index: number) {
  return {
    id: `i${index}`,
    key: `TT-${index}`,
    summary: index === 1 ? 'Deleted task' : `Deleted task ${index}`,
    issue_type: 'Task',
    priority: 'High',
    status: 'todo',
    status_id: 'todo',
    project_key: 'TT',
    project_name: 'Task Tracker',
    description: '',
    labels: [],
    reporter_id: 'u1',
  }
}

describe('ProjectTrashPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockTrash.mockReturnValue({
      data: [trashIssue(1)],
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

  it('renders an API error instead of an empty trash state', () => {
    const refetch = vi.fn()
    mockTrash.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('500'),
      refetch,
    })
    render(wrapper(<ProjectTrashPage />))
    expect(screen.getByRole('alert')).toBeInTheDocument()
    expect(screen.queryByText(/корзина пуста/i)).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: /повторить/i })).toBeInTheDocument()
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

  it('requests the next trash page when the current page is full', async () => {
    mockTrash.mockImplementation((_projectKey, offset = 0) => ({
      data:
        offset === 0
          ? Array.from({ length: 50 }, (_, index) => trashIssue(index + 1))
          : [trashIssue(51)],
      isLoading: false,
      error: null,
    }))

    const user = userEvent.setup()
    render(wrapper(<ProjectTrashPage />))

    await waitFor(() => expect(mockTrash).toHaveBeenCalledWith('TT', 0, 50))
    expect(screen.getByText('1–50')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: /вперёд|next/i }))

    await waitFor(() => expect(mockTrash).toHaveBeenCalledWith('TT', 50, 50))
    expect(screen.getByText('Deleted task 51')).toBeInTheDocument()
    expect(screen.getByText('51–51')).toBeInTheDocument()
  })
})
