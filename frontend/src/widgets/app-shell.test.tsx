import { describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'

import { AppShell } from './app-shell'

const useCurrentUser = vi.hoisted(() => vi.fn())
const useIssue = vi.hoisted(() => vi.fn())
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
}))
vi.mock('@/shared/api/useTrackerEvents', () => ({ useTrackerEvents: vi.fn() }))
vi.mock('@/shared/ui/theme-toggle', () => ({ ThemeToggle: () => null }))

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
  it('does not resolve an issue context on the create issue route', () => {
    mockHooks([])

    render(
      <MemoryRouter initialEntries={['/issues/create']}>
        <AppShell />
      </MemoryRouter>,
    )

    expect(useIssue).toHaveBeenCalledWith('')
    expect(useIssue).not.toHaveBeenCalledWith('create')
  })

  it('uses the query project key for create issue navigation context', () => {
    mockHooks([])

    render(
      <MemoryRouter initialEntries={['/issues/create?project_key=XP']}>
        <AppShell />
      </MemoryRouter>,
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
      <MemoryRouter initialEntries={[{ pathname: '/issues/create', state: { project_key: 'XP' } }]}>
        <AppShell />
      </MemoryRouter>,
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

  it('includes the administration link in the desktop sidebar', () => {
    mockHooks([])
    useCurrentUser.mockReturnValue({
      data: { email: 'admin@example.test', display_name: 'Admin', is_system_admin: true },
    })

    render(
      <MemoryRouter>
        <AppShell />
      </MemoryRouter>,
    )

    expect(screen.getByRole('link', { name: /администрирование|administration/i })).toHaveAttribute(
      'href',
      '/admin',
    )
  })

  it('opens an empty notification dropdown without a badge', async () => {
    const user = userEvent.setup()
    mockHooks([])

    render(
      <MemoryRouter>
        <AppShell />
      </MemoryRouter>,
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
      <MemoryRouter>
        <AppShell />
      </MemoryRouter>,
    )

    const trigger = screen.getByTestId('notification-trigger')
    expect(trigger).toHaveTextContent('12')
    await user.click(trigger)

    expect(await screen.findByText('Issue updated')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /отметить прочитанным|mark as read/i }))
    expect(markRead).toHaveBeenCalledWith('notification-1')

    await user.click(screen.getByRole('button', { name: /прочитать все|mark all as read/i }))
    await waitFor(() => expect(markAll).toHaveBeenCalledTimes(1))
    const viewAllLink = screen.getByRole('menuitem', {
      name: /все уведомления|view all notifications/i,
    })
    expect(viewAllLink).toHaveAttribute('href', '/notifications')
  })
})
