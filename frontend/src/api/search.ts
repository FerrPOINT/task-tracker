import { api } from './client'
import type { components } from './generated'

export type Issue = components['schemas']['IssueResponse']

export interface SearchFilters {
  q?: string
  project_key?: string
  status?: string
  assignee_id?: string
  priority?: string
  sort_by?: string
  sort_order?: string
  jql?: string
  limit?: number
  offset?: number
}

export class SearchRequestError extends Error {
  constructor(public readonly status: number) {
    super('Failed to search')
  }
}

export async function searchIssues(filters: SearchFilters = {}): Promise<Issue[]> {
  const { data, response } = await api.GET('/api/v1/search', {
    params: {
      query: Object.fromEntries(
        Object.entries({
          q: filters.q,
          project_key: filters.project_key,
          status: filters.status,
          assignee_id: filters.assignee_id,
          priority: filters.priority,
          sort_by: filters.sort_by,
          sort_order: filters.sort_order,
          jql: filters.jql,
          limit: filters.limit,
          offset: filters.offset,
        }).filter(([, v]) => v !== undefined && v !== ''),
      ),
    },
  })
  if (!data) throw new SearchRequestError(response.status)
  return data.issues ?? []
}
