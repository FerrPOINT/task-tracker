import { api } from './client'
import type { components } from './generated'

export type ProjectMember = components['schemas']['ProjectMemberResponse']
type AddMemberInput = components['schemas']['AddProjectMemberRequest']

export async function listProjectMembers(
  projectKey: string,
): Promise<{ members: ProjectMember[] }> {
  const { data, error } = await api.GET('/api/v1/projects/{project_key}/members', {
    params: { path: { project_key: projectKey } },
  })
  if (error) throw new Error('Failed to load members')
  return { members: data?.members ?? [] }
}

export async function addProjectMember(projectKey: string, input: AddMemberInput): Promise<void> {
  const { error } = await api.POST('/api/v1/projects/{project_key}/members', {
    params: { path: { project_key: projectKey } },
    body: input,
  })
  if (error) throw new Error('Failed to add member')
}

export async function removeProjectMember(projectKey: string, userId: string): Promise<void> {
  const { error } = await api.DELETE('/api/v1/projects/{project_key}/members/{user_id}', {
    params: { path: { project_key: projectKey, user_id: userId } },
  })
  if (error) throw new Error('Failed to remove member')
}

export type AddProjectMemberInput = AddMemberInput
