import { applySsoSession, restoreSso, type SsoSession } from '@pdlc/ui/sso'
import { apiBaseUrl } from '@/api/client'
import { ssoConfig, useAuthStore } from './store'

export async function loadSsoSession(session: SsoSession, canApply = () => true): Promise<boolean> {
  const response = await fetch(`${apiBaseUrl}/api/v1/users/me`, {
    headers: { Authorization: `Bearer ${session.accessToken}` },
  })
  if (!response.ok) throw new Error(`Не удалось открыть профиль Tracker (HTTP ${response.status}).`)
  const user = (await response.json()) as {
    id: string
    email: string
    username: string
    display_name: string
  }
  if (!canApply()) return false
  applySsoSession(session, () =>
    useAuthStore.getState().setAuth({
      token: session.accessToken,
      userId: user.id,
      email: user.email,
      username: user.username,
      displayName: user.display_name,
    }),
  )
  return true
}
let pending: Promise<void> | null = null
export function restoreTrackerSession(): Promise<void> {
  if (!pending) {
    const version = useAuthStore.getState().authVersion
    pending = restoreSso(ssoConfig).then(async (session) => {
      await loadSsoSession(session, () => useAuthStore.getState().authVersion === version)
    })
    void pending
      .finally(() => {
        pending = null
      })
      .catch(() => undefined)
  }
  return pending
}
