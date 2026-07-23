import { api } from './client'
import type { components } from './generated'

export type BoardResponse = components['schemas']['BoardResponse']
export type BacklogResponse = components['schemas']['BacklogResponse']
export type Issue = components['schemas']['IssueResponse']

export async function getBoard(projectKey: string): Promise<BoardResponse> {
  const { data, error } = await api.GET('/projects/{project_key}/board', {
    params: { path: { project_key: projectKey } },
  })
  if (error || !data) throw new Error('failed to load board')
  return data
}

export async function getBacklog(projectKey: string): Promise<BacklogResponse> {
  const { data, error } = await api.GET('/projects/{project_key}/backlog', {
    params: { path: { project_key: projectKey } },
  })
  if (error || !data) throw new Error('failed to load backlog')
  return data
}

export type { components }
