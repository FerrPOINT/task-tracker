import { Navigate, useLocation } from 'react-router'
import { useSsoEntry } from '@sdlc/ui/auth'
import { PlatformLogin } from '@sdlc/ui/ui'
import { useAuthStore, ssoConfig } from '@/shared/auth/store'

export function LoginPage() {
  const location = useLocation()
  const token = useAuthStore((state) => state.token)
  const authenticated = Boolean(token)
  const anonymous = !authenticated
  const from = (
    location.state as { from?: { pathname?: string; search?: string; hash?: string } } | null
  )?.from
  const returnTo = from ? `${from.pathname ?? '/'}${from.search ?? ''}${from.hash ?? ''}` : '/'
  const loggedOut = new URLSearchParams(location.search).has('logged_out')
  const entry = useSsoEntry({ config: ssoConfig, returnTo, autoStart: anonymous && !loggedOut })
  if (authenticated) return <Navigate to={returnTo} replace />
  return <PlatformLogin pending={entry.pending} error={entry.error} onSignIn={entry.signIn} />
}
