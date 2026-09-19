// Fleet-standard auth store from @sdlc/ui (token lives in memory only;
// refresh token is HttpOnly-cookie-only).
import { createAuthStore } from '@sdlc/ui/auth'

export const ssoConfig = { issuer: import.meta.env.VITE_AUTH_ISSUER ?? 'http://localhost:7701', clientId: 'task-tracker' }

export const useAuthStore = createAuthStore({
  storageKey: 'task-tracker-auth',
  legacyKeys: ['tt-refresh-token'],
})
export type { AuthState } from '@sdlc/ui/auth'
