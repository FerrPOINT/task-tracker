import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter, Routes, Route } from 'react-router'

import { ProjectBacklogPage } from './'
import { ThemeProvider } from '@/shared/lib/theme'

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

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter initialEntries={['/projects/TT/backlog']}>
        <Routes>
          <Route path="/projects/:projectKey/backlog" element={children} />
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
  })

  it('renders sprint list', async () => {
    render(wrapper(<ProjectBacklogPage />))
    await waitFor(() => expect(screen.getByText(/Sprint 1/)).toBeInTheDocument())
    expect(screen.getByText(/Sprint 2/)).toBeInTheDocument()
  })
})
