import { memo, useCallback, useEffect, useState } from 'react'
import { ProjectLink as Link } from '@/shared/lib/project-navigation'
import { useSearchParams } from 'react-router'
import { useTranslation } from 'react-i18next'
import type {
  NotificationItem,
  NotificationSettings,
  UpdateNotificationSettingsInput,
} from '@/api/notifications'
import {
  useMarkAllNotificationsRead,
  useMarkNotificationRead,
  useNotificationSettings,
  useNotifications,
  useUpdateNotificationSettings,
} from '@/shared/api/hooks'
import {
  Fieldset,
  RadioGroup,
  PageHeader,
  Select,
  Button,
  ResourceList,
  ResourceListRow,
  Checkbox,
  Switch,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  ErrorState,
  EmptyState,
  LoadingState,
  Label,
  Notice,
  PageNavigation,
} from '@pdlc/ui/ui'

import { Check } from 'lucide-react'

const PAGE_SIZE = 20

const NOTIFICATION_EVENTS = [
  'issue_assigned',
  'issue_moved',
  'issue_updated',
  'issue_commented',
  'issue_comment_edited',
  'issue_comment_deleted',
  'issue_worklog_logged',
  'issue_attachment_added',
  'issue_link_created',
  'issue_link_deleted',
] as const

function sameSettings(left: NotificationSettings, right: UpdateNotificationSettingsInput) {
  return (
    left.email_frequency === right.email_frequency &&
    left.notify_own_changes === right.notify_own_changes &&
    left.disabled_event_types.length === right.disabled_event_types.length &&
    left.disabled_event_types.every((event) => right.disabled_event_types.includes(event))
  )
}

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
      className={`flex min-w-0 items-stretch border-l-2 ${notification.is_read ? 'border-l-transparent' : 'border-l-accent'}`}
    >
      {notification.action_url ? (
        <ResourceListRow asChild interactive className="min-w-0 flex-1">
          <Link to={notification.action_url} className="block" onClick={handleClick}>
            {content}
          </Link>
        </ResourceListRow>
      ) : (
        <ResourceListRow className="min-w-0 flex-1">{content}</ResourceListRow>
      )}
      {!notification.is_read && (
        <ResourceListRow className="flex shrink-0 items-center">
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label={`${t('notifications.markRead')}: ${notification.title}`}
            title={t('notifications.markRead')}
            disabled={pendingRead}
            onClick={() => onMarkRead(notification.id)}
          >
            <Check className="h-4 w-4" />
          </Button>
        </ResourceListRow>
      )}
    </li>
  )
})

export function NotificationsPage() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const filterParam = searchParams.get('filter')
  const pageParam = searchParams.get('page')
  const parsedPage = Number(pageParam)
  const hasValidPage = pageParam !== null && Number.isSafeInteger(parsedPage) && parsedPage > 0
  const showUnread = filterParam === 'unread'
  const page = hasValidPage ? parsedPage - 1 : 0
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
  const settingsDirty = !!settings && !!settingsDraft && !sameSettings(settings, settingsDraft)
  const readMutation = readAction === 'all' ? markAllNotificationsRead : markNotificationRead
  const notifications = notificationList?.notifications ?? []
  const visibleNotifications = notifications.slice(0, PAGE_SIZE)
  const hasNextPage = notifications.length > PAGE_SIZE
  const unreadCount = notificationList?.unread_count ?? 0

  const updateListParams = useCallback(
    (nextUnread: boolean, nextPage: number, replace = false) => {
      setSearchParams(
        (current) => {
          const next = new URLSearchParams(current)
          if (nextUnread) next.set('filter', 'unread')
          else next.delete('filter')
          if (nextPage > 0) next.set('page', String(nextPage + 1))
          else next.delete('page')
          return next
        },
        { replace },
      )
    },
    [setSearchParams],
  )

  useEffect(() => {
    const canonicalFilter = showUnread ? 'unread' : null
    const canonicalPage = hasValidPage && parsedPage > 1 ? String(parsedPage) : null
    if (filterParam === canonicalFilter && pageParam === canonicalPage) return

    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current)
        if (canonicalFilter) next.set('filter', canonicalFilter)
        else next.delete('filter')
        if (canonicalPage) next.set('page', canonicalPage)
        else next.delete('page')
        return next
      },
      { replace: true },
    )
  }, [filterParam, hasValidPage, pageParam, parsedPage, setSearchParams, showUnread])

  useEffect(() => {
    if (page > 0 && notificationList && notifications.length === 0) {
      updateListParams(showUnread, 0, true)
    }
  }, [notificationList, notifications.length, page, showUnread, updateListParams])

  function updatePreference(input: Partial<UpdateNotificationSettingsInput>) {
    if (!displayedSettings) return
    const next = { ...displayedSettings, ...input }
    setSettingsDraft(settings && sameSettings(settings, next) ? null : next)
    updateSettings.reset()
  }

  function toggleEvent(eventType: string, enabled: boolean) {
    const disabled = new Set(displayedSettings?.disabled_event_types ?? [])
    if (enabled) disabled.delete(eventType)
    else disabled.add(eventType)
    updatePreference({ disabled_event_types: [...disabled] })
  }

  function saveSettings() {
    if (!settingsDraft || !settingsDirty) return
    updateSettings.mutate(settingsDraft, { onSuccess: () => setSettingsDraft(null) })
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title={t('notifications.title')}
        context={t('notifications.description')}
        actions={
          <Button
            variant="outline"
            size="sm"

            onClick={() => {
              updateListParams(showUnread, 0, true)
              setReadAction('all')
              markAllNotificationsRead.mutate()
            }}
            disabled={unreadCount === 0 || markAllNotificationsRead.isPending}
          >
            {t('notifications.markAllRead')}
          </Button>
        }
      />

      <div className="grid grid-cols-[minmax(0,1fr)] gap-6 lg:grid-cols-[minmax(0,1fr)_19rem]">
        <section aria-label={t('notifications.title')} className="min-w-0 space-y-3">
          <RadioGroup
            appearance="segmented"
            label={t('notifications.title')}
            value={showUnread ? 'unread' : 'all'}
            onValueChange={(value) => updateListParams(value === 'unread', 0)}
            options={[
              { value: 'all', label: t('notifications.all') },
              { value: 'unread', label: t('notifications.unread', { count: unreadCount }) },
            ]}
          />

          {readAction && (
            <Notice
              role="status"
              aria-live="polite"
              appearance="inline"
              tone={
                readMutation.isError ? 'danger' : readMutation.isSuccess ? 'success' : 'neutral'
              }
              actions={
                readMutation.isError && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => {
                      if (readAction === 'all') markAllNotificationsRead.mutate()
                      else if (markNotificationRead.variables)
                        markNotificationRead.mutate(markNotificationRead.variables)
                    }}
                  >
                    {t('common.retry')}
                  </Button>
                )
              }
            >
              {readMutation.isPending && (
                <span>
                  {t(
                    readAction === 'all'
                      ? 'notifications.markAllPending'
                      : 'notifications.markReadPending',
                  )}
                </span>
              )}
              {readMutation.isSuccess && (
                <span>
                  {t(
                    readAction === 'all'
                      ? 'notifications.markAllSuccess'
                      : 'notifications.markReadSuccess',
                  )}
                </span>
              )}
              {readMutation.isError && (
                <span>
                  {t(
                    readAction === 'all'
                      ? 'notifications.markAllError'
                      : 'notifications.markReadError',
                  )}
                </span>
              )}
            </Notice>
          )}

          {notificationsLoading ? (
            <LoadingState message={t('notifications.loading')} />
          ) : notificationsError ? (
            <ErrorState message={t('common.error')} onRetry={() => void refetchNotifications()} />
          ) : visibleNotifications.length === 0 ? (
            <EmptyState
              message={showUnread ? t('notifications.emptyUnread') : t('notifications.empty')}
            />
          ) : (
            <>
              <ResourceList asChild variant="bordered">
                <ul className="overflow-hidden">
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
              </ResourceList>
              {(page > 0 || hasNextPage) && (
                <PageNavigation
                  label={t('notifications.pagination')}
                  compact
                  page={page + 1}
                  hasNext={hasNextPage}
                  previousLabel={t('notifications.previousPage')}
                  nextLabel={t('notifications.nextPage')}
                  summary={t('notifications.page', { page: page + 1 })}
                  onPageChange={(next) => updateListParams(showUnread, next - 1)}
                  className="justify-center pt-1"
                />
              )}
            </>
          )}
        </section>

        <Card className="h-fit">
          <CardHeader>
            <CardTitle>{t('notifications.preferences')}</CardTitle>
          </CardHeader>
          <CardContent>
            {settingsLoading ? (
              <p className="text-sm text-text-muted">{t('notifications.loading')}</p>
            ) : !displayedSettings ? (
              <ErrorState message={t('common.error')} onRetry={() => void refetchSettings()} />
            ) : (
              <form
                className="space-y-4"
                onSubmit={(event) => {
                  event.preventDefault()
                  saveSettings()
                }}
              >
                <div className="space-y-2">
                  <Label htmlFor="notification-frequency">{t('notifications.frequency')}</Label>
                  <Select
                    id="notification-frequency"
                    className="w-full"
                    value={displayedSettings.email_frequency}
                    onValueChange={(nextValue) =>
                      updatePreference({
                        email_frequency: nextValue,
                      })
                    }
                    disabled={updateSettings.isPending}
                    options={[
                      { value: 'immediate', label: String(t('notifications.frequencyImmediate')) },
                      { value: 'hourly', label: String(t('notifications.frequencyHourly')) },
                      { value: 'daily', label: String(t('notifications.frequencyDaily')) },
                      { value: 'never', label: String(t('notifications.frequencyNone')) },
                    ]}
                  />
                </div>
                <Switch
                  label={t('notifications.ownChanges')}
                  checked={displayedSettings.notify_own_changes}
                  onCheckedChange={(checked) =>
                    updatePreference({ notify_own_changes: checked === true })
                  }
                  disabled={updateSettings.isPending}
                />
                <Fieldset
                  className="border-t border-border pt-3"
                  legend={t('notifications.eventsTitle')}
                >
                  {NOTIFICATION_EVENTS.map((eventType) => (
                    <Checkbox
                      key={eventType}
                      label={t(`notifications.events.${eventType}`)}
                      checked={!displayedSettings.disabled_event_types.includes(eventType)}
                      onCheckedChange={(checked) => toggleEvent(eventType, checked === true)}
                      disabled={updateSettings.isPending}
                    />
                  ))}
                </Fieldset>
                <div className="space-y-2 border-t border-border pt-4">
                  {(updateSettings.isPending ||
                    updateSettings.isSuccess ||
                    updateSettings.isError) && (
                    <Notice
                      role="status"
                      aria-live="polite"
                      appearance="inline"
                      tone={
                        updateSettings.isError
                          ? 'danger'
                          : updateSettings.isSuccess
                            ? 'success'
                            : 'neutral'
                      }
                    >
                      {updateSettings.isPending && <span>{t('common.saving')}</span>}
                      {updateSettings.isSuccess && <span>{t('common.saved')}</span>}
                      {updateSettings.isError && (
                        <span>{t('notifications.settingsSaveError')}</span>
                      )}
                    </Notice>
                  )}
                  <div className="flex justify-end gap-2">
                    <Button
                      type="button"
                      variant="ghost"

                      disabled={!settingsDirty || updateSettings.isPending}
                      onClick={() => {
                        setSettingsDraft(null)
                        updateSettings.reset()
                      }}
                    >
                      {t('common.cancel')}
                    </Button>
                    <Button
                      type="submit"

                      disabled={!settingsDirty || updateSettings.isPending}
                    >
                      {t('common.save')}
                    </Button>
                  </div>
                </div>
              </form>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
