import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { listWorklogs, createWorklog, updateWorklog, deleteWorklog } from '@/api/worklog'
import type { Worklog, LogWorkInput } from '@/entities/worklog/model'

const key = (issueId: string) => ['worklogs', issueId]

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
      await qc.refetchQueries({ queryKey: key(issueId), exact: true })
      await qc.refetchQueries({ queryKey: ['issue', issueId], exact: true })
    },
  })
}

export function useUpdateWorklog(issueId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: LogWorkInput }) => updateWorklog(id, input),
    onSuccess: async () => {
      await qc.refetchQueries({ queryKey: key(issueId), exact: true })
      await qc.refetchQueries({ queryKey: ['issue', issueId], exact: true })
    },
  })
}

export function useDeleteWorklog(issueId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => deleteWorklog(id),
    onSuccess: async () => {
      await qc.refetchQueries({ queryKey: key(issueId), exact: true })
      await qc.refetchQueries({ queryKey: ['issue', issueId], exact: true })
    },
  })
}

export function totalTimeSpent(worklogs: Worklog[]): number {
  return worklogs.reduce((sum, w) => sum + w.timeSpentSeconds, 0)
}

export function latestRemainingEstimate(worklogs: Worklog[]): number | null {
  if (worklogs.length === 0) return null
  const sorted = [...worklogs].sort((a, b) => b.startedAt.localeCompare(a.startedAt))
  return sorted[0]?.remainingEstimateSeconds ?? null
}
