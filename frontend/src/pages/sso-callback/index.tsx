import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
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
  const navigate = useNavigate()
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    let active = true
    void completion()
      .then(async (session) => {
        const response = await fetch(`${apiBaseUrl}/api/v1/users/me`, {
          headers: { Authorization: `Bearer ${session.accessToken}` },
        })
        if (!response.ok) throw new Error('Не удалось открыть профиль Task Tracker.')
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
      .catch((caught) => {
        if (active) setError(caught instanceof Error ? caught.message : 'Не удалось завершить вход')
      })
    return () => {
      active = false
    }
  }, [navigate])
  return (
    <main className="grid min-h-screen place-items-center bg-background p-4">
      {error ? (
        <div className="space-y-4 text-center">
          <p role="alert">{error}</p>
          <Button onClick={() => navigate('/login', { replace: true })}>Повторить вход</Button>
        </div>
      ) : (
        <p role="status">Завершаем вход...</p>
      )}
    </main>
  )
}
