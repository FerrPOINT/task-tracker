import { useMemo } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useScopedMutation } from '@/shared/api/useScopedMutation'
import { useAuthStore } from '@/shared/auth/store'
import { useSpaceSession } from '@/shared/auth/space-session'
import { handoffKeys } from '@/shared/api/task-handoff-keys'
import {
  getHandoffPreview,
  requestHandoff,
  cancelHandoff,
  type HandoffCommand,
  type HandoffCancel,
} from '@/api/task-handoff'

export function useHandoffPreview(id: string, enabled: boolean) {
  const token = useAuthStore((state) => state.token)
  const contextEpoch = useSpaceSession((state) => state.session?.contextEpoch)
  const policyEpoch = useSpaceSession((state) => state.session?.policyEpoch)
  // Bearers never become query keys; each credential/data context retires its own view.
  const scope = useMemo(
    () => ({ authenticated: Boolean(token), contextEpoch, policyEpoch, key: crypto.randomUUID() }),
    [token, contextEpoch, policyEpoch],
  )
  const query = useQuery({
    queryKey: handoffKeys.view(id, scope.key),
    queryFn: ({ signal }) => getHandoffPreview(id, signal),
    enabled: enabled && scope.authenticated && Boolean(id),
    retry: false,
    gcTime: 0,
  })
  return { query, scope }
}
function useInvalidateHandoff() {
  const cache = useQueryClient()
  return (id: string) => {
    for (const key of [
      handoffKeys.task(id),
      ['task-lifecycle', id],
      ['issue', id],
      ['project-stage-board'],
    ])
      void cache.invalidateQueries({ queryKey: key })
  }
}
export function useRequestHandoff() {
  const invalidate = useInvalidateHandoff()
  return useScopedMutation({
    mutationFn: ({ id, command }: { id: string; command: HandoffCommand }) =>
      requestHandoff(id, command),
    retry: false,
    onSettled: (_data, _error, { id }) => invalidate(id),
  })
}
export function useCancelHandoff() {
  const invalidate = useInvalidateHandoff()
  return useScopedMutation({
    mutationFn: ({
      id,
      handoff,
      command,
    }: {
      id: string
      handoff: string
      command: HandoffCancel
    }) => cancelHandoff(id, handoff, command),
    retry: false,
    onSettled: (_data, _error, { id }) => invalidate(id),
  })
}
