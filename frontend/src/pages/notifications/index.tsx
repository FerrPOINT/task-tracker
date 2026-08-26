import { memo, useState } from 'react'
import { Link } from 'react-router'
import { useTranslation } from 'react-i18next'
import type { NotificationItem, UpdateNotificationSettingsInput } from '@/api/notifications'
import {
  useMarkAllNotificationsRead,
  useNotificationSettings,
  useNotifications,
  useUpdateNotificationSettings,
} from '@/shared/api/hooks'
import { Button } from '@/shared/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/shared/ui/card'
import { Label } from '@/shared/ui/label'

const NotificationCard = memo(function NotificationCard({
  notification,
}: {
  notification: NotificationItem
}) {
  return (
    <Card className={!notification.is_read ? 'border-l-4 border-l-accent' : undefined}>
      <CardContent className="p-4">
        {notification.action_url ? (
          <Link to={notification.action_url} className="block hover:text-accent">
            <h2 className="font-semibold">{notification.title}</h2>
            {notification.body && (
              <p className="mt-1 text-sm text-text-secondary">{notification.body}</p>
            )}
          </Link>
        ) : (
          <>
            <h2 className="font-semibold">{notification.title}</h2>
            {notification.body && (
              <p className="mt-1 text-sm text-text-secondary">{notification.body}</p>
            )}
          </>
        )}
      </CardContent>
    </Card>
  )
})

export function NotificationsPage() {
  const { t } = useTranslation()
  const [showUnread, setShowUnread] = useState(false)
  const { data: notifications = [], isLoading: notificationsLoading } = useNotifications()
  const { data: settings, isLoading: settingsLoading } = useNotificationSettings()
  const markAllNotificationsRead = useMarkAllNotificationsRead()
  const updateSettings = useUpdateNotificationSettings()
  const visibleNotifications = showUnread
    ? notifications.filter((notification) => !notification.is_read)
    : notifications
  const unreadCount = notifications.filter((notification) => !notification.is_read).length

  function updatePreference(input: Partial<UpdateNotificationSettingsInput>) {
    const current: UpdateNotificationSettingsInput = settings ?? {
      email_frequency: 'immediate',
      disabled_event_types: [],
      notify_own_changes: false,
    }
    updateSettings.mutate({ ...current, ...input })
  }

  return (
    <div className="mx-auto max-w-5xl space-y-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-xl font-bold sm:text-2xl">{t('notifications.title')}</h1>
          <p className="mt-1 text-sm text-text-muted">{t('notifications.description')}</p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => markAllNotificationsRead.mutate()}
          disabled={unreadCount === 0 || markAllNotificationsRead.isPending}
        >
          {t('notifications.markAllRead')}
        </Button>
      </div>

      <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_19rem]">
        <section aria-label={t('notifications.title')} className="space-y-3">
          <div className="flex items-center gap-1 rounded-lg border border-border bg-surface p-1">
            <Button
              variant={showUnread ? 'ghost' : 'secondary'}
              size="sm"
              aria-pressed={!showUnread}
              onClick={() => setShowUnread(false)}
            >
              {t('notifications.all')}
            </Button>
            <Button
              variant={showUnread ? 'secondary' : 'ghost'}
              size="sm"
              aria-pressed={showUnread}
              onClick={() => setShowUnread(true)}
            >
              {t('notifications.unread', { count: unreadCount })}
            </Button>
          </div>

          {notificationsLoading ? (
            <p className="rounded-lg border border-border bg-surface p-6 text-sm text-text-muted">
              {t('notifications.loading')}
            </p>
          ) : visibleNotifications.length === 0 ? (
            <p className="rounded-lg border border-border bg-surface p-6 text-sm text-text-muted">
              {showUnread ? t('notifications.emptyUnread') : t('notifications.empty')}
            </p>
          ) : (
            visibleNotifications.map((notification) => (
              <NotificationCard key={notification.id} notification={notification} />
            ))
          )}
        </section>

        <Card className="h-fit">
          <CardHeader>
            <CardTitle>{t('notifications.preferences')}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-5">
            {settingsLoading ? (
              <p className="text-sm text-text-muted">{t('notifications.loading')}</p>
            ) : (
              <>
                <div className="space-y-2">
                  <Label htmlFor="notification-frequency">{t('notifications.frequency')}</Label>
                  <select
                    id="notification-frequency"
                    className="flex h-9 w-full rounded-md border border-border-strong bg-surface px-3 text-sm text-text-primary"
                    value={settings?.email_frequency ?? 'immediate'}
                    onChange={(event) =>
                      updatePreference({
                        email_frequency: event.target.value,
                      })
                    }
                    disabled={updateSettings.isPending}
                  >
                    <option value="immediate">{t('notifications.frequencyImmediate')}</option>
                    <option value="hourly">{t('notifications.frequencyHourly')}</option>
                    <option value="daily">{t('notifications.frequencyDaily')}</option>
                    <option value="never">{t('notifications.frequencyNone')}</option>
                  </select>
                </div>
                <div className="flex items-start gap-2">
                  <input
                    id="notify-own-changes"
                    type="checkbox"
                    className="mt-0.5 h-4 w-4 accent-accent"
                    checked={settings?.notify_own_changes ?? false}
                    onChange={(event) =>
                      updatePreference({ notify_own_changes: event.target.checked })
                    }
                    disabled={updateSettings.isPending}
                  />
                  <Label htmlFor="notify-own-changes" className="leading-5">
                    {t('notifications.ownChanges')}
                  </Label>
                </div>
              </>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
