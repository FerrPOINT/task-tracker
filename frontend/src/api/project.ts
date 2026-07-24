import { api } from './client'
import type { components } from './generated'

export type Project = components['schemas']['ProjectResponse']

export async function listProjects(): Promise<Project[]> {
  const { data, error } = await api.GET('/api/v1/projects')
  if (error || !data) throw new Error('failed to load projects')
  return data.projects
}
