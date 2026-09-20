import { memo, useEffect, useState } from 'react'
import { Link } from 'react-router'
import { useTranslation } from 'react-i18next'
import type { NotificationItem, UpdateNotificationSettingsInput } from '@/api/notifications'
import {
  useMarkAllNotificationsRead,
  useMarkNotificationRead,
  useNotificationSettings,
  useNotifications,
  useUpdateNotificationSettings,
} from '@/shared/api/hooks'
import { Button } from '@sdlc/ui/ui'
import { Card, CardContent, CardHeader, CardTitle } from '@sdlc/ui/ui'
import { ErrorState } from '@sdlc/ui/ui'
import { Label } from '@sdlc/ui/ui'
import { Check, ChevronLeft, ChevronRight } from 'lucide-react'

const PAGE_SIZE = 20

const NOTIFICATION_EVENTS = [
  ['issue_assigned', 'Назначение задачи'],
  ['issue_moved', 'Смена статуса'],
  ['issue_updated', 'Изменение задачи'],
  ['issue_commented', 'Новый комментарий'],
  ['issue_comment_edited', 'Изменение комментария'],
  ['issue_comment_deleted', 'Удаление комментария'],
  ['issue_worklog_logged', 'Учёт времени'],
  ['issue_attachment_added', 'Новый файл'],
  ['issue_link_created', 'Новая связь'],
  ['issue_link_deleted', 'Удаление связи'],
] as const

const NotificationRow = memo(function NotificationRow({
  notification,
  onMarkRead,
  pendingRead,
}: {
  notification: NotificationItem
  onMarkRead: (id: string) => void
  pendingRead: boolean
}) {
  const { t, i18n } = useTranslation()
  const handleClick = () => {
    if (!notification.is_read) onMarkRead(notification.id)
  }
  const createdAt = new Date(notification.created_at)
  const dateLabel = new Intl.DateTimeFormat(i18n.language, {
    day: 'numeric',
    month: 'short',
    hour: '2-digit',
    minute: '2-digit',
  }).format(createdAt)
  const content = (
    <>
      <div className="flex min-w-0 items-start gap-2">
        <h2 className="min-w-0 flex-1 break-words text-sm font-semibold">{notification.title}</h2>
        <time
          dateTime={notification.created_at}
          title={createdAt.toLocaleString(i18n.language)}
          className="shrink-0 pt-0.5 text-xs text-text-muted"
        >
          {dateLabel}
        </time>
      </div>
      {notification.body && (
        <p className="mt-0.5 break-words text-xs text-text-secondary">{notification.body}</p>
      )}
    </>
  )
  return (
    <li
      className={`flex min-w-0 items-center gap-2 border-l-2 px-2 py-1.5 ${notification.is_read ? 'border-l-transparent' : 'border-l-accent'}`}
    >
      {notification.action_url ? (
        <Link
          to={notification.action_url}
          className="block min-w-0 flex-1 py-1 hover:text-accent"
          onClick={handleClick}
        >
          {content}
        </Link>
      ) : (
        <div className="min-w-0 flex-1 py-1">{content}</div>
      )}
      {!notification.is_read && (
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="h-10 w-10 shrink-0"
          aria-label={`${t('notifications.markRead')}: ${notification.title}`}
          title={t('notifications.markRead')}
          disabled={pendingRead}
          onClick={() => onMarkRead(notification.id)}
        >
          <Check className="h-4 w-4" />
        </Button>
      )}
    </li>
  )
})

export function NotificationsPage() {
  const { t } = useTranslation()
  const [showUnread, setShowUnread] = useState(false)
  const [page, setPage] = useState(0)
  const [settingsDraft, setSettingsDraft] = useState<UpdateNotificationSettingsInput | null>(null)
  const [readAction, setReadAction] = useState<'one' | 'all' | null>(null)
  const {
    data: notificationList,
    isLoading: notificationsLoading,
    error: notificationsError,
    refetch: refetchNotifications,
  } = useNotifications({ includeRead: !showUnread, limit: PAGE_SIZE + 1, offset: page * PAGE_SIZE })
  const {
    data: settings,
    isLoading: settingsLoading,
    refetch: refetchSettings,
  } = useNotificationSettings()
  const markNotificationRead = useMarkNotificationRead()
  const markAllNotificationsRead = useMarkAllNotificationsRead()
  const updateSettings = useUpdateNotificationSettings()
  const displayedSettings = settingsDraft ?? settings
  const readMutation = readAction === 'all' ? markAllNotificationsRead : markNotificationRead
  const notifications = notificationList?.notifications ?? []
  const visibleNotifications = notifications.slice(0, PAGE_SIZE)
  const hasNextPage = notifications.length > PAGE_SIZE
  const unreadCount = notificationList?.unread_count ?? 0

  useEffect(() => {
    if (page > 0 && notificationList && notifications.length === 0) setPage(page - 1)
  }, [page, notificationList, notifications.length])

  useEffect(() => {
    if (!settings || !settingsDraft || updateSettings.isPending || updateSettings.isError) return
    if (
      settings.email_frequency === settingsDraft.email_frequency &&
      settings.notify_own_changes === settingsDraft.notify_own_changes &&
      settings.disabled_event_types.length === settingsDraft.disabled_event_types.length &&
      settings.disabled_event_types.every((event) =>
        settingsDraft.disabled_event_types.includes(event),
      )
    )
      setSettingsDraft(null)
  }, [settings, settingsDraft, updateSettings.isPending, updateSettings.isError])

  function updatePreference(input: Partial<UpdateNotificationSettingsInput>) {
    if (!displayedSettings) return
    const next = { ...displayedSettings, ...input }
    setSettingsDraft(next)
    updateSettings.mutate(next)
  }

  function toggleEvent(eventType: string, enabled: boolean) {
    const disabled = new Set(displayedSettings?.disabled_event_types ?? [])
    if (enabled) disabled.delete(eventType)
    else disabled.add(eventType)
    updatePreference({ disabled_event_types: [...disabled] })
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-xl font-bold sm:text-2xl">{t('notifications.title')}</h1>
          <p className="mt-1 text-sm text-text-muted">{t('notifications.description')}</p>
        </div>
        <Button
          variant="outline"
          size="sm"
          className="min-h-10"
          onClick={() => {
            setPage(0)
            setReadAction('all')
            markAllNotificationsRead.mutate()
          }}
          disabled={unreadCount === 0 || markAllNotificationsRead.isPending}
        >
          {t('notifications.markAllRead')}
        </Button>
      </div>

      <div className="grid grid-cols-[minmax(0,1fr)] gap-6 lg:grid-cols-[minmax(0,1fr)_19rem]">
        <section aria-label={t('notifications.title')} className="min-w-0 space-y-3">
          <div className="flex items-center gap-1 rounded-lg border border-border bg-surface p-1">
            <Button
              variant={showUnread ? 'ghost' : 'secondary'}
              size="sm"
              className="min-h-10"
              aria-pressed={!showUnread}
              onClick={() => {
                setShowUnread(false)
                setPage(0)
              }}
            >
              {t('notifications.all')}
            </Button>
            <Button
              variant={showUnread ? 'secondary' : 'ghost'}
              size="sm"
              className="min-h-10"
              aria-pressed={showUnread}
              onClick={() => {
                setShowUnread(true)
                setPage(0)
              }}
            >
              {t('notifications.unread', { count: unreadCount })}
            </Button>
          </div>

          {readAction && (
            <div role="status" aria-live="polite" className="text-sm">
              {readMutation.isPending && (
                <span className="text-text-muted">
                  {t(
                    readAction === 'all'
                      ? 'notifications.markAllPending'
                      : 'notifications.markReadPending',
                  )}
                </span>
              )}
              {readMutation.isSuccess && (
                <span className="text-success">
                  {t(
                    readAction === 'all'
                      ? 'notifications.markAllSuccess'
                      : 'notifications.markReadSuccess',
                  )}
                </span>
              )}
              {readMutation.isError && (
                <span className="flex items-center gap-2 text-danger">
                  {t(
                    readAction === 'all'
                      ? 'notifications.markAllError'
                      : 'notifications.markReadError',
                  )}
                  <Button
                    variant="ghost"
                    size="sm"
                    className="min-h-10"
                    onClick={() => {
                      if (readAction === 'all') markAllNotificationsRead.mutate()
                      else if (markNotificationRead.variables)
                        markNotificationRead.mutate(markNotificationRead.variables)
                    }}
                  >
                    {t('common.retry')}
                  </Button>
                </span>
              )}
            </div>
          )}

          {notificationsLoading ? (
            <p className="rounded-lg border border-border bg-surface p-6 text-sm text-text-muted">
              {t('notifications.loading')}
            </p>
          ) : notificationsError ? (
            <ErrorState message={t('common.error')} onRetry={() => void refetchNotifications()} />
          ) : visibleNotifications.length === 0 ? (
            <p className="rounded-lg border border-border bg-surface p-6 text-sm text-text-muted">
              {showUnread ? t('notifications.emptyUnread') : t('notifications.empty')}
            </p>
          ) : (
            <>
              <ul className="divide-y divide-border overflow-hidden rounded-md border border-border bg-surface">
                {visibleNotifications.map((notification) => (
                  <NotificationRow
                    key={notification.id}
                    notification={notification}
                    pendingRead={
                      markNotificationRead.isPending &&
                      markNotificationRead.variables === notification.id
                    }
                    onMarkRead={(id) => {
                      setReadAction('one')
                      markNotificationRead.mutate(id)
                    }}
                  />
                ))}
              </ul>
              {(page > 0 || hasNextPage) && (
                <nav
                  aria-label={t('notifications.pagination')}
                  className="flex items-center justify-center gap-3 pt-1"
                >
                  <Button
                    variant="outline"
                    size="icon"
                    className="h-10 w-10"
                    aria-label={t('notifications.previousPage')}
                    title={t('notifications.previousPage')}
                    disabled={page === 0}
                    onClick={() => setPage(page - 1)}
                  >
                    <ChevronLeft className="h-4 w-4" />
                  </Button>
                  <span className="text-sm text-text-secondary">
                    {t('notifications.page', { page: page + 1 })}
                  </span>
                  <Button
                    variant="outline"
                    size="icon"
                    className="h-10 w-10"
                    aria-label={t('notifications.nextPage')}
                    title={t('notifications.nextPage')}
                    disabled={!hasNextPage}
                    onClick={() => setPage(page + 1)}
                  >
                    <ChevronRight className="h-4 w-4" />
                  </Button>
                </nav>
              )}
            </>
          )}
        </section>

        <Card className="h-fit">
          <CardHeader>
            <CardTitle>{t('notifications.preferences')}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-5">
            {settingsLoading ? (
              <p className="text-sm text-text-muted">{t('notifications.loading')}</p>
            ) : !displayedSettings ? (
              <ErrorState message={t('common.error')} onRetry={() => void refetchSettings()} />
            ) : (
              <>
                <div aria-live="polite" className="min-h-5 text-xs">
                  {updateSettings.isPending && (
                    <span className="text-text-muted">{t('common.saving')}</span>
                  )}
                  {updateSettings.isSuccess && (
                    <span className="text-success">{t('common.saved')}</span>
                  )}
                  {updateSettings.isError && (
                    <span className="flex items-center gap-2 text-danger">
                      {t('notifications.settingsSaveError')}
                      <Button
                        variant="ghost"
                        size="sm"
                        className="min-h-10"
                        disabled={!settingsDraft}
                        onClick={() => {
                          if (settingsDraft) updateSettings.mutate(settingsDraft)
                        }}
                      >
                        {t('common.retry')}
                      </Button>
                    </span>
                  )}
                </div>
                <div className="space-y-2">
                  <Label htmlFor="notification-frequency">{t('notifications.frequency')}</Label>
                  <select
                    id="notification-frequency"
                    className="flex min-h-10 w-full rounded-md border border-border-strong bg-surface px-3 text-sm text-text-primary"
                    value={displayedSettings.email_frequency}
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
                <div className="flex min-h-10 items-center gap-2">
                  <input
                    id="notify-own-changes"
                    type="checkbox"
                    className="mt-0.5 h-4 w-4 accent-accent"
                    checked={displayedSettings.notify_own_changes}
                    onChange={(event) =>
                      updatePreference({ notify_own_changes: event.target.checked })
                    }
                    disabled={updateSettings.isPending}
                  />
                  <Label htmlFor="notify-own-changes" className="leading-5">
                    {t('notifications.ownChanges')}
                  </Label>
                </div>
                <fieldset className="space-y-2 border-t border-border pt-4">
                  <legend className="mb-2 text-sm font-medium">События</legend>
                  {NOTIFICATION_EVENTS.map(([eventType, label]) => (
                    <label
                      key={eventType}
                      className="flex min-h-10 cursor-pointer items-center gap-2 text-sm text-text-secondary"
                    >
                      <input
                        type="checkbox"
                        className="h-4 w-4 accent-accent"
                        checked={!displayedSettings.disabled_event_types.includes(eventType)}
                        onChange={(event) => toggleEvent(eventType, event.target.checked)}
                        disabled={updateSettings.isPending}
                      />
                      <span>{label}</span>
                    </label>
                  ))}
                </fieldset>
              </>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
