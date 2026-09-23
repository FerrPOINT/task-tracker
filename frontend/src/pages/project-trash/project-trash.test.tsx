import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Routes, Route, useLocation, useNavigate } from 'react-router'
import { toast } from 'sonner'

import { ProjectTrashPage } from './'
import { ThemeProvider } from '@sdlc/ui/lib'

const mockRestore = vi.hoisted(() => vi.fn())
const mockPurge = vi.hoisted(() => vi.fn())
const mockTrash = vi.hoisted(() => vi.fn())
const mockRestoreState = vi.hoisted(() => vi.fn())
const mockPurgeState = vi.hoisted(() => vi.fn())

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

vi.mock('@/shared/api/hooks', () => ({
  useTrash: (...args: unknown[]) => mockTrash(...args),
  useRestoreIssue: () => mockRestoreState(),
  usePurgeIssue: () => mockPurgeState(),
}))

function LocationProbe() {
  const location = useLocation()
  const navigate = useNavigate()

  return (
    <>
      <output aria-label="current location">{`${location.pathname}${location.search}`}</output>
      <button type="button" onClick={() => navigate(-1)}>
        Back in history
      </button>
    </>
  )
}

function wrapper(children: React.ReactNode, url = '/projects/TT/trash') {
  return (
    <ThemeProvider>
      <MemoryRouter initialEntries={[url]}>
        <Routes>
          <Route
            path="/projects/:projectKey/trash"
            element={
              <>
                {children}
                <LocationProbe />
              </>
            }
          />
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
    mockRestoreState.mockReturnValue({ mutate: mockRestore, isPending: false })
    mockPurgeState.mockReturnValue({
      mutate: mockPurge,
      reset: vi.fn(),
      isPending: false,
      error: null,
    })
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
    expect(mockRestore).toHaveBeenCalledWith(
      'i1',
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    )
    mockRestore.mock.calls[0]?.[1].onSuccess()
    expect(toast.success).toHaveBeenCalledWith('Задача TT-1 восстановлена.')
  })

  it('explains a failed restore without exposing the API error', async () => {
    render(wrapper(<ProjectTrashPage />))
    await userEvent.setup().click(screen.getByRole('button', { name: 'Восстановить TT-1' }))
    mockRestore.mock.calls[0]?.[1].onError(new Error('500: failed to restore'))
    expect(toast.error).toHaveBeenCalledWith('Не удалось восстановить TT-1. Повторите попытку.')
  })

  it('purges an issue', async () => {
    render(wrapper(<ProjectTrashPage />))
    await waitFor(() => expect(screen.getByText('Deleted task')).toBeInTheDocument())
    const purgeButton = screen.getByRole('button', { name: /удалить TT-1 навсегда/i })
    await userEvent.click(purgeButton)
    expect(
      screen.getByText('Удалить TT-1 навсегда? Это действие нельзя отменить.'),
    ).toBeInTheDocument()
    const confirmButton = screen.getByRole('button', { name: /подтвердить/i })
    await userEvent.click(confirmButton)
    expect(mockPurge).toHaveBeenCalledWith(
      'i1',
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    )
    mockPurge.mock.calls[0]?.[1].onSuccess()
    expect(toast.success).toHaveBeenCalledWith('Задача TT-1 удалена навсегда.')
  })

  it('keeps a failed purge open with a localized error and clears it on close', async () => {
    const reset = vi.fn()
    mockPurgeState.mockReturnValue({
      mutate: mockPurge,
      reset,
      isPending: false,
      error: new Error('500: failed to purge'),
    })
    const user = userEvent.setup()
    render(wrapper(<ProjectTrashPage />))

    await user.click(screen.getByRole('button', { name: 'Удалить TT-1 навсегда' }))
    expect(reset).toHaveBeenCalledOnce()
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Не удалось удалить задачу навсегда. Повторите попытку.',
    )
    await user.click(screen.getByRole('button', { name: /отмена/i }))
    expect(reset).toHaveBeenCalledTimes(2)
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
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
    expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
      '/projects/TT/trash?page=2',
    )

    await user.click(screen.getByRole('button', { name: 'Back in history' }))
    await waitFor(() => expect(mockTrash).toHaveBeenLastCalledWith('TT', 0, 50))
    expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
      '/projects/TT/trash',
    )
  })

  it('opens a later page directly and preserves unrelated parameters', async () => {
    mockTrash.mockImplementation((_projectKey, offset = 0) => ({
      data: offset === 50 ? [trashIssue(51)] : [],
      isLoading: false,
      error: null,
    }))

    render(wrapper(<ProjectTrashPage />, '/projects/TT/trash?source=qa&page=2'))

    await waitFor(() => expect(mockTrash).toHaveBeenLastCalledWith('TT', 50, 50))
    expect(screen.getByText('Deleted task 51')).toBeInTheDocument()
    expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
      '/projects/TT/trash?source=qa&page=2',
    )
  })

  it('canonicalizes invalid page parameters without losing unrelated parameters', async () => {
    render(wrapper(<ProjectTrashPage />, '/projects/TT/trash?page=-3&source=qa'))

    await waitFor(() =>
      expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
        '/projects/TT/trash?source=qa',
      ),
    )
    expect(mockTrash).toHaveBeenLastCalledWith('TT', 0, 50)
  })

  it('returns an empty out-of-range page directly to the first page', async () => {
    mockTrash.mockImplementation((_projectKey, offset = 0) => ({
      data: offset === 0 ? Array.from({ length: 50 }, (_, index) => trashIssue(index + 1)) : [],
      isLoading: false,
      error: null,
    }))
    mockTrash.mockClear()

    render(wrapper(<ProjectTrashPage />, '/projects/TT/trash?page=999&source=qa'))

    expect(mockTrash).toHaveBeenCalledWith('TT', 49900, 50)
    await waitFor(() =>
      expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
        '/projects/TT/trash?source=qa',
      ),
    )
    await waitFor(() => expect(screen.getByText('1–50')).toBeInTheDocument())
    const offsetTransitions = mockTrash.mock.calls
      .map(([, offset]) => offset)
      .filter((offset, index, offsets) => index === 0 || offset !== offsets[index - 1])
    expect(offsetTransitions).toEqual([49900, 0])
  })
})
