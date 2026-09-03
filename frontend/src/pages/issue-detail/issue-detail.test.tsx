import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter, Routes, Route } from 'react-router'

import { IssueDetailPage } from './'
import { ThemeProvider } from '@sdlc/ui/lib'
import { useAuthStore } from '@/shared/auth/store'

const mockGetIssue = vi.hoisted(() => vi.fn())
const mockComments = vi.hoisted(() => vi.fn())
const mockWorklogs = vi.hoisted(() => vi.fn())
const mockIssueLinks = vi.hoisted(() => vi.fn())

const issueData = {
  id: 'i1',
  key: 'TT-1',
  summary: 'Test issue summary',
  description: 'Test issue description',
  status: 'In Progress',
  status_id: 'in-progress',
  priority: 'High',
  issue_type: 'Task',
  project_key: 'TT',
  project_name: 'Task Tracker',
  reporter_id: 'u1',
  reporter_name: 'Reporter',
  assignee_id: null,
  assignee_name: null,
  labels: [],
  sprint_id: null,
  original_estimate_seconds: 7200,
  remaining_estimate_seconds: 1800,
  time_spent_seconds: 5400,
}

vi.mock('@/api/issue', () => ({
  getIssue: (...args: unknown[]) => mockGetIssue(...args),
  updateIssue: vi.fn(),
  deleteIssue: vi.fn(),
  restoreIssue: vi.fn(),
  purgeIssue: vi.fn(),
  listTrash: vi.fn(),
}))

vi.mock('@/shared/api/hooks', () => ({
  useIssue: () => ({
    data: issueData,
    isLoading: false,
    error: null,
  }),
  useBoard: () => ({
    data: {
      columns: [
        { id: 'todo', name: 'To Do', wip_limit: null, issue_ids: [] },
        { id: 'done', name: 'Done', wip_limit: null, issue_ids: [] },
      ],
      issues: [],
      project_id: 'p1',
      project_key: 'TT',
      sprint: {
        id: 's1',
        name: 'Sprint 1',
        goal: '',
        state: 'active',
        issue_ids: [],
        velocity: 0,
        remaining_days: null,
        start_date: null,
        end_date: null,
      },
    },
    isLoading: false,
    error: null,
  }),
  useSprints: () => ({
    data: [
      {
        id: 's1',
        name: 'Sprint 1',
        goal: '',
        state: 'active',
        issue_ids: [],
        velocity: 0,
        remaining_days: null,
        start_date: null,
        end_date: null,
      },
    ],
    isLoading: false,
    error: null,
  }),
  useUpdateIssue: () => ({ mutate: vi.fn(), isPending: false }),
  useDeleteIssue: () => ({ mutate: vi.fn(), isPending: false }),
  useCurrentUser: () => ({ data: undefined, isLoading: false }),
  useProjects: () => ({
    data: [{ id: 'p1', key: 'TT', name: 'Task Tracker', owner_id: 'u1' }],
    isLoading: false,
  }),
  useProjectMembers: () => ({ data: { members: [] }, isLoading: false, error: null }),
  useUsers: () => ({ data: [], isLoading: false }),
  useStatuses: () => ({ data: [], isLoading: false }),
  useTransitions: () => ({ data: [], isLoading: false }),
  useProjectLabels: () => ({ data: [], isLoading: false }),
  useIssueLabels: () => ({ data: [], isLoading: false }),
  useAttachLabel: () => ({ mutate: vi.fn(), isPending: false }),
  useDetachLabel: () => ({ mutate: vi.fn(), isPending: false }),
  useCreateLabel: () => ({ mutate: vi.fn(), mutateAsync: vi.fn(), isPending: false }),
  useIssueLinks: (...args: unknown[]) => mockIssueLinks(...args),
  useCreateIssueLink: () => ({ mutate: vi.fn(), mutateAsync: vi.fn(), isPending: false }),
  useDeleteIssueLink: () => ({ mutate: vi.fn(), isPending: false }),
  useIssueVotes: () => ({ data: { count: 0, votes: [] }, isLoading: false }),
  useIssueWatchers: () => ({ data: [], isLoading: false }),
  useVoteIssue: () => ({ mutate: vi.fn(), isPending: false }),
  useUnvoteIssue: () => ({ mutate: vi.fn(), isPending: false }),
  useWatchIssue: () => ({ mutate: vi.fn(), isPending: false }),
  useUnwatchIssue: () => ({ mutate: vi.fn(), isPending: false }),
  useAttachments: () => ({ data: [], isLoading: false }),
  useUploadAttachment: () => ({ mutate: vi.fn(), isPending: false, isError: false }),
  useDeleteAttachment: () => ({ mutate: vi.fn(), isPending: false }),
  useProjectCustomFields: () => ({ data: [], isLoading: false }),
  useIssueCustomFieldValues: () => ({ data: [], isLoading: false }),
  useSetIssueCustomFieldValue: () => ({ mutate: vi.fn(), isPending: false }),
}))

vi.mock('@/features/comments/model/use-comments', () => ({
  useComments: (...args: unknown[]) => mockComments(...args),
  useCreateComment: () => ({ mutate: vi.fn(), isPending: false }),
  useUpdateComment: () => ({ mutate: vi.fn(), isPending: false }),
  useDeleteComment: () => ({ mutate: vi.fn(), isPending: false }),
}))

vi.mock('@/features/time-tracking/model/use-worklogs', () => ({
  useWorklogs: (...args: unknown[]) => mockWorklogs(...args),
  useCreateWorklog: () => ({ mutate: vi.fn(), isPending: false }),
  useUpdateWorklog: () => ({ mutate: vi.fn(), isPending: false }),
  useDeleteWorklog: () => ({ mutate: vi.fn(), isPending: false }),
  totalTimeSpent: () => 0,
  latestRemainingEstimate: () => null,
}))

vi.mock('@/api/attachment', () => ({
  downloadAttachment: vi.fn(),
}))

function wrapper(children: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return (
    <ThemeProvider>
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={['/issues/i1']}>
          <Routes>
            <Route path="/issues/:id" element={children} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>
    </ThemeProvider>
  )
}

const commentData = [
  {
    id: 'c1',
    issueId: 'i1',
    authorId: 'u1',
    authorName: 'Alice',
    body: 'This is a comment',
    createdAt: '2024-01-01T10:00:00Z',
    updatedAt: '2024-01-01T10:00:00Z',
  },
]

describe('IssueDetailPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useAuthStore.setState({ token: 'tok', userId: 'u1', email: 'a@b' })
    mockComments.mockReturnValue({
      data: commentData,
      isLoading: false,
      error: null,
    })
    mockWorklogs.mockReturnValue({
      data: [],
      isLoading: false,
      error: null,
    })
    mockIssueLinks.mockReturnValue({
      data: [],
      isLoading: false,
      error: null,
    })
  })

  it('renders loading state', () => {
    mockGetIssue.mockReturnValue(new Promise(() => {}))
    mockWorklogs.mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
    })
    render(wrapper(<IssueDetailPage />))
    expect(document.querySelector('.animate-spin')).toBeInTheDocument()
  })

  it('renders issue details (summary, description, status)', async () => {
    render(wrapper(<IssueDetailPage />))
    await waitFor(() => expect(screen.getByText('Test issue summary')).toBeInTheDocument())
    expect(screen.getByText('Test issue description')).toBeInTheDocument()
  })

  it('renders comments', async () => {
    render(wrapper(<IssueDetailPage />))
    await waitFor(() => expect(screen.getByText('Test issue summary')).toBeInTheDocument())
    // Comments are in the Activity tab (default) and Comments tab
    expect(screen.getAllByText('This is a comment').length).toBeGreaterThanOrEqual(1)
  })

  it('uses issue time tracking totals instead of deriving the sidebar summary from the worklog page', async () => {
    mockWorklogs.mockReturnValue({
      data: [
        {
          id: 'wl1',
          issueId: 'i1',
          userId: 'u1',
          userDisplayName: 'Alice',
          timeSpentSeconds: 900,
          startedAt: '2024-01-01T10:00:00Z',
          comment: null,
          createdAt: '2024-01-01T10:00:00Z',
          updatedAt: '2024-01-01T10:00:00Z',
        },
      ],
      isLoading: false,
      error: null,
    })

    render(wrapper(<IssueDetailPage />))

    await waitFor(() => expect(screen.getByText('Test issue summary')).toBeInTheDocument())
    const summary = screen.getByTestId('time-tracking-summary')
    expect(summary.textContent).toContain('1h 30m')
    expect(summary.textContent).toContain('2h')
    expect(summary.textContent).toContain('30m')
    expect(summary.textContent).not.toContain('15m')
  })
})
