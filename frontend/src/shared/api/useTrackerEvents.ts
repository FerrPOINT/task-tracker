import { useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { useAuthStore } from '@/shared/auth/store'
import { connectEventStream } from '@sdlc/ui/lib'

type TrackerEvent = {
  type: string
  issue_id?: string
  project_key?: string
  recipient_id?: string
}

function invalidateIssueCollectionQueries(
  qc: ReturnType<typeof useQueryClient>,
  projectKey?: string,
) {
  qc.invalidateQueries({ queryKey: ['projects'] })
  qc.invalidateQueries({ queryKey: ['dashboard'] })
  qc.invalidateQueries({ queryKey: ['search'] })
  qc.invalidateQueries({ queryKey: ['reports'] })
  if (projectKey) {
    qc.invalidateQueries({ queryKey: ['project', projectKey] })
    qc.invalidateQueries({ queryKey: ['backlog', projectKey] })
  } else {
    qc.invalidateQueries({ queryKey: ['project'] })
    qc.invalidateQueries({ queryKey: ['backlog'] })
  }
}

function invalidateIssueSummaryQueries(
  qc: ReturnType<typeof useQueryClient>,
  projectKey?: string,
  issueId?: string,
) {
  invalidateIssueCollectionQueries(qc, projectKey)
  if (issueId) {
    qc.invalidateQueries({ queryKey: ['issue', issueId] })
  }
}

function invalidateIssueEventQueries(
  qc: ReturnType<typeof useQueryClient>,
  projectKey?: string,
  issueId?: string,
) {
  invalidateIssueSummaryQueries(qc, projectKey, issueId)
  if (issueId) {
    qc.invalidateQueries({ queryKey: ['issue-labels', issueId] })
    qc.invalidateQueries({ queryKey: ['issue-links', issueId] })
    qc.invalidateQueries({ queryKey: ['issue-custom-fields', issueId] })
    qc.invalidateQueries({ queryKey: ['attachments', issueId] })
    qc.invalidateQueries({ queryKey: ['issue-votes', issueId] })
    qc.invalidateQueries({ queryKey: ['issue-watchers', issueId] })
  }
}

function invalidateSprintEventQueries(qc: ReturnType<typeof useQueryClient>, projectKey?: string) {
  qc.invalidateQueries({ queryKey: ['sprints'] })
  qc.invalidateQueries({ queryKey: ['reports'] })
  if (projectKey) {
    qc.invalidateQueries({ queryKey: ['project', projectKey] })
    qc.invalidateQueries({ queryKey: ['backlog', projectKey] })
  } else {
    qc.invalidateQueries({ queryKey: ['project'] })
    qc.invalidateQueries({ queryKey: ['backlog'] })
  }
}

/**
 * Subscribe to the backend SSE stream (`/api/v1/events`) and invalidate
 * the affected TanStack Query caches. Bearer auth is passed via a short-lived
 * token query param (EventSource cannot set headers); the backend accepts
 * `access_token` query auth for this endpoint.
 */
export function useTrackerEvents() {
  const qc = useQueryClient()
  const token = useAuthStore((s) => s.token)

  useEffect(() => {
    if (!token) return
    // Transport (query-token auth, exponential backoff) comes from the
    // shared fleet kit; this hook only maps events to cache invalidations.
    return connectEventStream({
      url: `/api/v1/events?access_token=${encodeURIComponent(token)}`,
      eventTypes: ['tracker'],
      onEvent: (_type, payload) => {
        const evt = payload as TrackerEvent
        const pk = evt.project_key
        switch (evt.type) {
          case 'issue_created':
          case 'issue_updated':
          case 'issue_deleted':
          case 'issue_moved':
            invalidateIssueEventQueries(qc, pk, evt.issue_id)
            break
          case 'issue_commented':
            if (evt.issue_id) qc.invalidateQueries({ queryKey: ['comments', evt.issue_id] })
            break
          case 'worklog_logged':
            if (evt.issue_id) {
              qc.invalidateQueries({ queryKey: ['worklogs', evt.issue_id] })
              invalidateIssueSummaryQueries(qc, pk, evt.issue_id)
            }
            break
          case 'sprint_changed':
            invalidateSprintEventQueries(qc, pk)
            break
          case 'notification_created':
            qc.invalidateQueries({ queryKey: ['notifications'] })
            break
        }
      },
    })
  }, [qc, token])
}
