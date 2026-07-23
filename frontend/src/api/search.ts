import { api } from './client'
import type { components } from './generated'

export type SearchResult = components['schemas']['SearchResultResponse']

export async function searchIssues(q?: string): Promise<SearchResult> {
  const { data, error } = await api.GET('/search', {
    params: { query: { q: q ?? '' } },
  })
  if (error || !data) throw new Error('failed to search')
  return data
}
