import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router'

import { NotificationsPage } from './'

const useNotifications = vi.hoisted(() => vi.fn())
const useNotificationSettings = vi.hoisted(() => vi.fn())
const useMarkAllNotificationsRead = vi.hoisted(() => vi.fn())
const useUpdateNotificationSettings = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', () => ({
  useNotifications,
  useNotificationSettings,
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
    data: [
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
    isLoading: false,
  })
  useNotificationSettings.mockReturnValue({
    data: { email_frequency: 'daily', notify_own_changes: false },
    isLoading: false,
  })
  useMarkAllNotificationsRead.mockReturnValue({ mutate: vi.fn() })
  useUpdateNotificationSettings.mockReturnValue({ mutate: vi.fn() })
}

describe('NotificationsPage', () => {
  it('filters the notification list and marks all notifications as read', async () => {
    const markAll = vi.fn()
    mockHooks()
    useMarkAllNotificationsRead.mockReturnValue({ mutate: markAll })

    renderPage()

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
      data: { email_frequency: 'daily', disabled_event_types: ['issue_updated'], notify_own_changes: false },
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

    fireEvent.click(screen.getByLabelText(/notify about my own changes|уведомлять о моих изменениях/i))
    await waitFor(() =>
      expect(updateSettings).toHaveBeenLastCalledWith({
        email_frequency: 'daily',
        disabled_event_types: ['issue_updated'],
        notify_own_changes: true,
      }),
    )
  })
})
