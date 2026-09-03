import { useQuery, useMutation, useQueryClient, type QueryClient } from '@tanstack/react-query'
import type { components } from '@/api/generated'
import { listWorklogs, createWorklog, updateWorklog, deleteWorklog } from '@/api/worklog'
import type { Worklog, LogWorkInput } from '@/entities/worklog/model'

const key = (issueId: string) => ['worklogs', issueId]
type IssueCacheItem = Pick<components['schemas']['IssueResponse'], 'project_key'>

function invalidateIssueCollectionCaches(qc: QueryClient, projectKey?: string) {
  qc.invalidateQueries({ queryKey: ['projects'] })
  qc.invalidateQueries({ queryKey: ['dashboard'] })
  qc.invalidateQueries({ queryKey: ['search'] })
  qc.invalidateQueries({ queryKey: ['reports'] })
  if (projectKey) {
    qc.invalidateQueries({ queryKey: ['project', projectKey] })
    qc.invalidateQueries({ queryKey: ['backlog', projectKey] })
  } else {
    qc.invalidateQueries({ queryKey: ['project'] })
    qc.invalidateQueries({ queryKey: ['backlog'] })
  }
}

async function refreshAfterWorklogMutation(qc: QueryClient, issueId: string) {
  await qc.refetchQueries({ queryKey: key(issueId), exact: true })
  await qc.refetchQueries({ queryKey: ['issue', issueId], exact: true })
  const issue = qc.getQueryData<IssueCacheItem>(['issue', issueId])
  invalidateIssueCollectionCaches(qc, issue?.project_key)
}

export function useWorklogs(issueId: string) {
  return useQuery({
    queryKey: key(issueId),
    queryFn: () => listWorklogs(issueId),
  })
}

export function useCreateWorklog(issueId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (input: LogWorkInput) => createWorklog(issueId, input),
    onSuccess: async () => {
      await refreshAfterWorklogMutation(qc, issueId)
    },
  })
}

export function useUpdateWorklog(issueId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: LogWorkInput }) => updateWorklog(id, input),
    onSuccess: async () => {
      await refreshAfterWorklogMutation(qc, issueId)
    },
  })
}

export function useDeleteWorklog(issueId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => deleteWorklog(id),
    onSuccess: async () => {
      await refreshAfterWorklogMutation(qc, issueId)
    },
  })
}

export function totalTimeSpent(worklogs: Worklog[]): number {
  return worklogs.reduce((sum, w) => sum + w.timeSpentSeconds, 0)
}

export function latestRemainingEstimate(_worklogs: Worklog[]): number | null {
  // remaining_estimate_seconds lives on the Issue entity, not on individual worklogs.
  // The caller should read it from the issue DTO instead.
  return null
}
