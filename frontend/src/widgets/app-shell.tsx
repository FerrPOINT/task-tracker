import { useEffect, useState } from 'react'
import { Link, useLocation, Outlet } from 'react-router'
import * as DialogPrimitive from '@radix-ui/react-dialog'
import * as DropdownMenuPrimitive from '@radix-ui/react-dropdown-menu'
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
  Check,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button, PageFrame, PlatformMark } from '@sdlc/ui/ui'
import { ThemeToggle } from '@sdlc/ui/ui'
import { PlatformHeader } from '@sdlc/ui/ui'
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
  const reportsProjectKey =
    location.pathname === '/reports'
      ? new URLSearchParams(location.search).get('project_key')
      : undefined
  // On the issue page the project is not part of the URL; resolve it from
  // the loaded issue so sidebar Board/Backlog/Trash keep their context.
  const { data: issue } = useIssue(issueId ?? '')
  return match?.[1] ?? createIssueProjectKey ?? reportsProjectKey ?? issue?.project_key
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
      aria-label={compact ? label : undefined}
      aria-current={active ? 'page' : undefined}
      className={`flex min-h-11 items-center rounded-md px-3 text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus md:min-h-10 ${compact ? 'justify-center' : 'gap-3'} ${
        active
          ? 'bg-surface-raised text-text-primary'
          : 'text-text-secondary hover:bg-surface-raised hover:text-text-primary'
      }`}
    >
      <Icon className="h-4 w-4 shrink-0" aria-hidden="true" />
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
  useEffect(() => {
    if (!window.matchMedia) return
    const desktop = window.matchMedia('(min-width: 768px)')
    const closeOnDesktop = () => {
      if (desktop.matches) setMobileMenuOpen(false)
    }
    desktop.addEventListener('change', closeOnDesktop)
    return () => desktop.removeEventListener('change', closeOnDesktop)
  }, [])
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
  const pageLayout =
    location.pathname === '/issues/create' || location.pathname.endsWith('/settings/custom-fields')
      ? 'reading'
      : /^\/issues\/[^/]+$/.test(location.pathname)
        ? 'detail-with-aside'
        : 'wide'

  const navItems = [
    { to: '/', icon: LayoutDashboard, labelKey: 'navigation.dashboard' },
    { to: '/projects', icon: FolderKanban, labelKey: 'navigation.projects' },
    { to: '/search', icon: Search, labelKey: 'navigation.search' },
    { to: '/reports', icon: BarChart3, labelKey: 'navigation.reports' },
    { to: '/admin', icon: ShieldCheck, labelKey: 'navigation.admin' },
  ]

  const projectItems = projectKey
    ? [
        { to: `/projects/${projectKey}/board`, icon: Columns2, labelKey: 'navigation.board' },
        { to: `/projects/${projectKey}/backlog`, icon: List, labelKey: 'navigation.backlog' },
        {
          to: `/reports?project_key=${projectKey}`,
          icon: BarChart3,
          labelKey: 'navigation.projectReports',
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
    const [pathname, query] = path.split('?')
    if (location.pathname !== pathname) return false
    if (pathname === '/reports') {
      const selectedProject = new URLSearchParams(location.search).get('project_key')
      const targetProject = new URLSearchParams(query).get('project_key')
      return selectedProject === targetProject
    }
    return true
  }

  function closeMobileMenu() {
    setMobileMenuOpen(false)
  }

  return (
    <div className="min-h-screen bg-background text-text-primary">
      <PlatformHeader
        currentServiceKey="task-tracker"
        leading={
          <>
            <DialogPrimitive.Root open={mobileMenuOpen} onOpenChange={setMobileMenuOpen}>
              <DialogPrimitive.Trigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-11 w-11 md:hidden"
                  aria-label={t('navigation.openMenu')}
                >
                  <Menu className="h-[18px] w-[18px]" aria-hidden="true" />
                </Button>
              </DialogPrimitive.Trigger>
              <DialogPrimitive.Portal>
                <DialogPrimitive.Overlay className="fixed inset-0 z-[60] bg-black/50 md:hidden" />
                <DialogPrimitive.Content
                  aria-describedby={undefined}
                  className="fixed inset-y-0 left-0 z-[61] h-dvh w-[calc(100vw-2rem)] max-w-72 overflow-y-auto border-r border-border bg-surface p-3 shadow-lg focus:outline-none md:hidden"
                >
                  <div className="mb-3 flex min-h-11 items-center justify-between gap-2 border-b border-border pb-2">
                    <DialogPrimitive.Title className="text-sm font-semibold">
                      {t('app.name')}
                    </DialogPrimitive.Title>
                    <DialogPrimitive.Close asChild>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-11 w-11"
                        aria-label={t('navigation.closeMenu')}
                      >
                        <X className="h-4 w-4" aria-hidden="true" />
                      </Button>
                    </DialogPrimitive.Close>
                  </div>
                  <Button asChild className="mb-3 min-h-11 w-full gap-2">
                    <Link
                      to={
                        projectKey ? `/issues/create?project_key=${projectKey}` : '/issues/create'
                      }
                      onClick={closeMobileMenu}
                      aria-label={t('navigation.create')}
                    >
                      <Plus className="h-4 w-4" aria-hidden="true" />
                      {t('navigation.create')}
                    </Link>
                  </Button>
                  <nav aria-label={t('navigation.mainNav')}>
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
                  </nav>
                  {projectKey && (
                    <nav
                      aria-label={t('navigation.projectNav')}
                      className="mt-4 border-t border-border pt-3"
                    >
                      <div className="mb-1 truncate px-3 text-xs font-medium uppercase text-text-muted">
                        {currentProject?.name ?? projectKey}
                      </div>
                      {projectItems.map((item) => (
                        <SidebarLink
                          key={item.to}
                          to={item.to}
                          icon={item.icon}
                          label={t(item.labelKey)}
                          active={isActive(item.to)}
                          onClick={closeMobileMenu}
                        />
                      ))}
                    </nav>
                  )}
                </DialogPrimitive.Content>
              </DialogPrimitive.Portal>
            </DialogPrimitive.Root>
            <Link
              to="/"
              className="hidden items-center gap-2 font-bold focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus min-[360px]:flex"
              aria-label={t('app.name')}
            >
              <PlatformMark size="sm" withName={false} />
            </Link>
          </>
        }
        context={
          <DropdownMenu modal={false}>
            <DropdownMenuTrigger asChild>
              <button
                type="button"
                aria-label={t('navigation.projects')}
                title={currentProject?.name ?? t('navigation.projects')}
                className="hidden min-h-10 min-w-0 max-w-32 items-center gap-1 rounded-md px-2 text-sm text-text-secondary hover:bg-surface-raised hover:text-text-primary lg:flex xl:max-w-52"
              >
                <span className="truncate">{currentProject?.name ?? t('navigation.projects')}</span>
                <ChevronDown className="h-3.5 w-3.5 shrink-0" />
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent
              align="start"
              className="max-h-[calc(100dvh-4rem)] w-64 overflow-y-auto"
            >
              <DropdownMenuItem asChild>
                <Link to="/projects" className="gap-2">
                  <FolderKanban className="h-4 w-4" />
                  {t('navigation.allProjects')}
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
        }
        actions={
          <>
            <Button
              asChild
              size="sm"
              className="hidden min-h-10 min-w-10 gap-1 px-2.5 text-xs md:inline-flex"
            >
              <Link
                to={projectKey ? `/issues/create?project_key=${projectKey}` : '/issues/create'}
                aria-label={t('navigation.create')}
              >
                <Plus className="h-4 w-4" aria-hidden="true" />
                <span className="hidden xl:inline">{t('navigation.create')}</span>
              </Link>
            </Button>
            <ThemeToggle />
            <DropdownMenu modal={false}>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="relative h-11 w-11 md:h-10 md:w-10"
                  aria-label={t('notifications.open')}
                  data-testid="notification-trigger"
                >
                  <Bell className="h-[18px] w-[18px]" />
                  {unreadCount > 0 && (
                    <span className="notification-count absolute -right-1 -top-1 min-w-4 rounded-full bg-danger px-1 text-[10px] font-semibold leading-4">
                      {unreadCount}
                    </span>
                  )}
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-[calc(100vw-1rem)] max-w-80 p-0">
                <DropdownMenuPrimitive.Group className="flex items-center justify-between gap-2 border-b border-border px-3 py-2">
                  <DropdownMenuPrimitive.Label className="text-sm font-semibold">
                    {t('notifications.title')}
                  </DropdownMenuPrimitive.Label>
                  <DropdownMenuItem
                    className="min-h-11 px-2 text-xs"
                    onSelect={() => markAllNotificationsRead.mutate()}
                    disabled={unreadCount === 0 || markAllNotificationsRead.isPending}
                  >
                    {t('notifications.markAllRead')}
                  </DropdownMenuItem>
                </DropdownMenuPrimitive.Group>
                <DropdownMenuPrimitive.Group className="max-h-96 overflow-y-auto p-1">
                  {notifications.slice(0, 10).map((notification) => {
                    const content = (
                      <div className="min-w-0">
                        <div className="line-clamp-2 break-words font-medium">
                          {notification.title}
                        </div>
                        {notification.body && (
                          <div className="mt-0.5 line-clamp-2 text-xs text-text-muted">
                            {notification.body}
                          </div>
                        )}
                      </div>
                    )
                    return (
                      <DropdownMenuPrimitive.Group
                        key={notification.id}
                        className="flex items-start gap-1"
                      >
                        {notification.action_url ? (
                          <DropdownMenuItem
                            asChild
                            className="min-h-11 min-w-0 flex-1 items-start p-2"
                          >
                            <Link
                              to={notification.action_url}
                              onClick={() => {
                                if (!notification.is_read)
                                  markNotificationRead.mutate(notification.id)
                              }}
                            >
                              {content}
                            </Link>
                          </DropdownMenuItem>
                        ) : (
                          <DropdownMenuItem
                            className="min-h-11 min-w-0 flex-1 items-start p-2"
                            onSelect={() => {
                              if (!notification.is_read)
                                markNotificationRead.mutate(notification.id)
                            }}
                          >
                            {content}
                          </DropdownMenuItem>
                        )}
                        {!notification.is_read && (
                          <DropdownMenuItem
                            className="min-h-11 w-11 shrink-0 justify-center p-0"
                            aria-label={`${t('notifications.markRead')}: ${notification.title}`}
                            title={t('notifications.markRead')}
                            onSelect={() => markNotificationRead.mutate(notification.id)}
                          >
                            <Check className="h-4 w-4" aria-hidden="true" />
                          </DropdownMenuItem>
                        )}
                      </DropdownMenuPrimitive.Group>
                    )
                  })}
                  {notifications.length === 0 && (
                    <DropdownMenuPrimitive.Label className="px-3 py-6 text-center text-sm text-text-muted">
                      {t('notifications.empty')}
                    </DropdownMenuPrimitive.Label>
                  )}
                </DropdownMenuPrimitive.Group>
                <DropdownMenuPrimitive.Separator className="h-px bg-border" />
                <DropdownMenuItem asChild className="m-1 justify-center text-accent">
                  <Link to="/notifications">{t('notifications.viewAll')}</Link>
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
            <DropdownMenu modal={false}>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-11 w-11 md:h-10 md:w-10"
                  aria-label={t('navigation.account')}
                >
                  <User className="h-[18px] w-[18px]" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-56">
                <div className="break-words px-2 py-1.5 text-sm font-medium text-text-primary">
                  {user?.display_name ?? user?.email ?? t('navigation.user')}
                </div>
                {user?.display_name && user.display_name !== user.email && (
                  <div className="break-words px-2 pb-2 text-xs text-text-muted">{user?.email}</div>
                )}
                <DropdownMenuItem asChild>
                  <Link to="/admin" className="gap-2 text-text-secondary">
                    <ShieldCheck className="h-4 w-4" />
                    <span>{t('navigation.admin')}</span>
                  </Link>
                </DropdownMenuItem>
                <DropdownMenuItem
                  onSelect={() => logout.mutate()}
                  disabled={logout.isPending}
                  className="gap-2 text-text-secondary"
                >
                  <LogOut className="h-4 w-4" />
                  <span>{t('navigation.logout')}</span>
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </>
        }
      />

      <div className="flex min-h-[calc(100dvh-var(--shell-header-height))]">
        <aside
          className={`hidden shrink-0 flex-col gap-2 border-r border-border bg-surface p-3 md:flex ${sidebarCollapsed ? 'w-[var(--shell-sidebar-compact)]' : 'w-[var(--shell-sidebar-expanded)]'}`}
        >
          <div
            className={`flex min-h-9 items-center ${sidebarCollapsed ? 'justify-center' : 'justify-between px-2'}`}
          >
            {!sidebarCollapsed && (
              <span className="truncate text-xs font-medium uppercase text-text-muted">
                {t('navigation.menu')}
              </span>
            )}
            <Button
              variant="ghost"
              size="icon"
              className="h-9 w-9"
              aria-label={
                sidebarCollapsed ? t('navigation.expandSidebar') : t('navigation.collapseSidebar')
              }
              aria-expanded={!sidebarCollapsed}
              onClick={() => {
                const next = !sidebarCollapsed
                setSidebarCollapsed(next)
                window.localStorage.setItem('tt-sidebar-collapsed', String(next))
              }}
            >
              {sidebarCollapsed ? (
                <PanelLeftOpen className="h-4 w-4" aria-hidden="true" />
              ) : (
                <PanelLeftClose className="h-4 w-4" aria-hidden="true" />
              )}
            </Button>
          </div>
          <nav aria-label={t('navigation.mainNav')}>
            {navItems.map((item) => (
              <SidebarLink
                key={item.to}
                to={item.to}
                icon={item.icon}
                label={t(item.labelKey)}
                active={isActive(item.to)}
                compact={sidebarCollapsed}
              />
            ))}
          </nav>
          {projectKey && (
            <nav
              aria-label={t('navigation.projectNav')}
              className="mt-3 border-t border-border pt-3"
            >
              {!sidebarCollapsed && (
                <div className="mb-1 truncate px-3 text-xs font-medium uppercase text-text-muted">
                  {currentProject?.name ?? projectKey}
                </div>
              )}
              {projectItems.map((item) => (
                <SidebarLink
                  key={item.to}
                  to={item.to}
                  icon={item.icon}
                  label={t(item.labelKey)}
                  active={isActive(item.to)}
                  compact={sidebarCollapsed}
                />
              ))}
            </nav>
          )}
        </aside>

        <main className="shell-main flex-1">
          <PageFrame mode={pageLayout}>
            <Outlet />
          </PageFrame>
        </main>
      </div>
    </div>
  )
}
