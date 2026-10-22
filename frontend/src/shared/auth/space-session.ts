import { create } from 'zustand'
import type { SsoSession } from '@pdlc/ui/sso'
import type { CatalogIdentity } from '@/api/space-catalog'
import { ssoConfig, useAuthStore } from './store'

export interface SpaceSession {
  identity: CatalogIdentity
  userId: string
  expiresAt: number
  isSystemAdmin: boolean
  policyRevision: number | null
  policyEpoch: number
  contextKey: string | null
  contextEpoch: number
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
  const previous = useSpaceSession.getState().session
  const same =
    previous !== null &&
    previous.identity.issuer === ssoConfig.issuer &&
    previous.identity.subject === sso.subject &&
    previous.userId === profile.id
  useSpaceSession.setState({
    session: {
      identity: { issuer: ssoConfig.issuer, subject: sso.subject },
      userId: profile.id,
      expiresAt: sso.expiresAt,
      isSystemAdmin: profile.is_system_admin === true,
      policyRevision: same ? previous.policyRevision : null,
      policyEpoch: same ? previous.policyEpoch : 0,
      contextKey: same ? previous.contextKey : null,
      contextEpoch: same ? previous.contextEpoch : 0,
    },
  })
}
useAuthStore.subscribe((auth) => {
  const session = useSpaceSession.getState().session
  if (session && (!auth.token || session.userId !== auth.userId))
    useSpaceSession.setState({ session: null })
})

/** Revisions are monotonic for one central identity; a late page cannot restore an older policy. */
export function observeSpacePolicy(identity: CatalogIdentity, revision: number): void {
  const session = useSpaceSession.getState().session
  if (
    !session ||
    session.expiresAt <= Date.now() ||
    session.identity.issuer !== identity.issuer ||
    session.identity.subject !== identity.subject ||
    !Number.isSafeInteger(revision) ||
    revision < 0 ||
    (session.policyRevision !== null && revision <= session.policyRevision)
  )
    return
  useSpaceSession.setState({
    session: { ...session, policyRevision: revision, policyEpoch: session.policyEpoch + 1 },
  })
}

/** Router-owned explicit context invalidates queued effects, without deriving a Space from a Project. */
export function observeSpaceContext(identity: CatalogIdentity, key: string): void {
  const session = useSpaceSession.getState().session
  if (
    !session ||
    session.expiresAt <= Date.now() ||
    session.identity.issuer !== identity.issuer ||
    session.identity.subject !== identity.subject ||
    session.contextKey === key
  )
    return
  useSpaceSession.setState({
    session: { ...session, contextKey: key, contextEpoch: session.contextEpoch + 1 },
  })
}

/** A paused UI request may resume only for the identity/context that initiated it. */
export function isCurrentSpaceActor(expected: SpaceSession): boolean {
  const current = useSpaceSession.getState().session
  return Boolean(
    current &&
    current.expiresAt > Date.now() &&
    current.identity.issuer === expected.identity.issuer &&
    current.identity.subject === expected.identity.subject &&
    current.userId === expected.userId &&
    current.isSystemAdmin === expected.isSystemAdmin &&
    current.policyEpoch === expected.policyEpoch &&
    current.contextEpoch === expected.contextEpoch &&
    current.contextKey === expected.contextKey &&
    useAuthStore.getState().userId === current.userId &&
    useAuthStore.getState().token,
  )
}
