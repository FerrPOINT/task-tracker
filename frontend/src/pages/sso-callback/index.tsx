import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router'

import { completeSso } from '@pdlc/ui/sso'
import { SsoCallbackView } from '@pdlc/ui/ui'
import { ssoConfig, useAuthStore } from '@/shared/auth/store'
import { loadSsoSession } from '@/shared/auth/session'
import { finishPlatformLogin } from '@pdlc/ui/platform'

let pending: ReturnType<typeof completeSso> | null = null
function completion() {
  if (!pending) {
    pending = completeSso(ssoConfig)
    void pending
      .finally(() => {
        pending = null
      })
      .catch(() => undefined)
  }
  return pending
}

export function SsoCallbackPage() {
  const navigate = useNavigate()
  const [failed, setFailed] = useState(false)
  useEffect(() => {
    let active = true
    const version = useAuthStore.getState().authVersion
    void completion()
      .then(async (session) => {
        if (!active) return
        const applied = await loadSsoSession(
          session,
          () => active && useAuthStore.getState().authVersion === version,
        )
        if (!active || !applied) return
        if (!finishPlatformLogin(import.meta.env.VITE_PLATFORM_ORIGIN))
          navigate(session.returnTo, { replace: true })
      })
      .catch(() => {
        if (active) setFailed(true)
      })
    return () => {
      active = false
    }
  }, [navigate])
  return <SsoCallbackView failed={failed} onRetry={() => navigate('/login', { replace: true })} />
}
