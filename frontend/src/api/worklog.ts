import type { Worklog, LogWorkInput } from '@/entities/worklog/model'
import { parseDuration } from '@/shared/lib/time'
import { api } from './client'

const WORKLOG_PAGE_SIZE = 500

function mapDto(w: {
  id: string
  issue_id: string
  author_id: string
  author_name?: string | null
  started_at: string
  duration_seconds: number
  description?: string | null
  created_at: string
  updated_at: string
}): Worklog {
  return {
    id: w.id,
    issueId: w.issue_id,
    userId: w.author_id,
    userDisplayName: w.author_name ?? '',
    timeSpentSeconds: w.duration_seconds,
    startedAt: w.started_at,
    comment: w.description ?? null,
    createdAt: w.created_at,
    updatedAt: w.updated_at,
  }
}

export async function listWorklogs(issueId: string): Promise<Worklog[]> {
  const worklogs: Worklog[] = []
  let offset = 0

  for (;;) {
    const page = await listWorklogPage(issueId, offset)
    worklogs.push(...page)

    if (page.length < WORKLOG_PAGE_SIZE) {
      break
    }
    offset += WORKLOG_PAGE_SIZE
  }

  return worklogs.sort((a, b) => b.startedAt.localeCompare(a.startedAt))
}

async function listWorklogPage(issueId: string, offset: number): Promise<Worklog[]> {
  const { data, error } = await api.GET('/api/v1/issues/{issue_id}/worklogs', {
    params: {
      path: { issue_id: issueId },
      query: { limit: WORKLOG_PAGE_SIZE, offset },
    },
  })
  if (error || !data) throw new Error('Failed to load worklogs')
  return data.worklogs.map(mapDto)
}

export async function createWorklog(issueId: string, input: LogWorkInput): Promise<Worklog> {
  const timeSpentSeconds = parseDuration(input.timeSpent)
  if (timeSpentSeconds === null || timeSpentSeconds === 0) {
    throw new Error('Invalid time spent')
  }

  const { data, error } = await api.POST('/api/v1/issues/{issue_id}/worklogs', {
    params: { path: { issue_id: issueId } },
    body: {
      started_at: input.startedAt ?? new Date().toISOString(),
      duration_seconds: timeSpentSeconds,
      description: input.comment?.trim() ?? null,
    },
  })
  if (error || !data) throw new Error('Failed to create worklog')
  return mapDto(data)
}

export async function updateWorklog(worklogId: string, input: LogWorkInput): Promise<Worklog> {
  const timeSpentSeconds = parseDuration(input.timeSpent)
  if (timeSpentSeconds === null || timeSpentSeconds === 0) {
    throw new Error('Invalid time spent')
  }

  const { data, error } = await api.PATCH('/api/v1/worklogs/{id}', {
    params: { path: { id: worklogId } },
    body: {
      started_at: input.startedAt,
      duration_seconds: timeSpentSeconds,
      description: input.comment?.trim() ?? null,
    },
  })
  if (error || !data) throw new Error('Failed to update worklog')
  return mapDto(data)
}

export async function deleteWorklog(worklogId: string): Promise<void> {
  const { error } = await api.DELETE('/api/v1/worklogs/{id}', {
    params: { path: { id: worklogId } },
  })
  if (error) throw new Error('Failed to delete worklog')
}
