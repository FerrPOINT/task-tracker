import { useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { getSdlcMetadata, getSdlcRoutingPolicy, getSdlcSnapshot } from '@/api/sdlc'
import { useAuthStore } from '@/shared/auth/store'

export function useSdlcTask(issueId: string, after: string) {
  const token = useAuthStore((state) => state.token)
  // Never reuse a previous credential's scoped data or put a bearer in a query key.
  const session = useMemo(() => ({ token, cacheKey: crypto.randomUUID() }), [token])
  const queryKey = ['sdlc', issueId, session.cacheKey] as const
  const snapshot = useQuery({
    queryKey: [...queryKey, 'snapshot'],
    queryFn: ({ signal }) => getSdlcSnapshot(issueId, signal),
    enabled: Boolean(session.token && issueId),
    retry: false,
    gcTime: 0,
  })
  const forbidden =
    snapshot.error instanceof Error &&
    'status' in snapshot.error &&
    (snapshot.error.status === 401 || snapshot.error.status === 403)
  const metadata = useQuery({
    queryKey: [...queryKey, 'metadata', after],
    queryFn: ({ signal }) => getSdlcMetadata(issueId, after, signal),
    enabled: Boolean(session.token && snapshot.data && !forbidden),
    retry: false,
    gcTime: 0,
  })
  const context = snapshot.data?.context
  const routingPolicy = useQuery({
    queryKey: [...queryKey, 'routing-policy', context?.project_id, context?.tracker_instance_id],
    queryFn: ({ signal }) => getSdlcRoutingPolicy(context!, signal),
    enabled: Boolean(
      session.token &&
      context?.permissions.can_confirm &&
      context.stage === 'Clarification' &&
      !forbidden &&
      !snapshot.error &&
      !snapshot.isFetching,
    ),
    retry: false,
    gcTime: 0,
  })
  return { snapshot, metadata, routingPolicy, queryKey, authenticated: Boolean(session.token) }
}
