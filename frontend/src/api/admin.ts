import type { components } from './generated'
import { api } from './client'

type AuditLogEntry = components['schemas']['AuditLogResponse']
export type SystemSetting = components['schemas']['SystemSettingResponse']
export type UpdateSystemSettingInput = components['schemas']['UpdateSystemSettingRequest']

function requestError(error: unknown, fallback: string): Error {
  return new Error(error ? JSON.stringify(error) : fallback)
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
