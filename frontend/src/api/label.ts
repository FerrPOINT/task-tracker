import { api } from './client'
import type { components } from './generated'

export type Label = components['schemas']['LabelResponse']

export async function listProjectLabels(projectKey: string): Promise<Label[]> {
  const { data, error } = await api.GET('/api/v1/projects/{project_key}/labels', {
    params: { path: { project_key: projectKey } },
  })
  if (error || !data) throw new Error('Failed to load labels')
  return data.labels
}

export async function createLabel(projectKey: string, name: string, color: string): Promise<Label> {
  const { data, error } = await api.POST('/api/v1/projects/{project_key}/labels', {
    params: { path: { project_key: projectKey } },
    body: { name, color },
  })
  if (error || !data) throw new Error('Failed to create label')
  return data
}

export async function listIssueLabels(issueId: string): Promise<Label[]> {
  const { data, error } = await api.GET('/api/v1/issues/{issue_id}/labels', {
    params: { path: { issue_id: issueId } },
  })
  if (error || !data) throw new Error('Failed to load issue labels')
  return data.labels
}

export async function attachLabel(issueId: string, labelId: string): Promise<void> {
  const { error } = await api.POST('/api/v1/issues/{issue_id}/labels', {
    params: { path: { issue_id: issueId } },
    body: { label_id: labelId },
  })
  if (error) throw new Error('Failed to attach label')
}

export async function detachLabel(issueId: string, labelId: string): Promise<void> {
  const { error } = await api.DELETE('/api/v1/issues/{issue_id}/labels/{label_id}', {
    params: { path: { issue_id: issueId, label_id: labelId } },
  })
  if (error) throw new Error('Failed to detach label')
}
