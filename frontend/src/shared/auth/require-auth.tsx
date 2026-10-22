// Route guard from @pdlc/ui/auth: one silent refresh, then /login.
import { RequireAuth as Guard } from '@pdlc/ui/auth'
import { ssoConfig, useAuthStore } from '@/shared/auth/store'
import { restoreTrackerSession } from './session'

export function RequireAuth() {
  return (
    <Guard
      store={useAuthStore}
      refresh={restoreTrackerSession}
      application={{ issuer: ssoConfig.issuer, key: 'task-tracker' }}
    />
  )
}
