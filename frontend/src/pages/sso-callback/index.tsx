import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
import { useTranslation } from 'react-i18next'
import { completeSso } from '@sdlc/ui/sso'
import { Button } from '@sdlc/ui/ui'
import { ssoConfig, useAuthStore } from '@/shared/auth/store'
import { loadSsoSession } from '@/shared/auth/session'
import { finishPlatformLogin } from '@sdlc/ui/platform'

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
  const { t } = useTranslation()
  const navigate = useNavigate()
  const [failed, setFailed] = useState(false)
  useEffect(() => {
    let active = true
    const version = useAuthStore.getState().authVersion
    void completion()
      .then(async (session) => {
        if (!active) return
        const applied = await loadSsoSession(session, () => active && useAuthStore.getState().authVersion === version)
        if (!active || !applied) return
        if (!finishPlatformLogin(import.meta.env.VITE_PLATFORM_ORIGIN)) navigate(session.returnTo, { replace: true })
      })
      .catch(() => {
        if (active) setFailed(true)
      })
    return () => {
      active = false
    }
  }, [navigate])
  return (
    <main className="grid min-h-screen place-items-center bg-background p-4">
      {failed ? (
        <div className="space-y-4 text-center">
          <p role="alert" className="text-sm text-danger">
            {t('auth.sso.callbackError')}
          </p>
          <Button className="min-h-11" onClick={() => navigate('/login', { replace: true })}>
            {t('auth.sso.retry')}
          </Button>
        </div>
      ) : (
        <p role="status">{t('auth.sso.completing')}</p>
      )}
    </main>
  )
}
