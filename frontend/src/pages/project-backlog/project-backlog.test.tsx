import { describe, it, expect, vi, beforeEach } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter, Routes, Route, useLocation, useNavigate } from 'react-router'

import { ProjectBacklogPage } from './'
import { ThemeProvider } from '@sdlc/ui/lib'

const mockBacklog = vi.hoisted(() => vi.fn())
const mockSprints = vi.hoisted(() => vi.fn())
const mockMoveIssue = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', () => ({
  useBacklog: (...args: unknown[]) => mockBacklog(...args),
  useSprints: (...args: unknown[]) => mockSprints(...args),
  useMoveIssueToSprint: () => ({
    mutate: mockMoveIssue,
    isPending: false,
  }),
  useRemoveIssueFromSprint: () => ({
    mutate: vi.fn(),
    isPending: false,
  }),
  useCreateSprint: () => ({
    mutate: vi.fn(),
    isPending: false,
    error: null,
  }),
  useUpdateSprint: () => ({
    mutate: vi.fn(),
    isPending: false,
    error: null,
  }),
  useStartSprint: () => ({
    mutate: vi.fn(),
    isPending: false,
  }),
  useCloseSprint: () => ({
    mutate: vi.fn(),
    isPending: false,
  }),
}))

vi.mock('@/features/sprints/ui/SprintFormDialog', () => ({
  SprintFormDialog: () => null,
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

function wrapper(children: React.ReactNode, initialEntry = '/projects/TT/backlog') {
  return (
    <ThemeProvider>
      <MemoryRouter initialEntries={[initialEntry]}>
        <Routes>
          <Route
            path="/projects/:projectKey/backlog"
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

const backlogData = {
  project_id: 'p1',
  project_key: 'TT',
  sprint: {
    id: 's1',
    name: 'Sprint 1',
    goal: '',
    state: 'active',
    issue_ids: [],
    velocity: 10,
    remaining_days: 5,
    start_date: null,
    end_date: null,
  },
  sprint_issues: [
    {
      id: 'i1',
      key: 'TT-1',
      summary: 'Sprint issue',
      priority: 'High',
      issue_type: 'Task',
      status: 'In Progress',
      status_id: 'in-progress',
      project_key: 'TT',
      project_name: 'Task Tracker',
      description: '',
      labels: [],
      reporter_id: 'u1',
      assignee_name: 'Alice',
      sprint_id: 's1',
    },
  ],
  backlog_issues: [
    {
      id: 'i2',
      key: 'TT-2',
      summary: 'Backlog issue',
      priority: 'Medium',
      issue_type: 'Task',
      status: 'Todo',
      status_id: 'todo',
      project_key: 'TT',
      project_name: 'Task Tracker',
      description: '',
      labels: [],
      reporter_id: 'u1',
      assignee_name: 'Bob',
      sprint_id: null,
    },
  ],
}

const sprintsData = [
  {
    id: 's1',
    name: 'Sprint 1',
    goal: '',
    state: 'active',
    issue_ids: [],
    velocity: 10,
    remaining_days: 5,
    start_date: null,
    end_date: null,
  },
  {
    id: 's2',
    name: 'Sprint 2',
    goal: '',
    state: 'planned',
    issue_ids: [],
    velocity: 0,
    remaining_days: null,
    start_date: null,
    end_date: null,
  },
]

describe('ProjectBacklogPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockBacklog.mockReturnValue({
      data: backlogData,
      isLoading: false,
      error: null,
    })
    mockSprints.mockReturnValue({
      data: sprintsData,
      isLoading: false,
      error: null,
    })
  })

  it('renders loading state', () => {
    mockBacklog.mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
    })
    render(wrapper(<ProjectBacklogPage />))
    expect(screen.getByText(/загрузка/i)).toBeInTheDocument()
  })

  it('renders backlog issues', async () => {
    render(wrapper(<ProjectBacklogPage />))
    await waitFor(() => expect(screen.getByText('Backlog issue')).toBeInTheDocument())
    const issueLink = screen.getByRole('link', { name: /TT-2 Backlog issue/i })
    expect(issueLink).toHaveAttribute('href', '/issues/i2')
    expect(issueLink).toHaveAttribute('title', 'Backlog issue')
    expect(issueLink).not.toHaveClass('contents')
    expect(screen.getByRole('button', { name: /действия с задачей TT-2/i })).toBeInTheDocument()
  })

  it('renders sprint list', async () => {
    render(wrapper(<ProjectBacklogPage />))
    await waitFor(() => expect(screen.getByText(/Sprint 1/)).toBeInTheDocument())
    expect(screen.getByText(/Sprint 2/)).toBeInTheDocument()
  })

  it('does not duplicate future sprint issues in the active sprint section', async () => {
    mockBacklog.mockReturnValue({
      data: {
        ...backlogData,
        sprint_issues: [
          ...backlogData.sprint_issues,
          {
            id: 'i3',
            key: 'TT-3',
            summary: 'Future sprint issue',
            priority: 'Low',
            issue_type: 'Task',
            status: 'Todo',
            status_id: 'todo',
            project_key: 'TT',
            project_name: 'Task Tracker',
            description: '',
            labels: [],
            reporter_id: 'u1',
            assignee_name: 'Carol',
            sprint_id: 's2',
          },
        ],
      },
      isLoading: false,
      error: null,
    })

    render(wrapper(<ProjectBacklogPage />))

    await waitFor(() => expect(screen.getByText('Future sprint issue')).toBeInTheDocument())
    expect(screen.getAllByText('Future sprint issue')).toHaveLength(1)
  })

  it('preserves project key on every issue create link', async () => {
    render(wrapper(<ProjectBacklogPage />))
    const links = await screen.findAllByRole('link')
    const createLinks = links.filter((link) =>
      link.getAttribute('href')?.startsWith('/issues/create'),
    )
    expect(createLinks).toHaveLength(2)
    createLinks.forEach((link) => {
      expect(link).toHaveAttribute('href', '/issues/create?project_key=TT')
    })
  })

  it('does not render active sprint actions for the backlog sentinel', async () => {
    mockBacklog.mockReturnValue({
      data: {
        ...backlogData,
        sprint: {
          id: 'none',
          name: 'Backlog',
          goal: '',
          state: 'future',
          issue_ids: [],
          velocity: 0,
          remaining_days: null,
          start_date: null,
          end_date: null,
        },
        sprint_issues: [],
      },
      isLoading: false,
      error: null,
    })
    mockSprints.mockReturnValue({
      data: [],
      isLoading: false,
      error: null,
    })

    render(wrapper(<ProjectBacklogPage />))

    await waitFor(() => expect(screen.getByText('Активного спринта нет')).toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /начать спринт/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /завершить спринт/i })).not.toBeInTheDocument()
  })

  it('loads the requested page from the URL', async () => {
    render(wrapper(<ProjectBacklogPage />, '/projects/TT/backlog?offset=100&view=all'))

    await waitFor(() => expect(mockBacklog).toHaveBeenCalledWith('TT', 100, 100))
    expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
      '/projects/TT/backlog?offset=100&view=all',
    )
  })

  it('keeps pagination in browser history and preserves unrelated query parameters', async () => {
    mockBacklog.mockImplementation((_key: string, offset: number) => ({
      data: {
        ...backlogData,
        backlog_offset: offset,
        backlog_limit: 100,
        backlog_total: 201,
      },
      isLoading: false,
      error: null,
    }))

    render(wrapper(<ProjectBacklogPage />, '/projects/TT/backlog?view=all'))

    fireEvent.click(await screen.findByRole('button', { name: 'Вперёд' }))
    await waitFor(() => expect(mockBacklog).toHaveBeenLastCalledWith('TT', 100, 100))
    expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
      '/projects/TT/backlog?view=all&offset=100',
    )

    fireEvent.click(screen.getByRole('button', { name: 'Back in history' }))
    await waitFor(() => expect(mockBacklog).toHaveBeenLastCalledWith('TT', 0, 100))
    expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
      '/projects/TT/backlog?view=all',
    )
  })

  it('removes an invalid offset while preserving other query parameters', async () => {
    render(wrapper(<ProjectBacklogPage />, '/projects/TT/backlog?offset=-20&view=all'))

    await waitFor(() => expect(mockBacklog).toHaveBeenCalledWith('TT', 0, 100))
    await waitFor(() =>
      expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
        '/projects/TT/backlog?view=all',
      ),
    )
  })
})
