import { api } from './client'
import { ApiError, InvalidResponseError } from '@pdlc/ui/lib'
import type { components } from './generated'

export type IssueLink = components['schemas']['IssueLinkResponse']

export async function listIssueLinks(issueId: string): Promise<IssueLink[]> {
  const { data, error, response } = await api.GET('/api/v1/issues/{issue_id}/links', {
    params: { path: { issue_id: issueId } },
  })
  if (error || !response.ok) throw new ApiError(response.status, 'Failed to load links')
  if (!data || !Array.isArray(data.links)) throw new InvalidResponseError(response.status)
  return data.links
}

export async function createIssueLink(
  issueId: string,
  targetKey: string,
  linkType: string,
): Promise<IssueLink> {
  const { data, error } = await api.POST('/api/v1/issues/{issue_id}/links', {
    params: { path: { issue_id: issueId } },
    body: { target_key: targetKey, link_type: linkType },
  })
  if (error || !data) throw new Error('Failed to create link')
  return data
}

export async function deleteIssueLink(id: string): Promise<void> {
  const { error } = await api.DELETE('/api/v1/issue-links/{id}', {
    params: { path: { id } },
  })
  if (error) throw new Error('Failed to delete link')
}
