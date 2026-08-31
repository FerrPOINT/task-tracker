import { api } from './client'
import type { components } from './generated'

type UpdateIssueInput = components['schemas']['UpdateIssueRequest']
export type Issue = components['schemas']['IssueResponse']

export async function updateIssue(id: string, input: UpdateIssueInput): Promise<Issue> {
  const { data, error } = await api.PATCH('/api/v1/issues/{id}', {
    params: { path: { id } },
    body: input,
  })
  if (error || !data) throw new Error('Failed to update issue')
  return data
}

export async function deleteIssue(id: string): Promise<void> {
  const { error } = await api.DELETE('/api/v1/issues/{id}', {
    params: { path: { id } },
  })
  if (error) throw new Error('Failed to delete issue')
}

export async function getIssue(id: string): Promise<Issue | null> {
  const { data, error } = await api.GET('/api/v1/issues/{id}', {
    params: { path: { id } },
  })
  if (error || !data) return null
  return data
}

export async function restoreIssue(id: string): Promise<Issue> {
  const { data, error } = await api.POST('/api/v1/issues/{id}/restore', {
    params: { path: { id } },
  })
  if (error || !data) throw new Error('Failed to restore issue')
  return data
}

export async function purgeIssue(id: string): Promise<void> {
  const { error } = await api.DELETE('/api/v1/issues/{id}/trash', {
    params: { path: { id } },
  })
  if (error) throw new Error('Failed to permanently delete issue')
}

export async function listTrash(projectKey: string, limit = 50, offset = 0): Promise<Issue[]> {
  const { data, error } = await api.GET('/api/v1/projects/{key}/trash', {
    params: { path: { key: projectKey }, query: { limit, offset } },
  })
  if (error || !data) throw new Error('Failed to list trash')
  return data.issues
}
