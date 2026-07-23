import { api } from './client'
import type { components } from './generated'

export type Issue = components['schemas']['IssueResponse']

export async function getIssue(id: string): Promise<Issue | null> {
  const { data, error } = await api.GET('/issues/{id}', {
    params: { path: { id } },
  })
  if (error || !data) return null
  return data
}
