import { act, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { ReactNode } from 'react'
import { useCreateWorklog, useDeleteWorklog, useUpdateWorklog } from './use-worklogs'
import { createWorklog, deleteWorklog, updateWorklog } from '@/api/worklog'
import type { LogWorkInput, Worklog } from '@/entities/worklog/model'

vi.mock('@/api/worklog', () => ({
  listWorklogs: vi.fn(),
  createWorklog: vi.fn(),
  updateWorklog: vi.fn(),
  deleteWorklog: vi.fn(),
}))

function wrapper(client: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  )
}

function seedIssue(client: QueryClient) {
  client.setQueryData(['issue', 'issue-1'], { project_key: 'TT' })
}

function expectIssueCachesRefreshed(client: QueryClient) {
  expect(client.refetchQueries).toHaveBeenCalledWith({
    queryKey: ['worklogs', 'issue-1'],
    exact: true,
  })
  expect(client.refetchQueries).toHaveBeenCalledWith({
    queryKey: ['issue', 'issue-1'],
    exact: true,
  })
  expect(client.invalidateQueries).toHaveBeenCalledWith({ queryKey: ['projects'] })
  expect(client.invalidateQueries).toHaveBeenCalledWith({ queryKey: ['dashboard'] })
  expect(client.invalidateQueries).toHaveBeenCalledWith({ queryKey: ['search'] })
  expect(client.invalidateQueries).toHaveBeenCalledWith({ queryKey: ['reports'] })
  expect(client.invalidateQueries).toHaveBeenCalledWith({ queryKey: ['project', 'TT'] })
  expect(client.invalidateQueries).toHaveBeenCalledWith({ queryKey: ['backlog', 'TT'] })
}

const input: LogWorkInput = {
  timeSpent: '1h',
  startedAt: '2026-09-01T10:00:00.000Z',
  comment: 'Done',
}

const worklog: Worklog = {
  id: 'worklog-1',
  issueId: 'issue-1',
  userId: 'user-1',
  userDisplayName: 'Demo User',
  timeSpentSeconds: 3600,
  startedAt: '2026-09-01T10:00:00.000Z',
  comment: 'Done',
  createdAt: '2026-09-01T10:00:00.000Z',
  updatedAt: '2026-09-01T10:00:00.000Z',
}

describe('worklog hooks', () => {
  afterEach(() => {
    vi.clearAllMocks()
  })

  it('refreshes issue collection caches after creating a worklog', async () => {
    vi.mocked(createWorklog).mockResolvedValue(worklog)
    const client = new QueryClient()
    vi.spyOn(client, 'refetchQueries').mockResolvedValue()
    vi.spyOn(client, 'invalidateQueries').mockResolvedValue()
    seedIssue(client)

    const { result } = renderHook(() => useCreateWorklog('issue-1'), {
      wrapper: wrapper(client),
    })

    await act(async () => {
      await result.current.mutateAsync(input)
    })

    expect(createWorklog).toHaveBeenCalledWith('issue-1', input)
    expectIssueCachesRefreshed(client)
  })

  it('refreshes issue collection caches after updating a worklog', async () => {
    vi.mocked(updateWorklog).mockResolvedValue(worklog)
    const client = new QueryClient()
    vi.spyOn(client, 'refetchQueries').mockResolvedValue()
    vi.spyOn(client, 'invalidateQueries').mockResolvedValue()
    seedIssue(client)

    const { result } = renderHook(() => useUpdateWorklog('issue-1'), {
      wrapper: wrapper(client),
    })

    await act(async () => {
      await result.current.mutateAsync({ id: 'worklog-1', input })
    })

    expect(updateWorklog).toHaveBeenCalledWith('worklog-1', input)
    expectIssueCachesRefreshed(client)
  })

  it('refreshes issue collection caches after deleting a worklog', async () => {
    vi.mocked(deleteWorklog).mockResolvedValue()
    const client = new QueryClient()
    vi.spyOn(client, 'refetchQueries').mockResolvedValue()
    vi.spyOn(client, 'invalidateQueries').mockResolvedValue()
    seedIssue(client)

    const { result } = renderHook(() => useDeleteWorklog('issue-1'), {
      wrapper: wrapper(client),
    })

    await act(async () => {
      await result.current.mutateAsync('worklog-1')
    })

    expect(deleteWorklog).toHaveBeenCalledWith('worklog-1')
    expectIssueCachesRefreshed(client)
  })
})
