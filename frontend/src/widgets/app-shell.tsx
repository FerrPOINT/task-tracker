import { useState } from 'react'
import { Link, useLocation, Outlet } from 'react-router'
import {
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
  Settings2,
  PanelLeftClose,
  PanelLeftOpen,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button, PlatformMark } from '@sdlc/ui/ui'
import { ThemeToggle } from '@sdlc/ui/ui'
import { ServiceSwitcher } from '@sdlc/ui/ui'
import { useTrackerEvents } from '@/shared/api/useTrackerEvents'
import {
  useCurrentUser,
  useLogout,
  useMarkAllNotificationsRead,
  useMarkNotificationRead,
  useIssue,
  useNotifications,
  useProjects,
} from '@/shared/api/hooks'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@sdlc/ui/ui'

const projectKeyPattern = /^\/projects\/([^/]+)(?:\/|$)/
const issuePattern = /^\/issues\/([^/]+)$/

function useCurrentProjectKey() {
  const location = useLocation()
  const match = location.pathname.match(projectKeyPattern)
  const issueMatch = location.pathname.match(issuePattern)
  const isCreateIssueRoute = location.pathname === '/issues/create'
  const issueId = isCreateIssueRoute ? undefined : issueMatch?.[1]
  const createIssueProjectKey = isCreateIssueRoute
    ? (new URLSearchParams(location.search).get('project_key') ??
      (location.state as { project_key?: string } | null)?.project_key)
    : undefined
  // On the issue page the project is not part of the URL; resolve it from
  // the loaded issue so sidebar Board/Backlog/Trash keep their context.
  const { data: issue } = useIssue(issueId ?? '')
  return match?.[1] ?? createIssueProjectKey ?? issue?.project_key
}

function SidebarLink({
  to,
  icon: Icon,
  label,
  active,
  onClick,
  compact = false,
}: {
  to: string
  icon: React.ElementType
  label: string
  active: boolean
  onClick?: () => void
  compact?: boolean
}) {
  return (
    <Link
      to={to}
      onClick={onClick}
      title={compact ? label : undefined}
      className={`flex min-h-10 items-center rounded-md px-3 text-sm transition-colors ${compact ? 'justify-center' : 'gap-3'} ${
        active
          ? 'bg-surface-raised text-text-primary'
          : 'text-text-secondary hover:bg-surface-raised hover:text-text-primary'
      }`}
    >
      <Icon className="h-4 w-4 shrink-0" />
      {!compact && <span className="truncate">{label}</span>}
    </Link>
  )
}

export function AppShell() {
  const { t } = useTranslation()
  const location = useLocation()
  const projectKey = useCurrentProjectKey()
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false)
  const [sidebarCollapsed, setSidebarCollapsed] = useState(
    () =>
      typeof window !== 'undefined' &&
      window.localStorage.getItem('tt-sidebar-collapsed') === 'true',
  )
  const { data: user } = useCurrentUser()
  const { data: projects = [] } = useProjects()
  const { data: notificationList } = useNotifications()
  const markNotificationRead = useMarkNotificationRead()
  const markAllNotificationsRead = useMarkAllNotificationsRead()
  useTrackerEvents()
  const logout = useLogout()
  const notifications = notificationList?.notifications ?? []
  const unreadCount = notificationList?.unread_count ?? 0
  const currentProject = projects.find((project) => project.key === projectKey)

  // Admin link is only for system admins (checked via /auth/me).
  const navItems = [
    { to: '/', icon: LayoutDashboard, labelKey: 'navigation.dashboard' },
    { to: '/projects', icon: FolderKanban, labelKey: 'navigation.projects' },
    { to: '/search', icon: Search, labelKey: 'navigation.search' },
    { to: '/reports', icon: BarChart3, labelKey: 'navigation.reports' },
    ...(user?.is_system_admin
      ? [{ to: '/admin', icon: ShieldCheck, labelKey: 'navigation.admin' }]
      : []),
  ]

  const projectItems = projectKey
    ? [
        { to: `/projects/${projectKey}/board`, icon: Columns2, labelKey: 'navigation.board' },
        { to: `/projects/${projectKey}/backlog`, icon: List, labelKey: 'navigation.backlog' },
        {
          to: `/reports?project_key=${projectKey}`,
          icon: BarChart3,
          labelKey: 'navigation.reports',
        },
        { to: `/projects/${projectKey}/trash`, icon: Trash2, labelKey: 'trash.title' },
        {
          to: `/projects/${projectKey}/settings/custom-fields`,
          icon: Settings2,
          labelKey: 'navigation.settings',
        },
      ]
    : []

  function isActive(path: string) {
    const pathname = path.split('?')[0] ?? path
    if (pathname === '/') return location.pathname === '/'
    if (pathname.startsWith('/projects/') && projectKey) {
      return location.pathname.startsWith(`/projects/${projectKey}/`)
    }
    return location.pathname.startsWith(pathname)
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
            aria-label={t('navigation.toggleMenu')}
          >
            {mobileMenuOpen ? (
              <X className="h-[18px] w-[18px]" />
            ) : (
              <Menu className="h-[18px] w-[18px]" />
            )}
          </Button>
          <Link to="/" className="flex items-center gap-2 font-bold" aria-label={t('app.name')}>
            <PlatformMark size="sm" withName={false} />
            <span className="hidden sm:inline">{t('app.name')}</span>
          </Link>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <button
                type="button"
                className="hidden min-h-10 max-w-52 items-center gap-1 rounded-md px-2 text-sm text-text-secondary hover:bg-surface-raised hover:text-text-primary sm:flex"
              >
                <span className="truncate">{currentProject?.name ?? t('navigation.projects')}</span>
                <ChevronDown className="h-3.5 w-3.5 shrink-0" />
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-64">
              <DropdownMenuItem asChild>
                <Link to="/projects" className="gap-2">
                  <FolderKanban className="h-4 w-4" />
                  Все проекты
                </Link>
              </DropdownMenuItem>
              {projects.map((project) => (
                <DropdownMenuItem key={project.key} asChild>
                  <Link to={`/projects/${project.key}/board`} className="justify-between gap-2">
                    <span className="truncate">{project.name}</span>
                    <span className="text-xs text-text-muted">{project.key}</span>
                  </Link>
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
          <Link
            to="/search"
            className="hidden min-h-10 items-center gap-2 rounded-md px-2 text-sm text-text-secondary hover:bg-surface-raised hover:text-text-primary sm:flex"
          >
            <Search className="h-4 w-4" />
            <span>{t('navigation.search')}</span>
          </Link>
        </div>
        <div className="flex items-center gap-2 md:gap-3">
          <Button asChild size="sm" className="min-h-10 gap-1 px-2.5 text-xs">
            <Link
              to={projectKey ? `/issues/create?project_key=${projectKey}` : '/issues/create'}
              aria-label={t('navigation.create')}
            >
              <Plus className="h-3.5 w-3.5" />
              <span className="hidden sm:inline">{t('navigation.create')}</span>
            </Link>
          </Button>
          <ServiceSwitcher currentKey="task-tracker" />
          <ThemeToggle />
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="relative h-10 w-10"
                aria-label={t('notifications.open')}
                data-testid="notification-trigger"
              >
                <Bell className="h-[18px] w-[18px]" />
                {unreadCount > 0 && (
                  <span className="absolute -right-1 -top-1 min-w-4 rounded-full bg-danger px-1 text-[10px] font-semibold leading-4 text-white">
                    {unreadCount}
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
                  disabled={unreadCount === 0 || markAllNotificationsRead.isPending}
                >
                  {t('notifications.markAllRead')}
                </Button>
              </div>
              <div className="max-h-96 overflow-y-auto p-1">
                {notifications.slice(0, 10).map((notification) => (
                  <DropdownMenuItem key={notification.id} className="items-start gap-2 p-2">
                    <div className="min-w-0 flex-1">
                      {notification.action_url ? (
                        <Link
                          to={notification.action_url}
                          className="block hover:text-accent"
                          onClick={() => {
                            if (!notification.is_read) markNotificationRead.mutate(notification.id)
                          }}
                        >
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
              <Button
                variant="ghost"
                size="icon"
                className="h-10 w-10"
                aria-label={t('navigation.account')}
              >
                <User className="h-[18px] w-[18px]" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-56">
              <div className="px-2 py-1.5 text-sm font-medium text-text-primary">
                {user?.display_name ?? user?.email ?? 'User'}
              </div>
              <div className="px-2 pb-2 text-xs text-text-muted">{user?.email}</div>
              {user?.is_system_admin && (
                <DropdownMenuItem asChild>
                  <Link to="/admin" className="gap-2 text-text-secondary">
                    <ShieldCheck className="h-4 w-4" />
                    <span>{t('navigation.admin')}</span>
                  </Link>
                </DropdownMenuItem>
              )}
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
        {projectKey && (
          <aside
            className={`hidden shrink-0 flex-col gap-2 border-r border-border bg-surface p-3 md:flex ${sidebarCollapsed ? 'w-16' : 'w-60'}`}
          >
            <div
              className={`flex min-h-9 items-center ${sidebarCollapsed ? 'justify-center' : 'justify-between px-2'}`}
            >
              {!sidebarCollapsed && (
                <span className="truncate text-xs font-medium uppercase text-text-muted">
                  {currentProject?.name ?? projectKey}
                </span>
              )}
              <Button
                variant="ghost"
                size="icon"
                className="h-9 w-9"
                aria-label={
                  sidebarCollapsed ? 'Развернуть боковую панель' : 'Свернуть боковую панель'
                }
                onClick={() => {
                  const next = !sidebarCollapsed
                  setSidebarCollapsed(next)
                  window.localStorage.setItem('tt-sidebar-collapsed', String(next))
                }}
              >
                {sidebarCollapsed ? (
                  <PanelLeftOpen className="h-4 w-4" />
                ) : (
                  <PanelLeftClose className="h-4 w-4" />
                )}
              </Button>
            </div>
            {projectItems.map((item) => (
              <SidebarLink
                key={item.labelKey}
                to={item.to}
                icon={item.icon}
                label={t(item.labelKey)}
                active={isActive(item.to)}
                compact={sidebarCollapsed}
              />
            ))}
          </aside>
        )}

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

              {projectKey && (
                <>
                  <div className="mt-4 px-3 text-xs font-medium uppercase text-text-muted">
                    {currentProject?.name ?? projectKey}
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
                </>
              )}
            </aside>
          </div>
        )}

        <main className="min-w-0 flex-1 p-4 md:p-6">
          <div className="mx-auto w-full max-w-7xl">
            <Outlet />
          </div>
        </main>
      </div>
    </div>
  )
}
