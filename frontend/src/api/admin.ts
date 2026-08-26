import type { components } from './generated'
import { api } from './client'

export type AdminUser = components['schemas']['AdminUserResponse']
export type CreateAdminUserInput = components['schemas']['AdminCreateUserRequest']
type AuditLogEntry = components['schemas']['AuditLogResponse']
export type SystemSetting = components['schemas']['SystemSettingResponse']
export type UpdateSystemSettingInput = components['schemas']['UpdateSystemSettingRequest']

function requestError(error: unknown, fallback: string): Error {
  return new Error(error ? JSON.stringify(error) : fallback)
}

export async function listAdminUsers(): Promise<AdminUser[]> {
  const { data, error } = await api.GET('/api/v1/admin/users')
  if (!data) throw requestError(error, 'Failed to list admin users')
  return data.users
}

export async function createAdminUser(input: CreateAdminUserInput): Promise<AdminUser> {
  const { data, error } = await api.POST('/api/v1/admin/users', { body: input })
  if (!data) throw requestError(error, 'Failed to create admin user')
  return data
}

export async function updateAdminUserStatus(input: {
  id: string
  is_active: boolean
}): Promise<void> {
  const { error } = await api.PUT('/api/v1/admin/users/{id}/status', {
    params: { path: { id: input.id } },
    body: { is_active: input.is_active },
  })
  if (error) throw requestError(error, 'Failed to update user status')
}

export async function listAdminSettings(): Promise<SystemSetting[]> {
  const { data, error } = await api.GET('/api/v1/admin/system-settings')
  if (!data) throw requestError(error, 'Failed to list system settings')
  return data.settings
}

export async function updateAdminSetting(input: UpdateSystemSettingInput): Promise<SystemSetting> {
  const { data, error } = await api.PUT('/api/v1/admin/system-settings', { body: input })
  if (!data) throw requestError(error, 'Failed to update system setting')
  return data
}

export async function listAdminAuditLog(limit = 100): Promise<AuditLogEntry[]> {
  const { data, error } = await api.GET('/api/v1/admin/audit-log', { params: { query: { limit } } })
  if (!data) throw requestError(error, 'Failed to list audit log')
  return data.entries
}
