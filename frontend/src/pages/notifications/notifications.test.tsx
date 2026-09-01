import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import { NotificationsPage } from './'

const useNotifications = vi.hoisted(() => vi.fn())
const useNotificationSettings = vi.hoisted(() => vi.fn())
const useMarkNotificationRead = vi.hoisted(() => vi.fn())
const useMarkAllNotificationsRead = vi.hoisted(() => vi.fn())
const useUpdateNotificationSettings = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', () => ({
  useNotifications,
  useNotificationSettings,
  useMarkNotificationRead,
  useMarkAllNotificationsRead,
  useUpdateNotificationSettings,
}))

function renderPage() {
  return render(
    <MemoryRouter>
      <NotificationsPage />
    </MemoryRouter>,
  )
}

function mockHooks() {
  useNotifications.mockReturnValue({
    error: null,
    refetch: vi.fn(),
    data: {
      notifications: [
        {
          id: 'notification-1',
          title: 'Issue updated',
          body: 'TT-12 has moved to done',
          is_read: false,
          action_url: '/issues/12',
          created_at: '2026-08-24T10:00:00Z',
        },
        {
          id: 'notification-2',
          title: 'Mentioned in a comment',
          body: null,
          is_read: true,
          action_url: null,
          created_at: '2026-08-24T09:00:00Z',
        },
      ],
      unread_count: 1,
    },
    isLoading: false,
  })
  useNotificationSettings.mockReturnValue({
    data: { email_frequency: 'daily', disabled_event_types: [], notify_own_changes: false },
    isLoading: false,
  })
  useMarkNotificationRead.mockReturnValue({ mutate: vi.fn() })
  useMarkAllNotificationsRead.mockReturnValue({ mutate: vi.fn() })
  useUpdateNotificationSettings.mockReturnValue({ mutate: vi.fn() })
}

describe('NotificationsPage', () => {
  it('filters the notification list and marks all notifications as read', async () => {
    const markAll = vi.fn()
    mockHooks()
    useMarkAllNotificationsRead.mockReturnValue({ mutate: markAll })

    renderPage()

    expect(useNotifications).toHaveBeenCalledWith({ includeRead: true, limit: 50 })
    expect(screen.getByText('Issue updated')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /unread|непрочитанные/i }))
    expect(screen.queryByText('Mentioned in a comment')).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: /mark all as read|прочитать все/i }))
    await waitFor(() => expect(markAll).toHaveBeenCalledTimes(1))
  })

  it('sends a complete backend-compatible settings document', async () => {
    const updateSettings = vi.fn()
    mockHooks()
    useNotificationSettings.mockReturnValue({
      data: {
        email_frequency: 'daily',
        disabled_event_types: ['issue_updated'],
        notify_own_changes: false,
      },
      isLoading: false,
    })
    useUpdateNotificationSettings.mockReturnValue({ mutate: updateSettings })

    renderPage()

    fireEvent.change(screen.getByLabelText(/email frequency|частота email/i), {
      target: { value: 'hourly' },
    })
    await waitFor(() =>
      expect(updateSettings).toHaveBeenCalledWith({
        email_frequency: 'hourly',
        disabled_event_types: ['issue_updated'],
        notify_own_changes: false,
      }),
    )

    fireEvent.click(
      screen.getByLabelText(/notify about my own changes|уведомлять о моих изменениях/i),
    )
    await waitFor(() =>
      expect(updateSettings).toHaveBeenLastCalledWith({
        email_frequency: 'daily',
        disabled_event_types: ['issue_updated'],
        notify_own_changes: true,
      }),
    )
  })

  it('renders an API error state with retry instead of an empty list', () => {
    mockHooks()
    useNotifications.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('500'),
      refetch: vi.fn(),
    })

    renderPage()

    expect(screen.getByRole('alert')).toBeInTheDocument()
    expect(screen.queryByText('Issue updated')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: /повторить|retry/i })).toBeInTheDocument()
  })
})
