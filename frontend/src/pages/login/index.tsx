import { useState } from 'react'
import { Link, useNavigate } from 'react-router'
import { Layers } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@sdlc/ui/ui'
import { ErrorState } from '@sdlc/ui/ui'
import { Input } from '@sdlc/ui/ui'
import { ThemeToggle } from '@sdlc/ui/ui'
import { useLogin } from '@/shared/api/hooks'

export function LoginPage() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { mutate, isPending, error } = useLogin()
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    mutate(
      { email, password },
      {
        onSuccess: () => navigate('/'),
      },
    )
  }

  return (
    <div className="relative flex min-h-screen items-center justify-center bg-background p-4">
      <div className="absolute right-4 top-4">
        <ThemeToggle />
      </div>
      <div className="w-full max-w-sm rounded-lg border border-border bg-surface p-6 shadow-sm">
        <div className="mb-6 flex items-center justify-center gap-2 text-xl font-bold">
          <Layers className="h-6 w-6 text-accent" />
          TaskTracker
        </div>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium" htmlFor="login-email">
              {t('auth.email')}
            </label>
            <Input
              id="login-email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium" htmlFor="login-password">
              {t('auth.password')}
            </label>
            <Input
              id="login-password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
            />
          </div>
          {error && <ErrorState message={error.message} />}
          <Button type="submit" className="w-full" disabled={isPending}>
            {isPending ? `${t('auth.login')}…` : t('auth.login')}
          </Button>
          <Button variant="outline" className="w-full" asChild>
            <Link to="/register">{t('auth.createAccount')}</Link>
          </Button>
        </form>
      </div>
    </div>
  )
}
