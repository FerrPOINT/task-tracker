import { api } from './client'
import type { components } from './generated'

export type MoveIssueInput = components['schemas']['MoveIssueRequest']
export type Board = components['schemas']['BoardResponse']
export type Backlog = components['schemas']['BacklogResponse']

export async function getBoard(projectKey: string): Promise<Board> {
  const { data, error } = await api.GET('/projects/{project_key}/board', {
    params: { path: { project_key: projectKey } },
  })
  if (error || !data) throw new Error('Failed to load board')
  return data
}

export async function getBacklog(projectKey: string): Promise<Backlog> {
  const { data, error } = await api.GET('/projects/{project_key}/backlog', {
    params: { path: { project_key: projectKey } },
  })
  if (error || !data) throw new Error('Failed to load backlog')
  return data
}

export async function moveIssue(
  projectKey: string,
  input: MoveIssueInput,
): Promise<Board> {
  const { data, error } = await api.POST('/projects/{project_key}/board/move', {
    params: { path: { project_key: projectKey } },
    body: input,
  })
  if (error || !data) throw new Error('Failed to move issue')
  return data
}
