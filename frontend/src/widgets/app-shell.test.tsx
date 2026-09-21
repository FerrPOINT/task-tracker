import { beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@sdlc/ui/lib'

import { AppShell } from './app-shell'

const useCurrentUser = vi.hoisted(() => vi.fn())
const useIssue = vi.hoisted(() => vi.fn())
const useProjects = vi.hoisted(() => vi.fn())
const useLogout = vi.hoisted(() => vi.fn())
const useNotifications = vi.hoisted(() => vi.fn())
const useMarkNotificationRead = vi.hoisted(() => vi.fn())
const useMarkAllNotificationsRead = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', () => ({
  useCurrentUser,
  useLogout,
  useNotifications,
  useMarkNotificationRead,
  useMarkAllNotificationsRead,
  useIssue,
  useProjects,
}))
vi.mock('@/shared/api/useTrackerEvents', () => ({ useTrackerEvents: vi.fn() }))

type Notification = {
  id: string
  title: string
  body?: string | null
  is_read: boolean
  action_url?: string | null
  created_at: string
}

function mockHooks(notifications: Notification[] | undefined, unreadCount?: number) {
  useCurrentUser.mockReturnValue({ data: { email: 'user@example.test', display_name: 'User' } })
  useIssue.mockReturnValue({ data: undefined })
  useProjects.mockReturnValue({ data: [] })
  useLogout.mockReturnValue({ mutate: vi.fn() })
  useNotifications.mockReturnValue({
    data: notifications
      ? {
          notifications,
          unread_count:
            unreadCount ?? notifications.filter((notification) => !notification.is_read).length,
        }
      : undefined,
    isLoading: false,
  })
  useMarkNotificationRead.mockReturnValue({ mutate: vi.fn() })
  useMarkAllNotificationsRead.mockReturnValue({ mutate: vi.fn() })
}

describe('AppShell notifications', () => {
  beforeEach(() => {
    window.localStorage.removeItem('tt-sidebar-collapsed')
  })

  it('keeps global navigation available on desktop outside a project', () => {
    mockHooks([])
    render(
      <ThemeProvider>
        <MemoryRouter initialEntries={['/']}>
          <AppShell />
        </MemoryRouter>
      </ThemeProvider>,
    )

    const navigation = screen.getByRole('navigation', { name: 'Основная навигация' })
    expect(within(navigation).getByRole('link', { name: 'Дашборд' })).toHaveAttribute(
      'aria-current',
      'page',
    )
    expect(within(navigation).getByRole('link', { name: 'Отчёты' })).toHaveAttribute(
      'href',
      '/reports',
    )
    expect(within(navigation).getByRole('link', { name: 'Администрирование' })).toHaveAttribute(
      'href',
      '/admin',
    )
  })

  it('highlights only the current project destination', () => {
    mockHooks([])
    render(
      <ThemeProvider>
        <MemoryRouter initialEntries={['/projects/XP/board']}>
          <AppShell />
        </MemoryRouter>
      </ThemeProvider>,
    )

    const navigation = screen.getByRole('navigation', { name: 'Навигация проекта' })
    expect(within(navigation).getByRole('link', { name: 'Доска' })).toHaveAttribute(
      'aria-current',
      'page',
    )
    expect(within(navigation).getByRole('link', { name: 'Бэклог' })).not.toHaveAttribute(
      'aria-current',
    )
    expect(within(navigation).getByRole('link', { name: 'Корзина' })).not.toHaveAttribute(
      'aria-current',
    )
    expect(within(navigation).getByRole('link', { name: 'Настройки проекта' })).not.toHaveAttribute(
      'aria-current',
    )
  })

  it('retains project navigation and selects project reports for filtered reports', () => {
    mockHooks([])
    render(
      <ThemeProvider>
        <MemoryRouter initialEntries={['/reports?project_key=XP']}>
          <AppShell />
        </MemoryRouter>
      </ThemeProvider>,
    )

    const projectNavigation = screen.getByRole('navigation', { name: 'Навигация проекта' })
    expect(within(projectNavigation).getByRole('link', { name: 'Отчёты проекта' })).toHaveAttribute(
      'aria-current',
      'page',
    )
    expect(
      within(screen.getByRole('navigation', { name: 'Основная навигация' })).getByRole('link', {
        name: 'Отчёты',
      }),
    ).not.toHaveAttribute('aria-current')
    expect(screen.getByRole('link', { name: 'Создать' })).toHaveAttribute(
      'href',
      '/issues/create?project_key=XP',
    )
  })

  it('closes the mobile navigation dialog with Escape and after choosing a link', async () => {
    const user = userEvent.setup()
    mockHooks([])
    render(
      <ThemeProvider>
        <MemoryRouter initialEntries={['/projects/XP/board']}>
          <AppShell />
        </MemoryRouter>
      </ThemeProvider>,
    )

    await user.click(screen.getByRole('button', { name: 'Открыть меню' }))
    let dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByRole('link', { name: 'Доска' })).toHaveAttribute(
      'aria-current',
      'page',
    )
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())

    await user.click(screen.getByRole('button', { name: 'Открыть меню' }))
    dialog = await screen.findByRole('dialog')
    await user.click(within(dialog).getByRole('link', { name: 'Бэклог' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
  })

  it('does not resolve an issue context on the create issue route', () => {
    mockHooks([])

    render(
      <ThemeProvider>
        <MemoryRouter initialEntries={['/issues/create']}>
          <AppShell />
        </MemoryRouter>
      </ThemeProvider>,
    )

    expect(useIssue).toHaveBeenCalledWith('')
    expect(useIssue).not.toHaveBeenCalledWith('create')
  })

  it('uses the query project key for create issue navigation context', () => {
    mockHooks([])

    render(
      <ThemeProvider>
        <MemoryRouter initialEntries={['/issues/create?project_key=XP']}>
          <AppShell />
        </MemoryRouter>
      </ThemeProvider>,
    )

    expect(useIssue).toHaveBeenCalledWith('')
    expect(useIssue).not.toHaveBeenCalledWith('create')
    expect(screen.getByRole('link', { name: /бэклог|backlog/i })).toHaveAttribute(
      'href',
      '/projects/XP/backlog',
    )
    expect(screen.getByRole('link', { name: /доска|board/i })).toHaveAttribute(
      'href',
      '/projects/XP/board',
    )
  })

  it('uses router state project key for create issue navigation context', () => {
    mockHooks([])

    render(
      <ThemeProvider>
        <MemoryRouter
          initialEntries={[{ pathname: '/issues/create', state: { project_key: 'XP' } }]}
        >
          <AppShell />
        </MemoryRouter>
      </ThemeProvider>,
    )

    expect(useIssue).toHaveBeenCalledWith('')
    expect(useIssue).not.toHaveBeenCalledWith('create')
    expect(screen.getByRole('link', { name: /бэклог|backlog/i })).toHaveAttribute(
      'href',
      '/projects/XP/backlog',
    )
    expect(screen.getByRole('link', { name: /доска|board/i })).toHaveAttribute(
      'href',
      '/projects/XP/board',
    )
  })

  it('shows product settings to every signed-in user', async () => {
    const user = userEvent.setup()
    mockHooks([])
    useCurrentUser.mockReturnValue({
      data: { email: 'member@example.test', display_name: 'Member', is_system_admin: false },
    })

    render(
      <ThemeProvider>
        <MemoryRouter>
          <AppShell />
        </MemoryRouter>
      </ThemeProvider>,
    )

    await user.click(screen.getByRole('button', { name: /аккаунт|account/i }))
    expect(
      await screen.findByRole('menuitem', { name: /администрирование|administration/i }),
    ).toHaveAttribute('href', '/admin')
  })

  it('opens an empty notification dropdown without a badge', async () => {
    const user = userEvent.setup()
    mockHooks([])

    render(
      <ThemeProvider>
        <MemoryRouter>
          <AppShell />
        </MemoryRouter>
      </ThemeProvider>,
    )

    const trigger = screen.getByTestId('notification-trigger')
    expect(trigger).not.toHaveTextContent('0')
    await user.click(trigger)

    expect(await screen.findByText(/нет уведомлений|no notifications yet/i)).toBeInTheDocument()
  })

  it('shows unread notifications and invokes mark-read and mark-all actions', async () => {
    const user = userEvent.setup()
    const markRead = vi.fn()
    const markAll = vi.fn()
    mockHooks(
      [
        {
          id: 'notification-1',
          title: 'Issue updated',
          body: 'TT-12 has moved to done',
          is_read: false,
          action_url: '/issues/12',
          created_at: '2026-08-24T10:00:00Z',
        },
      ],
      12,
    )
    useMarkNotificationRead.mockReturnValue({ mutate: markRead })
    useMarkAllNotificationsRead.mockReturnValue({ mutate: markAll })

    render(
      <ThemeProvider>
        <MemoryRouter>
          <AppShell />
        </MemoryRouter>
      </ThemeProvider>,
    )

    const trigger = screen.getByTestId('notification-trigger')
    expect(trigger).toHaveTextContent('12')
    await user.click(trigger)

    expect(await screen.findByText('Issue updated')).toBeInTheDocument()
    await user.click(screen.getByRole('menuitem', { name: /отметить прочитанным: Issue updated/i }))
    expect(markRead).toHaveBeenCalledWith('notification-1')

    await user.click(trigger)
    await user.click(screen.getByRole('menuitem', { name: /прочитать все|mark all as read/i }))
    await waitFor(() => expect(markAll).toHaveBeenCalledTimes(1))
    await user.click(trigger)
    const viewAllLink = screen.getByRole('menuitem', {
      name: /все уведомления|view all notifications/i,
    })
    expect(viewAllLink).toHaveAttribute('href', '/notifications')
  })
})
