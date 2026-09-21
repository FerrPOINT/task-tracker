import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import i18n from '@/shared/i18n/config'

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
  const notifications = [
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
  ]
  useNotifications.mockImplementation(({ includeRead }: { includeRead: boolean }) => ({
    error: null,
    refetch: vi.fn(),
    data: {
      notifications: includeRead ? notifications : notifications.filter((item) => !item.is_read),
      unread_count: 1,
    },
    isLoading: false,
  }))
  useNotificationSettings.mockReturnValue({
    data: { email_frequency: 'daily', disabled_event_types: [], notify_own_changes: false },
    isLoading: false,
  })
  useMarkNotificationRead.mockReturnValue({ mutate: vi.fn() })
  useMarkAllNotificationsRead.mockReturnValue({ mutate: vi.fn() })
  useUpdateNotificationSettings.mockReturnValue({ mutate: vi.fn(), reset: vi.fn() })
}

describe('NotificationsPage', () => {
  it('filters the notification list and marks all notifications as read', async () => {
    const markAll = vi.fn()
    mockHooks()
    useMarkAllNotificationsRead.mockReturnValue({ mutate: markAll })

    renderPage()

    expect(useNotifications).toHaveBeenCalledWith({ includeRead: true, limit: 21, offset: 0 })
    expect(screen.getByText('Issue updated')).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /TT-12 has moved to done/i })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /unread|непрочитанные/i }))
    expect(useNotifications).toHaveBeenLastCalledWith({ includeRead: false, limit: 21, offset: 0 })
    expect(screen.queryByText('Mentioned in a comment')).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: /mark all as read|прочитать все/i }))
    await waitFor(() => expect(markAll).toHaveBeenCalledTimes(1))
  })

  it('shows later notifications and resets to the first page when changing filters', () => {
    mockHooks()
    const notifications = Array.from({ length: 25 }, (_, index) => ({
      id: `notification-${index}`,
      title: `Event ${index + 1}`,
      body: null,
      is_read: false,
      action_url: null,
      created_at: '2026-08-24T10:00:00Z',
    }))
    useNotifications.mockImplementation(
      ({
        includeRead,
        limit,
        offset,
      }: {
        includeRead: boolean
        limit: number
        offset: number
      }) => ({
        data: {
          notifications: (includeRead ? notifications : notifications.slice(1)).slice(
            offset,
            offset + limit,
          ),
          unread_count: 24,
        },
        isLoading: false,
        error: null,
        refetch: vi.fn(),
      }),
    )

    renderPage()
    expect(screen.getAllByRole('listitem')).toHaveLength(20)
    expect(screen.queryByText('Event 21')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /next page|следующая страница/i }))
    expect(useNotifications).toHaveBeenLastCalledWith({ includeRead: true, limit: 21, offset: 20 })
    expect(screen.getByText('Event 21')).toBeInTheDocument()
    expect(screen.queryByText('Event 1')).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: /unread|непрочитанные/i }))
    expect(useNotifications).toHaveBeenLastCalledWith({ includeRead: false, limit: 21, offset: 0 })
    expect(screen.getByText('Event 2')).toBeInTheDocument()
    expect(screen.queryByText('Event 1')).not.toBeInTheDocument()
  })

  it('saves multiple preference edits in one backend-compatible request', async () => {
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
    useUpdateNotificationSettings.mockReturnValue({ mutate: updateSettings, reset: vi.fn() })

    renderPage()

    fireEvent.change(screen.getByLabelText(/email frequency|частота email/i), {
      target: { value: 'hourly' },
    })
    fireEvent.click(
      screen.getByLabelText(/notify about my own changes|уведомлять о моих изменениях/i),
    )
    fireEvent.click(screen.getByLabelText('Назначение задачи'))
    expect(updateSettings).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: /сохранить|save/i }))
    expect(updateSettings).toHaveBeenCalledOnce()
    expect(updateSettings).toHaveBeenCalledWith(
      {
        email_frequency: 'hourly',
        disabled_event_types: ['issue_updated', 'issue_assigned'],
        notify_own_changes: true,
      },
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    )
  })

  it('keeps changed preferences after a failed save and retries the same document', () => {
    const updateSettings = vi.fn()
    let failed = false
    mockHooks()
    useUpdateNotificationSettings.mockImplementation(() => ({
      mutate: updateSettings,
      reset: vi.fn(),
      isPending: false,
      isSuccess: false,
      isError: failed,
    }))

    const page = renderPage()
    fireEvent.change(screen.getByLabelText(/email frequency|частота email/i), {
      target: { value: 'hourly' },
    })
    fireEvent.click(screen.getByLabelText('Назначение задачи'))
    fireEvent.click(screen.getByRole('button', { name: /сохранить|save/i }))
    failed = true
    page.rerender(
      <MemoryRouter>
        <NotificationsPage />
      </MemoryRouter>,
    )

    expect(screen.getByLabelText(/email frequency|частота email/i)).toHaveValue('hourly')
    expect(screen.getByLabelText('Назначение задачи')).not.toBeChecked()
    expect(
      screen.getByText(/Could not save preferences|Не удалось сохранить настройки/i),
    ).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /сохранить|save/i }))
    expect(updateSettings).toHaveBeenCalledTimes(2)
    expect(updateSettings).toHaveBeenLastCalledWith(
      {
        email_frequency: 'hourly',
        disabled_event_types: ['issue_assigned'],
        notify_own_changes: false,
      },
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    )
  })

  it('cancels an unsaved preference draft without an API request', () => {
    const updateSettings = vi.fn()
    mockHooks()
    useUpdateNotificationSettings.mockReturnValue({ mutate: updateSettings, reset: vi.fn() })

    renderPage()
    fireEvent.change(screen.getByLabelText(/email frequency|частота email/i), {
      target: { value: 'hourly' },
    })
    expect(screen.getByLabelText(/email frequency|частота email/i)).toHaveValue('hourly')
    fireEvent.click(screen.getByRole('button', { name: /отмена|cancel/i }))

    expect(screen.getByLabelText(/email frequency|частота email/i)).toHaveValue('daily')
    expect(screen.getByRole('button', { name: /сохранить|save/i })).toBeDisabled()
    expect(updateSettings).not.toHaveBeenCalled()
  })

  it('translates every notification event in English', async () => {
    mockHooks()
    await i18n.changeLanguage('en')
    try {
      renderPage()
      const events = screen.getByRole('group', { name: 'Events' })
      expect(events).toBeInTheDocument()
      for (const label of [
        'Issue assigned',
        'Status changed',
        'Issue updated',
        'New comment',
        'Comment edited',
        'Comment deleted',
        'Time logged',
        'Attachment added',
        'Issue linked',
        'Issue unlinked',
      ]) {
        expect(screen.getByLabelText(label)).toBeInTheDocument()
      }
      expect(screen.queryByText('Назначение задачи')).not.toBeInTheDocument()
    } finally {
      await i18n.changeLanguage('ru')
    }
  })

  it('shows a failed mark-read action and allows retry without hiding the notification', () => {
    const markRead = vi.fn()
    mockHooks()
    useMarkNotificationRead.mockReturnValue({
      mutate: markRead,
      isPending: false,
      isSuccess: false,
      isError: true,
      variables: 'notification-1',
    })

    renderPage()
    fireEvent.click(screen.getByRole('button', { name: /Issue updated/i }))

    expect(screen.getByText('Issue updated')).toBeInTheDocument()
    expect(
      screen.getByText(
        /Could not mark notification as read|Не удалось отметить уведомление прочитанным/i,
      ),
    ).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /retry|повторить/i }))
    expect(markRead).toHaveBeenCalledTimes(2)
    expect(markRead).toHaveBeenLastCalledWith('notification-1')
  })

  it('shows pending and success feedback for mark-all', () => {
    const markAll = vi.fn()
    const status = { isPending: false, isSuccess: false, isError: false }
    mockHooks()
    useMarkAllNotificationsRead.mockImplementation(() => ({ mutate: markAll, ...status }))

    const page = renderPage()
    fireEvent.click(screen.getByRole('button', { name: /mark all as read|прочитать все/i }))
    status.isPending = true
    page.rerender(
      <MemoryRouter>
        <NotificationsPage />
      </MemoryRouter>,
    )
    expect(
      screen.getByText(/Marking all notifications as read|Отмечаем все уведомления/i),
    ).toBeInTheDocument()

    status.isPending = false
    status.isSuccess = true
    page.rerender(
      <MemoryRouter>
        <NotificationsPage />
      </MemoryRouter>,
    )
    expect(
      screen.getByText(/All notifications marked as read|Все уведомления прочитаны/i),
    ).toBeInTheDocument()
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

  it('provides an explicit mark-read action for a notification without a link', () => {
    const markRead = vi.fn()
    mockHooks()
    useNotifications.mockReturnValue({
      data: {
        notifications: [
          {
            id: 'notification-3',
            title: 'Build completed',
            body: null,
            is_read: false,
            action_url: null,
            created_at: '2026-08-24T11:00:00Z',
          },
        ],
        unread_count: 1,
      },
      isLoading: false,
      error: null,
      refetch: vi.fn(),
    })
    useMarkNotificationRead.mockReturnValue({ mutate: markRead })

    renderPage()
    fireEvent.click(screen.getByRole('button', { name: /Build completed/i }))
    expect(markRead).toHaveBeenCalledWith('notification-3')
  })

  it('does not expose default preferences when settings fail to load', () => {
    mockHooks()
    const retry = vi.fn()
    useNotificationSettings.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('network error'),
      refetch: retry,
    })

    renderPage()
    expect(screen.queryByLabelText(/email frequency|частота email/i)).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /повторить|retry/i }))
    expect(retry).toHaveBeenCalledOnce()
  })
})
