import { ssoNamespace } from '@pdlc/ui/sso'
// Fleet-standard auth store from @pdlc/ui (token lives in memory only;
// refresh token is HttpOnly-cookie-only).
import { createAuthStore } from '@pdlc/ui/auth'

export const ssoConfig = {
  issuer: import.meta.env.VITE_AUTH_ISSUER ?? 'http://localhost:7701',
  clientId: 'task-tracker',
  namespace: ssoNamespace(import.meta.env.VITE_AUTH_NAMESPACE),
}

export const useAuthStore = createAuthStore({
  storageKey: 'task-tracker-auth',
  legacyKeys: ['tt-refresh-token'],
})
export type { AuthState } from '@pdlc/ui/auth'
