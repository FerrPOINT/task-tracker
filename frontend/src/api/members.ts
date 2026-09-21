import { api } from './client'
import type { components } from './generated'

export type ProjectMember = components['schemas']['ProjectMemberResponse']

export async function listProjectMembers(
  projectKey: string,
): Promise<{ members: ProjectMember[] }> {
  const { data, error } = await api.GET('/api/v1/projects/{project_key}/members', {
    params: { path: { project_key: projectKey } },
  })
  if (error || !data) throw new Error('Failed to load project members')
  return data
}
