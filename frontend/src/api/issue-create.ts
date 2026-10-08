import { api } from './client'
import type { components } from './generated'

export type CreateIssueInput = components['schemas']['CreateIssueRequest'] & {
  operation_id?: string
}
export type Issue = components['schemas']['IssueResponse']

export async function createIssue(input: CreateIssueInput): Promise<Issue> {
  const { operation_id, ...body } = input
  const { data, error } = await api.POST('/api/v1/issues', {
    body,
    headers: operation_id ? { 'Idempotency-Key': operation_id } : undefined,
  })
  if (error || !data) throw new Error('Failed to create issue')
  return data
}
