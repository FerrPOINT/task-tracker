import { Layers } from 'lucide-react'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { ThemeToggle } from '@/shared/ui/theme-toggle'

export function LoginPage() {
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
        <form
          className="space-y-4"
          onSubmit={(e) => {
            e.preventDefault()
            window.location.href = '/'
          }}
        >
          <div className="space-y-2">
            <label className="text-sm font-medium">Логин или email</label>
            <Input type="text" defaultValue="demo" />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Пароль</label>
            <Input type="password" defaultValue="demo" />
          </div>
          <Button type="submit" className="w-full">
            Войти
          </Button>
          <Button variant="outline" className="w-full" asChild>
            <a href="/register">Создать аккаунт</a>
          </Button>
        </form>
        <p className="mt-4 text-center text-xs text-text-muted">MVP-демо: любой логин/пароль.</p>
      </div>
    </div>
  )
}
