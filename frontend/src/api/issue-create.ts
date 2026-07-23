import { api } from './client'
import type { components } from './generated'

export type CreateIssueInput = components['schemas']['CreateIssueRequest']
export type Issue = components['schemas']['IssueResponse']

export async function createIssue(input: CreateIssueInput): Promise<Issue> {
  const { data, error } = await api.POST('/issues', {
    body: input,
  })
  if (error || !data) throw new Error('Failed to create issue')
  return data
}
