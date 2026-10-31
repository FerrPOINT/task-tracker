import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  getStageBoard,
  getTaskLifecycleCatalog,
  getTaskLifecycle,
  transitionTaskLifecycle,
  type LifecycleCommand,
} from '@/api/task-lifecycle'

export function useTaskLifecycle(id: string) {
  return useQuery({
    queryKey: ['task-lifecycle', id],
    queryFn: ({ signal }) => getTaskLifecycle(id, signal),
    enabled: !!id,
  })
}
export function useStageBoard(projectKey: string) {
  return useQuery({
    queryKey: ['project-stage-board', projectKey],
    queryFn: ({ signal }) => getStageBoard(projectKey, signal),
    enabled: !!projectKey,
  })
}
export function useLifecycleTransition() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, command }: { id: string; command: LifecycleCommand }) =>
      transitionTaskLifecycle(id, command),
    // Refetch even after a lost response or conflict. Never remove a card optimistically.
    onSettled: (_data, _error, { id }) => {
      for (const key of [
        ['task-lifecycle', id],
        ['project-stage-board'],
        ['issue', id],
        ['project'],
        ['projects'],
        ['backlog'],
        ['search'],
        ['reports'],
        ['dashboard'],
      ])
        void qc.invalidateQueries({ queryKey: key })
    },
  })
}

export function useTaskLifecycleCatalog(enabled = true) {
  return useQuery({
    queryKey: ['task-lifecycle-catalog'],
    enabled,
    queryFn: ({ signal }) => getTaskLifecycleCatalog(signal),
    staleTime: 30_000,
  })
}
