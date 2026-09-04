// Fleet-standard auth store from @sdlc/ui (token lives in memory only;
// refresh token is HttpOnly-cookie-only).
import { createAuthStore } from '@sdlc/ui/auth'

export const useAuthStore = createAuthStore({
  storageKey: 'task-tracker-auth',
  legacyKeys: ['tt-refresh-token'],
})
export type { AuthState } from '@sdlc/ui/auth'
