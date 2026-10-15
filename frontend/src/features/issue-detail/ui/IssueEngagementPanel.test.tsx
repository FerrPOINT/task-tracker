import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@pdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { IssueEngagementPanel } from './IssueEngagementPanel'

const mockVotes = vi.hoisted(() => vi.fn())
const mockWatchers = vi.hoisted(() => vi.fn())
const mockVote = vi.hoisted(() => vi.fn())
const mockUnvote = vi.hoisted(() => vi.fn())
const mockWatch = vi.hoisted(() => vi.fn())
const mockUnwatch = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', () => ({
  useIssueVotes: (...args: unknown[]) => mockVotes(...args),
  useIssueWatchers: (...args: unknown[]) => mockWatchers(...args),
  useVoteIssue: () => ({ mutate: mockVote, isPending: false }),
  useUnvoteIssue: () => ({ mutate: mockUnvote, isPending: false }),
  useWatchIssue: () => ({ mutate: mockWatch, isPending: false }),
  useUnwatchIssue: () => ({ mutate: mockUnwatch, isPending: false }),
}))

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter>{children}</MemoryRouter>
    </ThemeProvider>
  )
}

function renderPanel(props?: { currentUserId?: string | null; reporterId?: string | null }) {
  return render(
    wrapper(
      <IssueEngagementPanel
        issueId="issue-1"
        projectKey="TT"
        currentUserId={props?.currentUserId ?? 'user-current'}
        reporterId={props?.reporterId ?? 'user-reporter'}
      />,
    ),
  )
}

describe('IssueEngagementPanel', () => {
  beforeAll(() => {
    i18n.changeLanguage('en')
  })

  beforeEach(() => {
    vi.clearAllMocks()
    mockVotes.mockReturnValue({
      data: {
        count: 2,
        votes: [
          {
            user_id: 'user-current',
            username: 'current',
            display_name: 'Current User',
            voted_at: '2026-09-01T10:00:00Z',
          },
          {
            user_id: 'user-other',
            username: 'other',
            display_name: 'Other User',
            voted_at: '2026-09-01T11:00:00Z',
          },
        ],
      },
      isLoading: false,
    })
    mockWatchers.mockReturnValue({
      data: [
        { user_id: 'user-current', username: 'current', display_name: 'Current User' },
        { user_id: 'user-other', username: 'other', display_name: 'Other User' },
      ],
      isLoading: false,
    })
  })

  it('renders votes and watchers with current user state', () => {
    renderPanel()

    expect(screen.getByTestId('issue-engagement-panel')).toBeInTheDocument()
    expect(screen.getByText('Votes')).toBeInTheDocument()
    expect(screen.getByText('2 total')).toBeInTheDocument()
    expect(screen.getByText('Watchers')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Remove vote' })).toHaveTextContent('Voted')
    expect(screen.getByRole('button', { name: 'Stop watching' })).toHaveTextContent('Watching')
    expect(screen.getByAltText('Current User')).toBeInTheDocument()
  })

  it('toggles vote and watch mutations', () => {
    mockVotes.mockReturnValue({
      data: { count: 1, votes: [] },
      isLoading: false,
    })
    mockWatchers.mockReturnValue({
      data: [],
      isLoading: false,
    })

    renderPanel()

    fireEvent.click(screen.getByRole('button', { name: 'Vote' }))
    fireEvent.click(screen.getByRole('button', { name: 'Watch' }))

    expect(mockVote).toHaveBeenCalledTimes(1)
    expect(mockWatch).toHaveBeenCalledTimes(1)
  })

  it('does not turn a failed vote read into zero or allow a write before readback', () => {
    const refetch = vi.fn()
    mockVotes.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      error: new Error('private upstream failure'),
      refetch,
    })
    const view = renderPanel()

    expect(screen.queryByText('0 total')).not.toBeInTheDocument()
    expect(screen.getByText('Unable to load votes. Try again.')).toBeInTheDocument()
    expect(screen.queryByText('private upstream failure')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Vote' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Stop watching' })).toBeEnabled()
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(refetch).toHaveBeenCalledTimes(1)
    expect(mockVote).not.toHaveBeenCalled()

    mockVotes.mockReturnValue({
      data: { count: 1, votes: [{ user_id: 'user-current' }] },
      isLoading: false,
      isError: false,
      refetch,
    })
    view.rerender(
      wrapper(
        <IssueEngagementPanel issueId="issue-1" projectKey="TT" currentUserId="user-current" />,
      ),
    )
    expect(screen.getByText('1 total')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Remove vote' })).toBeEnabled()
  })

  it('does not turn a failed watcher read into an empty list or allow a write before readback', () => {
    const refetch = vi.fn()
    mockWatchers.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      error: new Error('private upstream failure'),
      refetch,
    })
    const view = renderPanel()

    expect(screen.queryByText(i18n.t('engagement.noWatchers'))).not.toBeInTheDocument()
    expect(screen.getByText('Unable to load watchers. Try again.')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Watch' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Remove vote' })).toBeEnabled()
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(refetch).toHaveBeenCalledTimes(1)
    expect(mockWatch).not.toHaveBeenCalled()

    mockWatchers.mockReturnValue({ data: [], isLoading: false, isError: false, refetch })
    view.rerender(
      wrapper(
        <IssueEngagementPanel issueId="issue-1" projectKey="TT" currentUserId="user-current" />,
      ),
    )
    expect(screen.getByText(i18n.t('engagement.noWatchers'))).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Watch' })).toBeEnabled()
  })

  it('does not allow vote or watch writes before the initial reads finish', () => {
    mockVotes.mockReturnValue({ data: undefined, isLoading: true, isPending: true, isError: false })
    mockWatchers.mockReturnValue({
      data: undefined,
      isLoading: true,
      isPending: true,
      isError: false,
    })
    renderPanel()

    expect(screen.getByRole('button', { name: 'Vote' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Watch' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Vote' }))
    fireEvent.click(screen.getByRole('button', { name: 'Watch' }))
    expect(mockVote).not.toHaveBeenCalled()
    expect(mockWatch).not.toHaveBeenCalled()
  })

  it('does not treat an initial offline-paused read as confirmed empty state', () => {
    mockVotes.mockReturnValue({
      data: undefined,
      isLoading: false,
      isPending: true,
      isError: false,
    })
    mockWatchers.mockReturnValue({
      data: undefined,
      isLoading: false,
      isPending: true,
      isError: false,
    })
    renderPanel()

    expect(screen.queryByText('0 total')).not.toBeInTheDocument()
    expect(screen.queryByText(i18n.t('engagement.noWatchers'))).not.toBeInTheDocument()
    expect(screen.getAllByText(i18n.t('common.loading'))).toHaveLength(2)
    expect(screen.getByRole('button', { name: 'Vote' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Watch' })).toBeDisabled()
  })

  it('prevents voting on own issue but keeps watching available', () => {
    mockVotes.mockReturnValue({
      data: { count: 0, votes: [] },
      isLoading: false,
    })
    mockWatchers.mockReturnValue({
      data: [],
      isLoading: false,
    })

    renderPanel({ currentUserId: 'user-current', reporterId: 'user-current' })

    expect(screen.getByRole('button', { name: 'Vote' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Watch' })).not.toBeDisabled()
  })
})
