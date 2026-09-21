import { useCallback, useEffect, useState } from 'react'
import { Navigate, useLocation } from 'react-router'
import { useTranslation } from 'react-i18next'
import { beginSso } from '@sdlc/ui/sso'
import { Button, PlatformMark, ThemeToggle } from '@sdlc/ui/ui'
import { ssoConfig, useAuthStore } from '@/shared/auth/store'

export function LoginPage() {
  const { t } = useTranslation()
  const location = useLocation()
  const token = useAuthStore((state) => state.token)
  const destination = (location.state as { from?: { pathname?: string; search?: string } } | null)
    ?.from
  const returnTo = destination ? `${destination.pathname ?? '/'}${destination.search ?? ''}` : '/'
  const loggedOut = new URLSearchParams(location.search).has('logged_out')
  const [loginState, setLoginState] = useState<'ready' | 'redirecting' | 'error'>(
    loggedOut ? 'ready' : 'redirecting',
  )

  const startLogin = useCallback(() => {
    setLoginState('redirecting')
    void beginSso(ssoConfig, returnTo).catch(() => setLoginState('error'))
  }, [returnTo])

  useEffect(() => {
    if (token || loggedOut) return
    startLogin()
  }, [token, loggedOut, startLogin])

  if (token) return <Navigate to={returnTo} replace />
  return (
    <main className="relative grid min-h-screen place-items-center bg-background p-4">
      <div className="absolute right-4 top-4 [&_button]:min-h-11 [&_button]:min-w-11">
        <ThemeToggle />
      </div>
      <div className="w-full max-w-sm space-y-5 text-center">
        <PlatformMark withName />
        <h1 className="text-xl font-semibold">{t('auth.sso.title')}</h1>
        {loginState === 'redirecting' && (
          <p role="status" className="text-sm text-text-muted">
            {t('auth.sso.redirecting')}
          </p>
        )}
        {loginState === 'error' && (
          <p role="alert" className="text-sm text-danger">
            {t('auth.sso.unavailable')}
          </p>
        )}
        <Button
          className="min-h-11 w-full"
          disabled={loginState === 'redirecting'}
          onClick={startLogin}
        >
          {t(loginState === 'error' ? 'auth.sso.retry' : 'auth.sso.signIn')}
        </Button>
        <p className="border-l-2 border-warning bg-surface px-3 py-2 text-left text-sm text-text-secondary">
          {t('auth.sso.securityNotice')}
        </p>
      </div>
    </main>
  )
}
