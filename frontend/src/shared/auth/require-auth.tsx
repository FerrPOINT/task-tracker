// Route guard from @sdlc/ui/auth: one silent refresh, then /login.
import { RequireAuth as Guard } from '@sdlc/ui/auth'
import { refreshAccessToken } from '@/api/client'
import { useAuthStore } from '@/shared/auth/store'

export function RequireAuth() {
  return <Guard store={useAuthStore} refresh={refreshAccessToken} />
}
