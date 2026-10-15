import { api } from './client'
import type { components } from './generated'

export type CreateIssueInput = components['schemas']['CreateIssueRequest'] & {
  operation_id?: string
}
export type Issue = components['schemas']['IssueResponse']

export class IssueCreationValidationError extends Error {
  constructor() {
    super('Failed to create issue')
    this.name = 'IssueCreationValidationError'
  }
}

export async function createIssue(input: CreateIssueInput): Promise<Issue> {
  const { operation_id, ...body } = input
  const { data, error, response } = await api.POST('/api/v1/issues', {
    body,
    headers: operation_id ? { 'Idempotency-Key': operation_id } : undefined,
  })
  if (error || !data) {
    const code = (error as { error?: { code?: unknown } } | undefined)?.error?.code
    if ((response.status === 400 || response.status === 422) && code === 'VALIDATION_ERROR') {
      throw new IssueCreationValidationError()
    }
    throw new Error('Failed to create issue')
  }
  return data
}
