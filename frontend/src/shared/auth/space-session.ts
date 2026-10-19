import { create } from 'zustand'
import type { SsoSession } from '@pdlc/ui/sso'
import type { CatalogIdentity } from '@/api/space-catalog'
import { ssoConfig, useAuthStore } from './store'

export interface SpaceSession {
  identity: CatalogIdentity
  userId: string
  expiresAt: number
  isSystemAdmin: boolean
}
export const useSpaceSession = create<{ session: SpaceSession | null }>(() => ({ session: null }))
/** Only the validated SSO flow supplies the central subject; local profile IDs remain local. */
export function acceptSpaceSession(
  sso: SsoSession,
  profile: { id: string; is_system_admin?: boolean },
): void {
  const auth = useAuthStore.getState()
  if (
    !sso.subject ||
    !profile.id ||
    auth.userId !== profile.id ||
    auth.token !== sso.accessToken ||
    !Number.isFinite(sso.expiresAt) ||
    sso.expiresAt <= Date.now()
  ) {
    useSpaceSession.setState({ session: null })
    return
  }
  useSpaceSession.setState({
    session: {
      identity: { issuer: ssoConfig.issuer, subject: sso.subject },
      userId: profile.id,
      expiresAt: sso.expiresAt,
      isSystemAdmin: profile.is_system_admin === true,
    },
  })
}
useAuthStore.subscribe((auth) => {
  const session = useSpaceSession.getState().session
  if (session && (!auth.token || session.userId !== auth.userId))
    useSpaceSession.setState({ session: null })
})
