import { describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'

import { AppShell } from './app-shell'

const useCurrentUser = vi.hoisted(() => vi.fn())
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

function mockHooks(notifications: Notification[] | undefined) {
  useCurrentUser.mockReturnValue({ data: { email: 'user@example.test', display_name: 'User' } })
  useLogout.mockReturnValue({ mutate: vi.fn() })
  useNotifications.mockReturnValue({ data: notifications, isLoading: false })
  useMarkNotificationRead.mockReturnValue({ mutate: vi.fn() })
  useMarkAllNotificationsRead.mockReturnValue({ mutate: vi.fn() })
}

describe('AppShell notifications', () => {
  it('includes the administration link in the desktop sidebar', () => {
    mockHooks([])

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
    mockHooks([
      {
        id: 'notification-1',
        title: 'Issue updated',
        body: 'TT-12 has moved to done',
        is_read: false,
        action_url: '/issues/12',
        created_at: '2026-08-24T10:00:00Z',
      },
    ])
    useMarkNotificationRead.mockReturnValue({ mutate: markRead })
    useMarkAllNotificationsRead.mockReturnValue({ mutate: markAll })

    render(
      <MemoryRouter>
        <AppShell />
      </MemoryRouter>,
    )

    const trigger = screen.getByTestId('notification-trigger')
    expect(trigger).toHaveTextContent('1')
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
