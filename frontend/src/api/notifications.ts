import type { components } from './generated'
import { api } from './client'

export type NotificationItem = components['schemas']['NotificationResponse']
export type NotificationList = components['schemas']['NotificationListResponse']
export type NotificationSettings = components['schemas']['NotificationSettingsResponse']
export type UpdateNotificationSettingsInput =
  components['schemas']['UpdateNotificationSettingsRequest']

export type NotificationListOptions = {
  includeRead?: boolean
  limit?: number
  offset?: number
}

export async function listNotifications(
  options: NotificationListOptions = {},
): Promise<NotificationList> {
  const query: { include_read?: boolean; limit?: number; offset?: number } = {}
  if (options.includeRead !== undefined) query.include_read = options.includeRead
  if (options.limit !== undefined) query.limit = options.limit
  if (options.offset !== undefined) query.offset = options.offset
  const { data, error } = await api.GET('/api/v1/notifications', { params: { query } })
  if (!data) throw new Error(error ? JSON.stringify(error) : 'Failed to list notifications')
  return data
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
