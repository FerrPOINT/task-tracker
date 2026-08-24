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
  BarChart3,
  ShieldCheck,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import { ThemeToggle } from '@/shared/ui/theme-toggle'
import { useTrackerEvents } from '@/shared/api/useTrackerEvents'
import {
  useCurrentUser,
  useLogout,
  useMarkAllNotificationsRead,
  useMarkNotificationRead,
  useNotifications,
} from '@/shared/api/hooks'
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
  const { data: notifications = [] } = useNotifications()
  const markNotificationRead = useMarkNotificationRead()
  const markAllNotificationsRead = useMarkAllNotificationsRead()
  useTrackerEvents()
  const logout = useLogout()
  const unreadNotifications = notifications.filter((notification) => !notification.is_read)

  const navItems = [
    { to: '/', icon: LayoutDashboard, labelKey: 'navigation.dashboard' },
    { to: '/projects', icon: FolderKanban, labelKey: 'navigation.projects' },
    { to: '/search', icon: Search, labelKey: 'navigation.search' },
    { to: '/reports', icon: BarChart3, labelKey: 'navigation.reports' },
    { to: '/admin', icon: ShieldCheck, labelKey: 'navigation.admin' },
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
          <Link
            to="/reports"
            className="hidden items-center gap-2 rounded-md px-2 py-1 text-sm text-text-secondary hover:bg-surface-raised hover:text-text-primary sm:flex"
          >
            <BarChart3 className="h-4 w-4" />
            <span>{t('navigation.reports')}</span>
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
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="relative h-8 w-8"
                aria-label={t('notifications.open')}
                data-testid="notification-trigger"
              >
                <Bell className="h-[18px] w-[18px]" />
                {unreadNotifications.length > 0 && (
                  <span className="absolute -right-1 -top-1 min-w-4 rounded-full bg-danger px-1 text-[10px] font-semibold leading-4 text-white">
                    {unreadNotifications.length}
                  </span>
                )}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-80 p-0">
              <div className="flex items-center justify-between border-b border-border px-3 py-2">
                <span className="text-sm font-semibold">{t('notifications.title')}</span>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2"
                  onClick={() => markAllNotificationsRead.mutate()}
                  disabled={unreadNotifications.length === 0 || markAllNotificationsRead.isPending}
                >
                  {t('notifications.markAllRead')}
                </Button>
              </div>
              <div className="max-h-96 overflow-y-auto p-1">
                {notifications.slice(0, 10).map((notification) => (
                  <DropdownMenuItem key={notification.id} className="items-start gap-2 p-2">
                    <div className="min-w-0 flex-1">
                      {notification.action_url ? (
                        <Link to={notification.action_url} className="block hover:text-accent">
                          <div className="truncate font-medium">{notification.title}</div>
                          {notification.body && (
                            <div className="mt-0.5 line-clamp-2 text-xs text-text-muted">
                              {notification.body}
                            </div>
                          )}
                        </Link>
                      ) : (
                        <>
                          <div className="truncate font-medium">{notification.title}</div>
                          {notification.body && (
                            <div className="mt-0.5 line-clamp-2 text-xs text-text-muted">
                              {notification.body}
                            </div>
                          )}
                        </>
                      )}
                    </div>
                    {!notification.is_read && (
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-7 shrink-0 px-2"
                        onClick={(event) => {
                          event.preventDefault()
                          markNotificationRead.mutate(notification.id)
                        }}
                      >
                        {t('notifications.markRead')}
                      </Button>
                    )}
                  </DropdownMenuItem>
                ))}
                {notifications.length === 0 && (
                  <p className="px-3 py-6 text-center text-sm text-text-muted">
                    {t('notifications.empty')}
                  </p>
                )}
              </div>
              <div className="border-t border-border p-1">
                <DropdownMenuItem asChild>
                  <Link to="/notifications" className="justify-center text-accent">
                    {t('notifications.viewAll')}
                  </Link>
                </DropdownMenuItem>
              </div>
            </DropdownMenuContent>
          </DropdownMenu>
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
              <DropdownMenuItem
                onClick={() => logout.mutate()}
                className="gap-2 text-text-secondary"
              >
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
