import { useEffect, useLayoutEffect, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import {
  getVisibleProjectCatalog,
  visibleProjectKeys,
  type VisibleProjectQuery,
} from '@/api/project-catalog'
import { validityIsCurrent } from '@/api/space-catalog'
import { observeSpacePolicy, useSpaceSession } from '@/shared/auth/space-session'
const emptyIdentity = { issuer: '', subject: '' }

export function useVisibleProjects(query: VisibleProjectQuery = {}, enabled = true) {
  const session = useSpaceSession((state) => state.session)
  const identity = session?.identity ?? emptyIdentity
  const [, expire] = useState(0)
  const active = enabled && Boolean(session && session.expiresAt > Date.now())
  const result = useQuery({
    queryKey: visibleProjectKeys.page(identity, query),
    queryFn: ({ signal }) => getVisibleProjectCatalog(identity, query, signal),
    enabled: active,
    retry: false,
    staleTime: 0,
    gcTime: 60_000,
    refetchOnWindowFocus: 'always',
    refetchOnReconnect: 'always',
    refetchIntervalInBackground: true,
    refetchInterval: (value) => {
      if (value.state.status === 'error') return false
      const until = value.state.data?.validity.valid_until
      return until ? Math.max(1000, until * 1000 - Date.now()) : false
    },
  })
  const value = result.data
  const valid =
    active &&
    !result.isError &&
    Boolean(
      value &&
      validityIsCurrent(value.validity) &&
      value.identity.issuer === identity.issuer &&
      value.identity.subject === identity.subject &&
      (session?.policyRevision === null || value.policy_revision >= (session?.policyRevision ?? 0)),
    )
  useLayoutEffect(() => {
    if (valid && value) observeSpacePolicy(identity, value.policy_revision)
  }, [valid, value, identity])
  const until = value?.validity.valid_until
  useEffect(() => {
    if (!until) return
    const timer = setTimeout(() => expire((n) => n + 1), Math.max(0, until * 1000 - Date.now()))
    return () => clearTimeout(timer)
  }, [until])
  return {
    ...result,
    data: valid ? value : undefined,
    unavailable: !active || result.isError || (result.isSuccess && !valid),
  }
}
