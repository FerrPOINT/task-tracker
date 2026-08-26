import { api } from './client'
import type { components } from './generated'

export type Project = components['schemas']['ProjectResponse']
export type CreateProjectRequest = components['schemas']['CreateProjectRequest']
export type UpdateProjectRequest = components['schemas']['UpdateProjectRequest']

export async function listProjects(): Promise<Project[]> {
  const { data, error } = await api.GET('/api/v1/projects')
  if (error || !data) throw new Error('failed to load projects')
  return data.projects
}

export async function createProject(req: CreateProjectRequest): Promise<Project> {
  const { data, error } = await api.POST('/api/v1/projects', { body: req })
  if (error || !data) throw new Error('failed to create project')
  return data
}

export async function updateProject(key: string, req: UpdateProjectRequest): Promise<Project> {
  const { data, error } = await api.PATCH('/api/v1/projects/{project_key}', {
    params: { path: { project_key: key } },
    body: req,
  })
  if (error || !data) throw new Error('failed to update project')
  return data
}

export async function deleteProject(key: string): Promise<void> {
  const { error } = await api.DELETE('/api/v1/projects/{project_key}', {
    params: { path: { project_key: key } },
  })
  if (error) throw new Error('failed to delete project')
}
