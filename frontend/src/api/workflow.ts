import { api } from './client'
import type { components } from './generated'

export type Status = components['schemas']['StatusResponse']
export type Transition = components['schemas']['TransitionResponse']
export type IssueType = components['schemas']['IssueTypeResponse']

export async function listStatuses(): Promise<Status[]> {
  const { data, error } = await api.GET('/api/v1/statuses')
  if (error || !data) throw new Error('Failed to load statuses')
  return data
}

export async function listTransitions(): Promise<Transition[]> {
  const { data, error } = await api.GET('/api/v1/transitions')
  if (error || !data) throw new Error('Failed to load transitions')
  return data
}

export async function listIssueTypes(): Promise<IssueType[]> {
  const { data, error } = await api.GET('/api/v1/issue-types')
  if (error || !data) throw new Error('Failed to load issue types')
  return data
}
