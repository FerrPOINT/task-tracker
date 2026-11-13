import { useMemo } from 'react'
import type { IssueDetail } from '@/api/issue'
import { useQuery } from '@tanstack/react-query'
import { getSdlcMetadata, getSdlcRoutingPolicy, getSdlcSnapshot } from '@/api/sdlc'
import { useAuthStore } from '@/shared/auth/store'
import { canConfirmRequirementsContext } from './confirmation-context'

export function useSdlcSnapshot(issueId: string, lifecycle: IssueDetail['lifecycle'] | undefined) {
  const token = useAuthStore((state) => state.token)
  // Never reuse a previous credential's scoped data or put a bearer in a query key.
  const session = useMemo(() => ({ token, cacheKey: crypto.randomUUID() }), [token])
  const queryKey = ['sdlc', issueId, session.cacheKey] as const
  const snapshot = useQuery({
    queryKey: [...queryKey, 'snapshot'],
    queryFn: ({ signal }) => getSdlcSnapshot(issueId, signal),
    enabled: Boolean(session.token && issueId && lifecycle?.enrolled === true),
    retry: false,
    gcTime: 0,
  })
  return { snapshot, queryKey, lifecycle, authenticated: Boolean(session.token) }
}

export function useSdlcTask(
  issueId: string,
  after: string,
  lifecycle: IssueDetail['lifecycle'] | undefined,
) {
  const { snapshot, queryKey, authenticated } = useSdlcSnapshot(issueId, lifecycle)
  const forbidden =
    snapshot.error instanceof Error &&
    'status' in snapshot.error &&
    (snapshot.error.status === 401 || snapshot.error.status === 403)
  const metadata = useQuery({
    queryKey: [...queryKey, 'metadata', after],
    queryFn: ({ signal }) => getSdlcMetadata(issueId, after, signal),
    enabled: Boolean(authenticated && lifecycle?.enrolled === true && snapshot.data && !forbidden),
    retry: false,
    gcTime: 0,
  })
  const context = snapshot.data?.context
  const routingPolicy = useQuery({
    queryKey: [...queryKey, 'routing-policy', context?.project_id, context?.tracker_instance_id],
    queryFn: ({ signal }) => getSdlcRoutingPolicy(context!, signal),
    enabled: Boolean(
      authenticated &&
      lifecycle?.enrolled === true &&
      canConfirmRequirementsContext(context) &&
      !forbidden &&
      !snapshot.error &&
      !snapshot.isFetching,
    ),
    retry: false,
    gcTime: 0,
  })
  return { snapshot, metadata, routingPolicy, queryKey, authenticated }
}
