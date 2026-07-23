import { useState } from 'react'
import { Link, useLocation, Outlet } from 'react-router'
import { Layers, LayoutDashboard, FolderKanban, Search, List, Columns2, Trash2, Bell, User, Plus, ChevronDown, Menu, X } from 'lucide-react'
import { Button } from '@/shared/ui/button'
import { ThemeToggle } from '@/shared/ui/theme-toggle'

const navItems = [
  { to: '/', icon: LayoutDashboard, labelKey: 'Dashboard' },
  { to: '/projects', icon: FolderKanban, labelKey: 'Проекты' },
  { to: '/search', icon: Search, labelKey: 'Мои задачи' },
]

const projectItems = [
  { to: '/projects/project-1/backlog', icon: List, labelKey: 'Backlog' },
  { to: '/projects/project-1/board', icon: Columns2, labelKey: 'Доска' },
]

const systemItems = [
  { to: '/trash', icon: Trash2, labelKey: 'Trash' },
]

function SidebarLink({ to, icon: Icon, label, active, onClick }: { to: string; icon: React.ElementType; label: string; active: boolean; onClick?: () => void }) {
  return (
    <Link
      to={to}
      onClick={onClick}
      className={`flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors ${
        active
          ? 'bg-surface-raised text-text-primary'
          : 'text-text-secondary hover:bg-surface-raised hover:text-text-primary'
      }`}
    >
      <Icon className="h-4 w-4 shrink-0" />
      <span className="truncate">{label}</span>
    </Link>
  )
}

export function AppShell() {
  const location = useLocation()
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false)

  function isActive(path: string) {
    if (path === '/') return location.pathname === '/'
    return location.pathname.startsWith(path)
  }

  function closeMobileMenu() {
    setMobileMenuOpen(false)
  }

  return (
    <div className="min-h-screen bg-background text-text-primary">
      <header className="sticky top-0 z-50 flex h-12 items-center justify-between border-b border-border bg-surface px-3 md:px-4">
        <div className="flex items-center gap-3 md:gap-4">
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8 md:hidden"
            onClick={() => setMobileMenuOpen((v) => !v)}
            aria-label="Toggle menu"
          >
            {mobileMenuOpen ? <X className="h-[18px] w-[18px]" /> : <Menu className="h-[18px] w-[18px]" />}
          </Button>
          <Link to="/" className="flex items-center gap-2 font-bold">
            <Layers className="h-[18px] w-[18px] text-accent" />
            <span className="hidden sm:inline">TaskTracker</span>
          </Link>
          <Link
            to="/projects"
            className="hidden items-center gap-1 rounded-md px-2 py-1 text-sm text-text-secondary hover:bg-surface-raised hover:text-text-primary sm:flex"
          >
            <span>Проекты</span>
            <ChevronDown className="h-3.5 w-3.5" />
          </Link>
          <Link
            to="/search"
            className="hidden items-center gap-2 rounded-md px-2 py-1 text-sm text-text-secondary hover:bg-surface-raised hover:text-text-primary sm:flex"
          >
            <Search className="h-4 w-4" />
            <span>Поиск</span>
          </Link>
        </div>
        <div className="flex items-center gap-2 md:gap-3">
          <Button asChild size="sm" className="h-7 gap-1 px-2.5 text-xs">
            <Link to="/issues/create">
              <Plus className="h-3.5 w-3.5" />
              <span className="hidden sm:inline">Создать</span>
            </Link>
          </Button>
          <ThemeToggle />
          <Button variant="ghost" size="icon" className="hidden h-8 w-8 sm:inline-flex">
            <Bell className="h-[18px] w-[18px]" />
          </Button>
          <Button variant="ghost" size="icon" className="hidden h-8 w-8 sm:inline-flex">
            <User className="h-[18px] w-[18px]" />
          </Button>
        </div>
      </header>

      <div className="flex min-h-[calc(100vh-3rem)]">
        {/* Desktop sidebar */}
        <aside className="hidden w-60 shrink-0 flex-col gap-2 border-r border-border bg-surface p-3 md:flex">
          {navItems.map((item) => (
            <SidebarLink
              key={item.to}
              to={item.to}
              icon={item.icon}
              label={item.labelKey}
              active={isActive(item.to)}
            />
          ))}

          <div className="mt-3 px-3 text-xs font-medium uppercase tracking-wider text-text-muted">Task Tracker</div>
          {projectItems.map((item) => (
            <SidebarLink
              key={item.to}
              to={item.to}
              icon={item.icon}
              label={item.labelKey}
              active={isActive(item.to)}
            />
          ))}

          <div className="mt-3 px-3 text-xs font-medium uppercase tracking-wider text-text-muted">Система</div>
          {systemItems.map((item) => (
            <SidebarLink
              key={item.to}
              to={item.to}
              icon={item.icon}
              label={item.labelKey}
              active={isActive(item.to)}
            />
          ))}
        </aside>

        {/* Mobile menu overlay */}
        {mobileMenuOpen && (
          <div className="fixed inset-0 z-40 md:hidden">
            <div className="absolute inset-0 bg-black/40" onClick={() => setMobileMenuOpen(false)} />
            <aside className="absolute left-0 top-0 h-full w-64 border-r border-border bg-surface p-3 pt-14 shadow-lg">
              {navItems.map((item) => (
                <SidebarLink
                  key={item.to}
                  to={item.to}
                  icon={item.icon}
                  label={item.labelKey}
                  active={isActive(item.to)}
                  onClick={closeMobileMenu}
                />
              ))}

              <div className="mt-3 px-3 text-xs font-medium uppercase tracking-wider text-text-muted">Task Tracker</div>
              {projectItems.map((item) => (
                <SidebarLink
                  key={item.to}
                  to={item.to}
                  icon={item.icon}
                  label={item.labelKey}
                  active={isActive(item.to)}
                  onClick={closeMobileMenu}
                />
              ))}

              <div className="mt-3 px-3 text-xs font-medium uppercase tracking-wider text-text-muted">Система</div>
              {systemItems.map((item) => (
                <SidebarLink
                  key={item.to}
                  to={item.to}
                  icon={item.icon}
                  label={item.labelKey}
                  active={isActive(item.to)}
                  onClick={closeMobileMenu}
                />
              ))}
            </aside>
          </div>
        )}

        <main className="min-w-0 flex-1 p-4 md:p-6">
          <Outlet />
        </main>
      </div>
    </div>
  )
}
