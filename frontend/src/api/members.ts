import { api } from './client'
import type { components } from './generated'

export type ProjectMember = components['schemas']['ProjectMemberResponse']
type AddMemberInput = components['schemas']['AddProjectMemberRequest']

export async function listProjectMembers(projectId: string): Promise<{ members: ProjectMember[] }> {
  const { data, error } = await api.GET('/api/v1/projects/{project_id}/members', {
    params: { path: { project_id: projectId } },
  })
  if (error) throw new Error('Failed to load members')
  return { members: data?.members ?? [] }
}

export async function addProjectMember(projectId: string, input: AddMemberInput): Promise<void> {
  const { error } = await api.POST('/api/v1/projects/{project_id}/members', {
    params: { path: { project_id: projectId } },
    body: input,
  })
  if (error) throw new Error('Failed to add member')
}

export async function removeProjectMember(projectId: string, userId: string): Promise<void> {
  const { error } = await api.DELETE('/api/v1/projects/{project_id}/members/{user_id}', {
    params: { path: { project_id: projectId, user_id: userId } },
  })
  if (error) throw new Error('Failed to remove member')
}

export type AddProjectMemberInput = AddMemberInput
