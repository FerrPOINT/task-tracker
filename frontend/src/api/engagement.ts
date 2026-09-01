import { api } from './client'
import type { components } from './generated'

export type Vote = components['schemas']['VoteResponse']
export type VoteList = components['schemas']['VoteListResponse']
export type Watcher = components['schemas']['WatcherResponse']

export async function listIssueVotes(issueId: string): Promise<VoteList> {
  const { data, error } = await api.GET('/api/v1/issues/{issue_id}/votes', {
    params: { path: { issue_id: issueId } },
  })
  if (error || !data) throw new Error('Failed to load issue votes')
  return data
}

export async function voteIssue(issueId: string): Promise<Vote> {
  const { data, error } = await api.POST('/api/v1/issues/{issue_id}/vote', {
    params: { path: { issue_id: issueId } },
  })
  if (error || !data) throw new Error('Failed to vote for issue')
  return data
}

export async function unvoteIssue(issueId: string): Promise<void> {
  const { error } = await api.DELETE('/api/v1/issues/{issue_id}/vote', {
    params: { path: { issue_id: issueId } },
  })
  if (error) throw new Error('Failed to remove issue vote')
}

export async function listIssueWatchers(issueId: string): Promise<Watcher[]> {
  const { data, error } = await api.GET('/api/v1/issues/{issue_id}/watchers', {
    params: { path: { issue_id: issueId } },
  })
  if (error || !data) throw new Error('Failed to load issue watchers')
  return data.watchers
}

export async function watchIssue(issueId: string): Promise<void> {
  const { error } = await api.POST('/api/v1/issues/{issue_id}/watch', {
    params: { path: { issue_id: issueId } },
  })
  if (error) throw new Error('Failed to watch issue')
}

export async function unwatchIssue(issueId: string): Promise<void> {
  const { error } = await api.DELETE('/api/v1/issues/{issue_id}/watch', {
    params: { path: { issue_id: issueId } },
  })
  if (error) throw new Error('Failed to stop watching issue')
}
