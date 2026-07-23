import { api } from './client'
import type { components } from './generated'

export type UpdateIssueInput = components['schemas']['UpdateIssueRequest']
export type Issue = components['schemas']['IssueResponse']

export async function updateIssue(
  id: string,
  input: UpdateIssueInput,
): Promise<Issue> {
  const { data, error } = await api.PATCH('/issues/{id}', {
    params: { path: { id } },
    body: input,
  })
  if (error || !data) throw new Error('Failed to update issue')
  return data
}

export async function getIssue(id: string): Promise<Issue | null> {
  const { data, error } = await api.GET('/issues/{id}', {
    params: { path: { id } },
  })
  if (error || !data) return null
  return data
}
