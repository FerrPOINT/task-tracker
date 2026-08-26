import type { components } from './generated'
import { api } from './client'

export type NotificationItem = components['schemas']['NotificationResponse']
export type NotificationSettings = components['schemas']['NotificationSettingsResponse']
export type UpdateNotificationSettingsInput =
  components['schemas']['UpdateNotificationSettingsRequest']

export async function listNotifications(): Promise<NotificationItem[]> {
  const { data, error } = await api.GET('/api/v1/notifications')
  if (!data) throw new Error(error ? JSON.stringify(error) : 'Failed to list notifications')
  return data.notifications
}

export async function markNotificationRead(id: string): Promise<void> {
  const { error } = await api.PATCH('/api/v1/notifications/{id}/read', {
    params: { path: { id } },
  })
  if (error) throw new Error(JSON.stringify(error))
}

export async function markAllNotificationsRead(): Promise<void> {
  const { error } = await api.POST('/api/v1/notifications/read-all')
  if (error) throw new Error(JSON.stringify(error))
}

export async function getNotificationSettings(): Promise<NotificationSettings> {
  const { data, error } = await api.GET('/api/v1/notification-settings')
  if (!data) throw new Error(error ? JSON.stringify(error) : 'Failed to get notification settings')
  return data
}

export async function updateNotificationSettings(
  input: UpdateNotificationSettingsInput,
): Promise<NotificationSettings> {
  const { data, error } = await api.PATCH('/api/v1/notification-settings', { body: input })
  if (!data)
    throw new Error(error ? JSON.stringify(error) : 'Failed to update notification settings')
  return data
}
