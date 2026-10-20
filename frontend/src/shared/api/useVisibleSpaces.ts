import { useEffect, useLayoutEffect, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { observeSpacePolicy, useSpaceSession } from '@/shared/auth/space-session'
import {
  catalogIsCurrent,
  getVisibleSpaceCatalog,
  spaceCatalogKeys,
  type VisibleSpaceQuery,
} from '@/api/space-catalog'

const emptyIdentity = { issuer: '', subject: '' }

export function useVisibleSpaces(query: VisibleSpaceQuery = {}, enabled = true) {
  const session = useSpaceSession((state) => state.session)
  const identity = session?.identity ?? emptyIdentity
  const [, expire] = useState(0)
  const active = enabled && Boolean(session && session.expiresAt > Date.now())
  const result = useQuery({
    queryKey: spaceCatalogKeys.page(identity, query),
    enabled: active,
    queryFn: ({ signal }) => getVisibleSpaceCatalog(identity, query, signal),
    retry: false,
    staleTime: 0,
    gcTime: 60_000,
    refetchOnWindowFocus: 'always',
    refetchOnReconnect: 'always',
    refetchIntervalInBackground: true,
    refetchInterval: (current) => {
      if (current.state.status === 'error') return false
      const until = current.state.data?.validity.valid_until
      return until ? Math.max(1000, until * 1000 - Date.now()) : false
    },
  })
  const observed = result.data
  useLayoutEffect(() => {
    if (active && observed && !result.isError && catalogIsCurrent(observed, identity))
      observeSpacePolicy(identity, observed.policy_revision)
  }, [active, observed, result.isError, identity])
  const until = result.data?.validity.valid_until
  useEffect(() => {
    if (!until) return
    const timer = setTimeout(() => expire((n) => n + 1), Math.max(0, until * 1000 - Date.now()))
    return () => clearTimeout(timer)
  }, [until])
  const confirmed =
    active &&
    !result.isError &&
    Boolean(
      result.data &&
      catalogIsCurrent(result.data, identity) &&
      (session?.policyRevision === null ||
        result.data.policy_revision >= (session?.policyRevision ?? 0)),
    )
  return {
    ...result,
    data: confirmed ? result.data : undefined,
    unavailable: !active || result.isError || (result.isSuccess && !confirmed),
  }
}
