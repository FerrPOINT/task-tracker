import { useState } from 'react'
import { useNavigate } from 'react-router'
import { Layers } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { ThemeToggle } from '@/shared/ui/theme-toggle'
import { useRegister } from '@/shared/api/hooks'

export function RegisterPage() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { mutate, isPending, error } = useRegister()
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (password !== confirmPassword) return
    mutate(
      { username, email, password },
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
          {error && <div className="text-sm text-rose-500">{error.message}</div>}
          <Button type="submit" className="w-full" disabled={isPending}>
            {isPending ? `${t('auth.register')}…` : t('auth.register')}
          </Button>
          <Button variant="outline" className="w-full" asChild>
            <a href="/login">{t('auth.haveAccount')}</a>
          </Button>
        </form>
        <p className="mt-4 text-center text-xs text-text-muted">{t('auth.registerDemo')}</p>
      </div>
    </div>
  )
}
