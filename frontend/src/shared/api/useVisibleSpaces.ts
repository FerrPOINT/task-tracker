import { useEffect, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useSpaceSession } from '@/shared/auth/space-session'
import {
  catalogIsCurrent,
  getVisibleSpaceCatalog,
  spaceCatalogKeys,
  type VisibleSpaceQuery,
} from '@/api/space-catalog'

export function useVisibleSpaces(query: VisibleSpaceQuery = {}, enabled = true) {
  const session = useSpaceSession((state) => state.session)
  const identity = session?.identity ?? { issuer: '', subject: '' }
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
  const until = result.data?.validity.valid_until
  useEffect(() => {
    if (!until) return
    const timer = setTimeout(() => expire((n) => n + 1), Math.max(0, until * 1000 - Date.now()))
    return () => clearTimeout(timer)
  }, [until])
  const confirmed =
    active && !result.isError && Boolean(result.data && catalogIsCurrent(result.data, identity))
  return {
    ...result,
    data: confirmed ? result.data : undefined,
    unavailable: !active || result.isError || (result.isSuccess && !confirmed),
  }
}
