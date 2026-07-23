import { useState } from 'react'
import { useNavigate } from 'react-router'
import { Layers } from 'lucide-react'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { ThemeToggle } from '@/shared/ui/theme-toggle'
import { useLogin } from '@/shared/api/hooks'

export function LoginPage() {
  const navigate = useNavigate()
  const { mutate, isPending, error } = useLogin()
  const [email, setEmail] = useState('demo@example.com')
  const [password, setPassword] = useState('demo')

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
            <label className="text-sm font-medium">Email</label>
            <Input type="email" value={email} onChange={(e) => setEmail(e.target.value)} required />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Пароль</label>
            <Input type="password" value={password} onChange={(e) => setPassword(e.target.value)} required />
          </div>
          {error && <div className="text-sm text-rose-500">{error.message}</div>}
          <Button type="submit" className="w-full" disabled={isPending}>
            {isPending ? 'Вход…' : 'Войти'}
          </Button>
          <Button variant="outline" className="w-full" asChild>
            <a href="/register">Создать аккаунт</a>
          </Button>
        </form>
        <p className="mt-4 text-center text-xs text-text-muted">MVP-демо: demo / demo.</p>
      </div>
    </div>
  )
}
