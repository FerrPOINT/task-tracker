import { api } from './client'
import type { components } from './generated'

export type Issue = components['schemas']['IssueResponse']
export type Dashboard = components['schemas']['DashboardResponse']

export async function getDashboard(): Promise<Dashboard> {
  const { data, error } = await api.GET('/dashboard')
  if (error || !data) throw new Error('failed to load dashboard')
  return data
}
