import { Layers } from 'lucide-react'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { ThemeToggle } from '@/shared/ui/theme-toggle'

export function RegisterPage() {
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
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="space-y-2">
              <label className="text-sm font-medium">Имя пользователя</label>
              <Input type="text" defaultValue="ivan" />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">Отображаемое имя</label>
              <Input type="text" defaultValue="Иван Петров" />
            </div>
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Email</label>
            <Input type="email" defaultValue="ivan@example.com" />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Пароль</label>
            <Input type="password" placeholder="минимум 8 символов" />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Повтор пароля</label>
            <Input type="password" />
          </div>
          <Button type="submit" className="w-full">
            Зарегистрироваться
          </Button>
          <Button variant="outline" className="w-full" asChild>
            <a href="/login">Уже есть аккаунт</a>
          </Button>
        </form>
        <p className="mt-4 text-center text-xs text-text-muted">MVP-демо: регистрация без подтверждения.</p>
      </div>
    </div>
  )
}
