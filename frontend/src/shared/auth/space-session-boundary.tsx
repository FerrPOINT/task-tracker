import { Fragment, useEffect, useLayoutEffect, useState, type ReactNode } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { LoadingState } from '@pdlc/ui/ui'
import { useSpaceSession } from './space-session'

export function SpaceSessionBoundary({
  children,
  enabled = import.meta.env.VITE_SPACE_ACCESS_ENABLED === 'true',
}: {
  children: ReactNode
  enabled?: boolean
}) {
  return enabled ? <ActiveBoundary>{children}</ActiveBoundary> : children
}
function ActiveBoundary({ children }: { children: ReactNode }) {
  const session = useSpaceSession((state) => state.session)
  const cache = useQueryClient()
  const key = JSON.stringify(
    session
      ? [
          session.identity.issuer,
          session.identity.subject,
          session.userId,
          session.isSystemAdmin,
          session.policyEpoch,
          session.contextEpoch,
        ]
      : null,
  )
  const [accepted, setAccepted] = useState(key)
  useLayoutEffect(() => {
    if (accepted === key) return
    cache.clear()
    setAccepted(key)
  }, [accepted, key, cache])
  useEffect(() => {
    if (!session) return
    const timer = setTimeout(
      () => {
        if (useSpaceSession.getState().session === session)
          useSpaceSession.setState({ session: null })
      },
      Math.max(0, session.expiresAt - Date.now()),
    )
    return () => clearTimeout(timer)
  }, [session])
  if (accepted !== key) return <LoadingState />
  return <Fragment key={key}>{children}</Fragment>
}
