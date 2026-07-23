import { api } from './client'
import type { components } from './generated'

export type CreateIssueRequest = components['schemas']['CreateIssueRequest']
export type CreateIssueResponse = components['schemas']['CreateIssueResponse']

export async function createIssue(req: CreateIssueRequest): Promise<CreateIssueResponse> {
  const { data, error } = await api.POST('/issues', {
    body: req,
  })
  if (error || !data) throw new Error('failed to create issue')
  return data
}
