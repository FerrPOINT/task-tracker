import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
import { useTranslation } from 'react-i18next'
import { completeSso } from '@sdlc/ui/sso'
import { Button } from '@sdlc/ui/ui'
import { apiBaseUrl } from '@/api/client'
import { ssoConfig, useAuthStore } from '@/shared/auth/store'

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
    void completion()
      .then(async (session) => {
        const response = await fetch(`${apiBaseUrl}/api/v1/users/me`, {
          headers: { Authorization: `Bearer ${session.accessToken}` },
        })
        if (!response.ok) throw new Error('profile request failed')
        const user = (await response.json()) as {
          id: string
          email: string
          username: string
          display_name: string
        }
        if (!active) return
        useAuthStore.getState().setAuth({
          token: session.accessToken,
          userId: user.id,
          email: user.email,
          username: user.username,
          displayName: user.display_name,
        })
        navigate(session.returnTo, { replace: true })
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
