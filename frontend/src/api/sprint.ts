import { api } from './client'
import type { components } from './generated'

export type Sprint = components['schemas']['SprintResponse']
export type SprintList = components['schemas']['SprintListResponse']
export type CreateSprintRequest = components['schemas']['CreateSprintRequest']
export type UpdateSprintRequest = components['schemas']['UpdateSprintRequest']
export type MoveIssueToSprintRequest = components['schemas']['MoveIssueToSprintRequest']

export async function listSprints(projectKey: string): Promise<Sprint[]> {
  const { data, error } = await api.GET('/api/v1/projects/{project_key}/sprints', {
    params: { path: { project_key: projectKey } },
  })
  if (error || !data) throw new Error('failed to load sprints')
  return data.sprints
}

export async function createSprint(
  projectKey: string,
  req: CreateSprintRequest,
): Promise<Sprint> {
  const { data, error } = await api.POST('/api/v1/projects/{project_key}/sprints', {
    params: { path: { project_key: projectKey } },
    body: req,
  })
  if (error || !data) throw new Error('failed to create sprint')
  return data
}

export async function updateSprint(
  projectKey: string,
  sprintId: string,
  req: UpdateSprintRequest,
): Promise<Sprint> {
  const { data, error } = await api.PATCH('/api/v1/projects/{project_key}/sprints/{sprint_id}', {
    params: { path: { project_key: projectKey, sprint_id: sprintId } },
    body: req,
  })
  if (error || !data) throw new Error('failed to update sprint')
  return data
}

export async function startSprint(projectKey: string, sprintId: string): Promise<Sprint> {
  const { data, error } = await api.POST(
    '/api/v1/projects/{project_key}/sprints/{sprint_id}/start',
    {
      params: { path: { project_key: projectKey, sprint_id: sprintId } },
    },
  )
  if (error || !data) throw new Error('failed to start sprint')
  return data
}

export async function closeSprint(projectKey: string, sprintId: string): Promise<Sprint> {
  const { data, error } = await api.POST(
    '/api/v1/projects/{project_key}/sprints/{sprint_id}/close',
    {
      params: { path: { project_key: projectKey, sprint_id: sprintId } },
    },
  )
  if (error || !data) throw new Error('failed to close sprint')
  return data
}

export async function moveIssueToSprint(
  projectKey: string,
  sprintId: string,
  issueId: string,
): Promise<components['schemas']['IssueResponse']> {
  const { data, error } = await api.POST(
    '/api/v1/projects/{project_key}/sprints/{sprint_id}/issues',
    {
      params: { path: { project_key: projectKey, sprint_id: sprintId } },
      body: { issue_id: issueId },
    },
  )
  if (error || !data) throw new Error('failed to move issue to sprint')
  return data
}

export async function removeIssueFromSprint(
  projectKey: string,
  sprintId: string,
  issueId: string,
): Promise<components['schemas']['IssueResponse']> {
  const { data, error } = await api.POST(
    '/api/v1/projects/{project_key}/sprints/{sprint_id}/remove-issue',
    {
      params: { path: { project_key: projectKey, sprint_id: sprintId } },
      body: { issue_id: issueId },
    },
  )
  if (error || !data) throw new Error('failed to remove issue from sprint')
  return data
}
