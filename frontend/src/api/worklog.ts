import type { Worklog, CreateWorklogPayload, LogWorkInput } from '@/entities/worklog/model'
import { parseDuration, formatDuration } from '@/shared/lib/time'
import { recalcIssueTime } from './issue'

let nextId = 1

const worklogs: Worklog[] = [
  {
    id: `wl-${nextId++}`,
    issueId: 'issue-1',
    userId: 'user-1',
    userDisplayName: 'Ivan',
    timeSpentSeconds: 3600,
    remainingEstimateSeconds: 4 * 3600,
    startedAt: '2026-07-19T09:00:00Z',
    comment: 'Initial setup',
    createdAt: '2026-07-19T09:00:00Z',
    updatedAt: '2026-07-19T09:00:00Z',
  },
  {
    id: `wl-${nextId++}`,
    issueId: 'issue-1',
    userId: 'user-2',
    userDisplayName: 'Anna',
    timeSpentSeconds: 7200,
    remainingEstimateSeconds: 4 * 3600,
    startedAt: '2026-07-20T10:00:00Z',
    comment: 'API integration',
    createdAt: '2026-07-20T10:00:00Z',
    updatedAt: '2026-07-20T10:00:00Z',
  },
]

recalcIssueTime(worklogs)

function buildWorklog(issueId: string, payload: CreateWorklogPayload): Worklog {
  const now = new Date().toISOString()
  return {
    id: `wl-${nextId++}`,
    issueId,
    userId: 'user-1',
    userDisplayName: 'Ivan',
    timeSpentSeconds: payload.timeSpentSeconds,
    remainingEstimateSeconds: payload.remainingEstimateSeconds,
    startedAt: payload.startedAt,
    comment: payload.comment,
    createdAt: now,
    updatedAt: now,
  }
}

export async function listWorklogs(issueId: string): Promise<Worklog[]> {
  await delay(150)
  return worklogs.filter((w) => w.issueId === issueId).sort((a, b) => b.startedAt.localeCompare(a.startedAt))
}

export async function createWorklog(issueId: string, input: LogWorkInput): Promise<Worklog> {
  const timeSpentSeconds = parseDuration(input.timeSpent)
  if (timeSpentSeconds === null || timeSpentSeconds === 0) {
    throw new Error('Invalid time spent')
  }
  const remainingEstimateSeconds = input.remainingEstimate?.trim()
    ? parseDuration(input.remainingEstimate)
    : null

  const payload: CreateWorklogPayload = {
    timeSpentSeconds,
    remainingEstimateSeconds,
    startedAt: input.startedAt ?? new Date().toISOString(),
    comment: input.comment?.trim() ?? null,
  }

  await delay(200)
  const worklog = buildWorklog(issueId, payload)
  worklogs.push(worklog)
  recalcIssueTime(worklogs)
  return worklog
}

export async function updateWorklog(
  worklogId: string,
  input: LogWorkInput,
): Promise<Worklog> {
  const index = worklogs.findIndex((w) => w.id === worklogId)
  if (index === -1) throw new Error('Worklog not found')
  const existing = worklogs[index]!

  const timeSpentSeconds = parseDuration(input.timeSpent)
  if (timeSpentSeconds === null || timeSpentSeconds === 0) {
    throw new Error('Invalid time spent')
  }
  const remainingEstimateSeconds = input.remainingEstimate?.trim()
    ? parseDuration(input.remainingEstimate)
    : null

  await delay(200)
  const updated: Worklog = {
    ...existing,
    timeSpentSeconds,
    remainingEstimateSeconds,
    startedAt: input.startedAt ?? existing.startedAt,
    comment: input.comment?.trim() ?? null,
    updatedAt: new Date().toISOString(),
  }
  worklogs[index] = updated
  recalcIssueTime(worklogs)
  return updated
}

export async function deleteWorklog(worklogId: string): Promise<void> {
  const index = worklogs.findIndex((w) => w.id === worklogId)
  if (index === -1) throw new Error('Worklog not found')
  await delay(150)
  worklogs.splice(index, 1)
  recalcIssueTime(worklogs)
}

export function toHuman(seconds: number): string {
  return formatDuration(seconds)
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}
