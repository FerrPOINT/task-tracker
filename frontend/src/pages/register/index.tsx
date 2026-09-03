import { useState } from 'react'
import { Link, useNavigate } from 'react-router'
import { Layers } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@sdlc/ui/ui'
import { ErrorState } from '@sdlc/ui/ui'
import { Input } from '@sdlc/ui/ui'
import { ThemeToggle } from '@sdlc/ui/ui'
import { useRegister } from '@/shared/api/hooks'

export function RegisterPage() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { mutate, isPending, error } = useRegister()
  const [username, setUsername] = useState('')
  const [displayName, setDisplayName] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [passwordError, setPasswordError] = useState('')

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (password !== confirmPassword) {
      setPasswordError(t('auth.passwordMismatch'))
      return
    }
    setPasswordError('')
    mutate(
      { username, email, password, name: displayName || username },
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
            <label className="text-sm font-medium" htmlFor="register-username">
              {t('auth.username')}
            </label>
            <Input
              id="register-username"
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              required
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium" htmlFor="register-display-name">
              {t('auth.displayName')}
            </label>
            <Input
              id="register-display-name"
              type="text"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium" htmlFor="register-email">
              {t('auth.email')}
            </label>
            <Input
              id="register-email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium" htmlFor="register-password">
              {t('auth.password')}
            </label>
            <Input
              id="register-password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium" htmlFor="register-confirm">
              {t('auth.confirmPassword')}
            </label>
            <Input
              id="register-confirm"
              type="password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              required
            />
          </div>
          {passwordError && <ErrorState message={passwordError} />}
          {error && <ErrorState message={error.message} />}
          <Button type="submit" className="w-full" disabled={isPending}>
            {isPending ? `${t('auth.register')}…` : t('auth.register')}
          </Button>
          <Button variant="outline" className="w-full" asChild>
            <Link to="/login">{t('auth.haveAccount')}</Link>
          </Button>
        </form>
      </div>
    </div>
  )
}
