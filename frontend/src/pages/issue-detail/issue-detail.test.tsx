import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter, Routes, Route } from 'react-router'

import { IssueDetailPage } from './'
import { ThemeProvider } from '@sdlc/ui/lib'
import { useAuthStore } from '@/shared/auth/store'
import { toast } from 'sonner'

const mockGetIssue = vi.hoisted(() => vi.fn())
const mockUseIssue = vi.hoisted(() => vi.fn())
const mockComments = vi.hoisted(() => vi.fn())
const mockWorklogs = vi.hoisted(() => vi.fn())
const mockIssueLinks = vi.hoisted(() => vi.fn())
const mockUseUpdateIssue = vi.hoisted(() => vi.fn())
const mockUseDeleteIssue = vi.hoisted(() => vi.fn())

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))

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
  useIssue: (...args: unknown[]) => mockUseIssue(...args),
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
  useUpdateIssue: (...args: unknown[]) => mockUseUpdateIssue(...args),
  useDeleteIssue: (...args: unknown[]) => mockUseDeleteIssue(...args),
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
  afterEach(() => vi.unstubAllGlobals())

  beforeEach(() => {
    vi.clearAllMocks()
    useAuthStore.setState({ token: 'tok', userId: 'u1', email: 'a@b' })
    mockUseIssue.mockReturnValue({ data: issueData, isLoading: false, error: null })
    mockUseUpdateIssue.mockReturnValue({ mutate: vi.fn(), isPending: false })
    mockUseDeleteIssue.mockReturnValue({
      mutate: vi.fn(),
      reset: vi.fn(),
      isPending: false,
      error: null,
    })
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

  it('renders loading state while the issue itself loads', () => {
    mockUseIssue.mockReturnValue({ data: undefined, isLoading: true, error: null })
    render(wrapper(<IssueDetailPage />))
    expect(document.querySelector('.animate-spin')).toBeInTheDocument()
  })

  it('keeps issue details available while worklogs load', () => {
    mockWorklogs.mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
    })
    render(wrapper(<IssueDetailPage />))
    expect(screen.getByText('Test issue summary')).toBeInTheDocument()
    expect(screen.getByText(/загрузка активности|loading activity/i)).toBeInTheDocument()
  })

  it('shows a retryable issue error separately from a genuine 404', async () => {
    const retry = vi.fn()
    mockUseIssue.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('503'),
      refetch: retry,
    })
    const page = render(wrapper(<IssueDetailPage />))

    expect(screen.getByRole('alert')).toHaveTextContent(
      /не удалось загрузить задачу|could not load issue/i,
    )
    expect(screen.queryByText(/задача не найдена|issue not found/i)).not.toBeInTheDocument()
    await userEvent.setup().click(screen.getByRole('button', { name: /повторить|retry/i }))
    expect(retry).toHaveBeenCalledOnce()

    mockUseIssue.mockReturnValue({ data: null, isLoading: false, error: null })
    page.rerender(wrapper(<IssueDetailPage />))
    expect(screen.getByText(/задача не найдена|issue not found/i)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /повторить|retry/i })).not.toBeInTheDocument()
  })

  it('does not disguise failed worklogs as an empty activity feed', async () => {
    const retry = vi.fn()
    mockWorklogs.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('503'),
      refetch: retry,
    })
    render(wrapper(<IssueDetailPage />))

    expect(screen.getByRole('alert')).toHaveTextContent(
      /не удалось загрузить журнал работ|could not load worklogs/i,
    )
    expect(screen.getByText('This is a comment')).toBeInTheDocument()
    expect(screen.queryByText(/активности пока нет|no activity yet/i)).not.toBeInTheDocument()
    await userEvent.setup().click(screen.getByRole('tab', { name: /журнал работ|worklog/i }))
    expect(screen.getByRole('alert')).toHaveTextContent(
      /не удалось загрузить журнал работ|could not load worklogs/i,
    )
    await userEvent.setup().click(screen.getByRole('button', { name: /повторить|retry/i }))
    expect(retry).toHaveBeenCalledOnce()
  })

  it('shows a retryable comments error in the comments tab', async () => {
    const retry = vi.fn()
    mockComments.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('503'),
      refetch: retry,
    })
    render(wrapper(<IssueDetailPage />))

    await userEvent.setup().click(screen.getByRole('tab', { name: /комментарии|comments/i }))
    expect(screen.getByRole('alert')).toHaveTextContent(
      /не удалось загрузить комментарии|could not load comments/i,
    )
    expect(screen.queryByText(/пока нет комментариев|no comments yet/i)).not.toBeInTheDocument()
    await userEvent.setup().click(screen.getByRole('button', { name: /повторить|retry/i }))
    expect(retry).toHaveBeenCalledOnce()
  })

  it('places editable details between the heading and activity in document order', () => {
    render(wrapper(<IssueDetailPage />))
    const heading = screen.getByText('Test issue summary')
    const details = screen.getByText(/^(детали|details)$/i)
    const activity = screen.getByRole('tab', { name: /активность|activity/i })
    expect(heading.compareDocumentPosition(details) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(
      details.compareDocumentPosition(activity) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy()
  })

  it('links the issue back to its project board', () => {
    render(wrapper(<IssueDetailPage />))
    expect(screen.getByRole('link', { name: 'Task Tracker' })).toHaveAttribute(
      'href',
      '/projects/TT/board',
    )
  })

  it('reports clipboard success only after the key is copied', async () => {
    const user = userEvent.setup()
    const writeText = vi.fn().mockResolvedValue(undefined)
    const navigatorWithClipboard = Object.create(navigator)
    Object.defineProperty(navigatorWithClipboard, 'clipboard', { value: { writeText } })
    vi.stubGlobal('navigator', navigatorWithClipboard)
    render(wrapper(<IssueDetailPage />))

    await user.click(screen.getByRole('button', { name: /действия|actions/i }))
    await user.click(screen.getByRole('menuitem', { name: /скопировать ключ|copy key/i }))
    expect(writeText).toHaveBeenCalledWith('TT-1')
    await waitFor(() => expect(toast.success).toHaveBeenCalledWith('Ключ скопирован.'))
    expect(toast.error).not.toHaveBeenCalled()
  })

  it('reports a failed clipboard write without claiming success', async () => {
    const user = userEvent.setup()
    const writeText = vi.fn().mockRejectedValue(new Error('Permission denied'))
    const navigatorWithClipboard = Object.create(navigator)
    Object.defineProperty(navigatorWithClipboard, 'clipboard', { value: { writeText } })
    vi.stubGlobal('navigator', navigatorWithClipboard)
    render(wrapper(<IssueDetailPage />))

    await user.click(screen.getByRole('button', { name: /действия|actions/i }))
    await user.click(screen.getByRole('menuitem', { name: /скопировать ключ|copy key/i }))
    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith('Не удалось скопировать ключ. Повторите попытку.'),
    )
    expect(toast.success).not.toHaveBeenCalled()
  })

  it('reports an assignment failure', async () => {
    const mutate = vi.fn()
    mockUseUpdateIssue.mockReturnValue({ mutate, isPending: false })
    render(wrapper(<IssueDetailPage />))

    await userEvent
      .setup()
      .click(screen.getByRole('button', { name: /назначить на себя|assign to me/i }))
    expect(mutate).toHaveBeenCalledWith(
      { assignee_id: 'u1' },
      expect.objectContaining({ onError: expect.any(Function) }),
    )
    mutate.mock.calls[0]?.[1].onError(new Error('Failed to update issue'))
    expect(toast.error).toHaveBeenCalledWith(
      'Не удалось назначить задачу на вас. Повторите попытку.',
    )
  })

  it('resets deletion errors when opening and closing confirmation', async () => {
    const reset = vi.fn()
    mockUseDeleteIssue.mockReturnValue({
      mutate: vi.fn(),
      reset,
      isPending: false,
      error: new Error('Failed to delete issue'),
    })
    render(wrapper(<IssueDetailPage />))
    const user = userEvent.setup()

    await user.click(screen.getByRole('button', { name: /действия|actions/i }))
    await user.click(screen.getByRole('menuitem', { name: /удалить|delete/i }))
    expect(reset).toHaveBeenCalledOnce()
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Не удалось переместить задачу в корзину. Повторите попытку.',
    )
    await user.click(screen.getByRole('button', { name: /отмена|cancel/i }))
    expect(reset).toHaveBeenCalledTimes(2)
    await waitFor(() => {
      expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
      expect(document.body.style.pointerEvents).not.toBe('none')
    })
  })

  it('keeps desktop details and supplemental actions in one independent sidebar', () => {
    vi.stubGlobal('matchMedia', (query: string) => ({
      matches: query === '(min-width: 1024px)',
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }))
    render(wrapper(<IssueDetailPage />))

    const sidebar = screen.getByRole('complementary')
    expect(sidebar).toHaveClass('sticky')
    expect(within(sidebar).getByText(/^(детали|details)$/i)).toBeInTheDocument()
    expect(within(sidebar).getByText(/^(учёт времени|time tracking)$/i)).toBeInTheDocument()
    expect(within(sidebar).getByText(/^(участие|engagement)$/i)).toBeInTheDocument()
    expect(within(sidebar).queryByRole('tab', { name: /активность|activity/i })).toBeNull()
  })

  it('preserves an unsaved issue draft when crossing the desktop breakpoint', async () => {
    const listeners = new Set<() => void>()
    const media = {
      matches: false,
      addEventListener: (_event: string, listener: () => void) => listeners.add(listener),
      removeEventListener: (_event: string, listener: () => void) => listeners.delete(listener),
    }
    vi.stubGlobal('matchMedia', () => media)
    render(wrapper(<IssueDetailPage />))

    await userEvent.setup().click(screen.getByRole('button', { name: /изменить|edit/i }))
    fireEvent.change(screen.getByDisplayValue('Test issue summary'), {
      target: { value: 'Unsaved draft' },
    })
    act(() => {
      media.matches = true
      listeners.forEach((listener) => listener())
    })

    expect(screen.getByDisplayValue('Unsaved draft')).toBeInTheDocument()
    expect(screen.getByRole('complementary')).toHaveClass('sticky')
  })

  it('describes deletion as recoverable from the trash', async () => {
    const user = userEvent.setup()
    render(wrapper(<IssueDetailPage />))

    await user.click(screen.getByRole('button', { name: /действия|actions/i }))
    await user.click(screen.getByRole('menuitem', { name: /удалить|delete/i }))
    expect(
      screen.getByText(/переместить задачу в корзину|move this issue to the trash/i),
    ).toBeInTheDocument()
    expect(
      screen.queryByText(/без возможности восстановления|permanently/i),
    ).not.toBeInTheDocument()
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
