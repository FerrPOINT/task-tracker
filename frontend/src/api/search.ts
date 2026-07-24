import { api } from './client'
import type { components } from './generated'

export type Issue = components['schemas']['IssueResponse']

export async function searchIssues(q?: string): Promise<Issue[]> {
  const { data } = await api.GET('/api/v1/search', {
    params: { query: { q: q ?? '' } },
  })
  if (!data) throw new Error('Failed to search')
  return data.issues ?? []
}
