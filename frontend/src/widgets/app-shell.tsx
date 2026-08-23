import { useState } from 'react'
import { Link, useLocation, Outlet } from 'react-router'
import {
  Layers,
  LayoutDashboard,
  FolderKanban,
  Search,
  List,
  Columns2,
  Trash2,
  Bell,
  User,
  Plus,
  ChevronDown,
  Menu,
  X,
  LogOut,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import { ThemeToggle } from '@/shared/ui/theme-toggle'
import { useCurrentUser, useLogout } from '@/shared/api/hooks'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/shared/ui/dropdown-menu'

// /trash навигация появится вместе с фазой архива (soft-delete)
const systemItems: Array<{ to: string; icon: typeof Trash2; labelKey: string }> = []

const projectKeyPattern = /^\/projects\/([^/]+)\/(board|backlog)$/

function useCurrentProjectKey() {
  const location = useLocation()
  const match = location.pathname.match(projectKeyPattern)
  return match?.[1]
}

function SidebarLink({
  to,
  icon: Icon,
  label,
  active,
  onClick,
}: {
  to: string
  icon: React.ElementType
  label: string
  active: boolean
  onClick?: () => void
}) {
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
  const { t } = useTranslation()
  const location = useLocation()
  const projectKey = useCurrentProjectKey()
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false)
  const { data: user } = useCurrentUser()
  const logout = useLogout()

  const navItems = [
    { to: '/', icon: LayoutDashboard, labelKey: 'navigation.dashboard' },
    { to: '/projects', icon: FolderKanban, labelKey: 'navigation.projects' },
    { to: '/search', icon: Search, labelKey: 'navigation.search' },
  ]

  const projectItems = [
    { to: `/projects/${projectKey ?? 'TT'}/backlog`, icon: List, labelKey: 'navigation.backlog' },
    { to: `/projects/${projectKey ?? 'TT'}/board`, icon: Columns2, labelKey: 'navigation.board' },
  ]

  function isActive(path: string) {
    if (path === '/') return location.pathname === '/'
    if (path.startsWith('/projects/') && projectKey) {
      return location.pathname.startsWith(`/projects/${projectKey}/`)
    }
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
            {mobileMenuOpen ? (
              <X className="h-[18px] w-[18px]" />
            ) : (
              <Menu className="h-[18px] w-[18px]" />
            )}
          </Button>
          <Link to="/" className="flex items-center gap-2 font-bold">
            <Layers className="h-[18px] w-[18px] text-accent" />
            <span className="hidden sm:inline">TaskTracker</span>
          </Link>
          <Link
            to="/projects"
            className="hidden items-center gap-1 rounded-md px-2 py-1 text-sm text-text-secondary hover:bg-surface-raised hover:text-text-primary sm:flex"
          >
            <span>{t('navigation.projects')}</span>
            <ChevronDown className="h-3.5 w-3.5" />
          </Link>
          <Link
            to="/search"
            className="hidden items-center gap-2 rounded-md px-2 py-1 text-sm text-text-secondary hover:bg-surface-raised hover:text-text-primary sm:flex"
          >
            <Search className="h-4 w-4" />
            <span>{t('navigation.search')}</span>
          </Link>
        </div>
        <div className="flex items-center gap-2 md:gap-3">
          <Button asChild size="sm" className="h-7 gap-1 px-2.5 text-xs">
            <Link to="/issues/create">
              <Plus className="h-3.5 w-3.5" />
              <span className="hidden sm:inline">{t('navigation.create')}</span>
            </Link>
          </Button>
          <ThemeToggle />
          <Button variant="ghost" size="icon" className="hidden h-8 w-8 sm:inline-flex">
            <Bell className="h-[18px] w-[18px]" />
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon" className="h-8 w-8">
                <User className="h-[18px] w-[18px]" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-56">
              <div className="px-2 py-1.5 text-sm font-medium text-text-primary">
                {user?.display_name ?? user?.email ?? 'User'}
              </div>
              <div className="px-2 pb-2 text-xs text-text-muted">{user?.email}</div>
              <DropdownMenuItem onClick={() => logout.mutate()} className="gap-2 text-text-secondary">
                <LogOut className="h-4 w-4" />
                <span>{t('navigation.logout')}</span>
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
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
              label={t(item.labelKey)}
              active={isActive(item.to)}
            />
          ))}

          <div className="mt-3 px-3 text-xs font-medium uppercase tracking-wider text-text-muted">
            Task Tracker · {projectKey ?? 'TT'}
          </div>
          {projectItems.map((item) => (
            <SidebarLink
              key={item.labelKey}
              to={item.to}
              icon={item.icon}
              label={t(item.labelKey)}
              active={isActive(item.to)}
            />
          ))}

          <div className="mt-3 px-3 text-xs font-medium uppercase tracking-wider text-text-muted">
            {t('navigation.system')}
          </div>
          {systemItems.map((item) => (
            <SidebarLink
              key={item.to}
              to={item.to}
              icon={item.icon}
              label={t(item.labelKey)}
              active={isActive(item.to)}
            />
          ))}
        </aside>

        {/* Mobile menu overlay */}
        {mobileMenuOpen && (
          <div className="fixed inset-0 z-40 md:hidden">
            <div
              className="absolute inset-0 bg-black/40"
              onClick={() => setMobileMenuOpen(false)}
            />
            <aside className="absolute left-0 top-0 h-full w-64 border-r border-border bg-surface p-3 pt-14 shadow-lg">
              {navItems.map((item) => (
                <SidebarLink
                  key={item.to}
                  to={item.to}
                  icon={item.icon}
                  label={t(item.labelKey)}
                  active={isActive(item.to)}
                  onClick={closeMobileMenu}
                />
              ))}

              <div className="mt-3 px-3 text-xs font-medium uppercase tracking-wider text-text-muted">
                Task Tracker · {projectKey ?? 'TT'}
              </div>
              {projectItems.map((item) => (
                <SidebarLink
                  key={item.labelKey}
                  to={item.to}
                  icon={item.icon}
                  label={t(item.labelKey)}
                  active={isActive(item.to)}
                  onClick={closeMobileMenu}
                />
              ))}

              <div className="mt-3 px-3 text-xs font-medium uppercase tracking-wider text-text-muted">
                {t('navigation.system')}
              </div>
              {systemItems.map((item) => (
                <SidebarLink
                  key={item.to}
                  to={item.to}
                  icon={item.icon}
                  label={t(item.labelKey)}
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
