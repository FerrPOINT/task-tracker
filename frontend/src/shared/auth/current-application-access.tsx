import type { ReactNode } from 'react'
import { ApplicationAccessProvider } from '@pdlc/ui/auth'
import { ssoConfig, useAuthStore } from './store'

export function CurrentApplicationAccess({ children }: { children: ReactNode }) {
  const token = useAuthStore((state) => state.token)
  const logout = useAuthStore((state) => state.logout)
  return (
    <ApplicationAccessProvider issuer={ssoConfig.issuer} token={token} onSessionExpired={logout}>
      {children}
    </ApplicationAccessProvider>
  )
}
