import { act, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { ReactNode } from 'react'
import {
  useCreateIssueLink,
  useDeleteIssueLink,
  useStartSprint,
  useUpdateIssue,
  useVoteIssue,
  useWatchIssue,
} from './hooks'
import { createIssueLink, deleteIssueLink } from '@/api/link'
import { updateIssue } from '@/api/issue'
import { startSprint } from '@/api/sprint'
import { voteIssue, watchIssue } from '@/api/engagement'

vi.mock('@/api/link', () => ({
  listIssueLinks: vi.fn(),
  createIssueLink: vi.fn(),
  deleteIssueLink: vi.fn(),
}))

vi.mock('@/api/issue', () => ({
  updateIssue: vi.fn(),
  deleteIssue: vi.fn(),
  restoreIssue: vi.fn(),
  purgeIssue: vi.fn(),
  listTrash: vi.fn(),
  getIssue: vi.fn(),
}))

vi.mock('@/api/sprint', () => ({
  listSprints: vi.fn(),
  createSprint: vi.fn(),
  updateSprint: vi.fn(),
  startSprint: vi.fn(),
  closeSprint: vi.fn(),
  moveIssueToSprint: vi.fn(),
  removeIssueFromSprint: vi.fn(),
}))

vi.mock('@/api/engagement', () => ({
  listIssueVotes: vi.fn(),
  listIssueWatchers: vi.fn(),
  voteIssue: vi.fn(),
  unvoteIssue: vi.fn(),
  watchIssue: vi.fn(),
  unwatchIssue: vi.fn(),
}))

function wrapper(client: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
}

describe('shared api hooks', () => {
  afterEach(() => {
    vi.clearAllMocks()
  })

  it('invalidates both source and target issue link lists after creating a link', async () => {
    vi.mocked(createIssueLink).mockResolvedValue({
      id: 'link-1',
      source_id: 'issue-source',
      source_key: 'TT-1',
      target_id: 'issue-target',
      target_key: 'TT-2',
      link_type: 'relates',
    })
    const client = new QueryClient()
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    const { result } = renderHook(() => useCreateIssueLink('issue-source'), {
      wrapper: wrapper(client),
    })

    await act(async () => {
      await result.current.mutateAsync({ targetKey: 'TT-2', linkType: 'relates' })
    })

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue-links', 'issue-source'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue-links', 'issue-target'] })
  })

  it('invalidates all issue link lists after deleting a link', async () => {
    vi.mocked(deleteIssueLink).mockResolvedValue(undefined)
    const client = new QueryClient()
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    const { result } = renderHook(() => useDeleteIssueLink(), {
      wrapper: wrapper(client),
    })

    await act(async () => {
      await result.current.mutateAsync('link-1')
    })

    expect(deleteIssueLink).toHaveBeenCalledWith('link-1')
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue-links'] })
  })

  it('invalidates reports when an issue mutation changes issue-derived data', async () => {
    vi.mocked(updateIssue).mockResolvedValue({
      id: 'issue-1',
      key: 'TT-1',
      summary: 'Issue',
      description: '',
      issue_type: 'task',
      project_key: 'TT',
      status: 'Done',
      status_id: 'status-1',
      priority: 'Medium',
      labels: [],
      assignee_id: null,
      assignee_name: null,
      reporter_id: 'user-1',
      reporter_name: 'Demo User',
      project_name: 'Task Tracker',
      sprint_id: null,
      original_estimate_seconds: null,
      remaining_estimate_seconds: null,
      time_spent_seconds: 0,
    })
    const client = new QueryClient()
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    const { result } = renderHook(() => useUpdateIssue('issue-1'), {
      wrapper: wrapper(client),
    })

    await act(async () => {
      await result.current.mutateAsync({ status_id: 'status-1' })
    })

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['reports'] })
  })

  it('invalidates reports when a sprint lifecycle mutation changes report scope', async () => {
    vi.mocked(startSprint).mockResolvedValue({
      id: 'sprint-1',
      name: 'Sprint 1',
      goal: '',
      state: 'active',
      velocity: 0,
      remaining_days: null,
      issue_ids: [],
      start_date: '2026-09-01T10:00:00Z',
      end_date: null,
    })
    const client = new QueryClient()
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    const { result } = renderHook(() => useStartSprint('TT'), {
      wrapper: wrapper(client),
    })

    await act(async () => {
      await result.current.mutateAsync('sprint-1')
    })

    expect(startSprint).toHaveBeenCalledWith('TT', 'sprint-1')
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['sprints', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['backlog', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['project', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['reports'] })
  })

  it('invalidates issue-derived caches after voting for an issue', async () => {
    vi.mocked(voteIssue).mockResolvedValue({
      user_id: 'user-1',
      username: 'demo',
      display_name: 'Demo User',
      voted_at: '2026-09-01T10:00:00Z',
    })
    const client = new QueryClient()
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    const { result } = renderHook(() => useVoteIssue('issue-1', 'TT'), {
      wrapper: wrapper(client),
    })

    await act(async () => {
      await result.current.mutateAsync()
    })

    expect(voteIssue).toHaveBeenCalledWith('issue-1')
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue-votes', 'issue-1'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['projects'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['dashboard'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['search'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['project', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['backlog', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue', 'issue-1'] })
  })

  it('invalidates issue-derived caches after watching an issue', async () => {
    vi.mocked(watchIssue).mockResolvedValue(undefined)
    const client = new QueryClient()
    const invalidate = vi.spyOn(client, 'invalidateQueries')
    const { result } = renderHook(() => useWatchIssue('issue-1', 'TT'), {
      wrapper: wrapper(client),
    })

    await act(async () => {
      await result.current.mutateAsync()
    })

    expect(watchIssue).toHaveBeenCalledWith('issue-1')
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue-watchers', 'issue-1'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['projects'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['dashboard'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['search'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['project', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['backlog', 'TT'] })
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['issue', 'issue-1'] })
  })
})
