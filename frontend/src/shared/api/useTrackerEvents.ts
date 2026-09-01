import { useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { useAuthStore } from '@/shared/auth/store'

type TrackerEvent = {
  type: string
  issue_id?: string
  project_key?: string
  recipient_id?: string
}

function invalidateIssueEventQueries(
  qc: ReturnType<typeof useQueryClient>,
  projectKey?: string,
  issueId?: string,
) {
  qc.invalidateQueries({ queryKey: ['projects'] })
  qc.invalidateQueries({ queryKey: ['dashboard'] })
  qc.invalidateQueries({ queryKey: ['search'] })
  if (projectKey) {
    qc.invalidateQueries({ queryKey: ['project', projectKey] })
    qc.invalidateQueries({ queryKey: ['backlog', projectKey] })
  } else {
    qc.invalidateQueries({ queryKey: ['project'] })
    qc.invalidateQueries({ queryKey: ['backlog'] })
  }
  if (issueId) {
    qc.invalidateQueries({ queryKey: ['issue', issueId] })
    qc.invalidateQueries({ queryKey: ['issue-labels', issueId] })
    qc.invalidateQueries({ queryKey: ['issue-links', issueId] })
    qc.invalidateQueries({ queryKey: ['issue-custom-fields', issueId] })
    qc.invalidateQueries({ queryKey: ['attachments', issueId] })
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
    let es: EventSource | null = null
    let retryDelay = 1000
    let closed = false
    let timer: ReturnType<typeof setTimeout> | undefined

    const connect = () => {
      if (closed) return
      es = new EventSource(`/api/v1/events?access_token=${encodeURIComponent(token)}`)

      es.addEventListener('tracker', (e) => {
        let evt: TrackerEvent | null = null
        try {
          evt = JSON.parse((e as MessageEvent).data)
        } catch {
          return
        }
        if (!evt) return
        // A healthy stream resets the reconnect backoff.
        retryDelay = 1000

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
              qc.invalidateQueries({ queryKey: ['issue', evt.issue_id] })
            }
            break
          case 'sprint_changed':
            qc.invalidateQueries({ queryKey: ['sprints'] })
            break
          case 'notification_created':
            qc.invalidateQueries({ queryKey: ['notifications'] })
            break
        }
      })

      es.onopen = () => {
        retryDelay = 1000
      }

      es.onerror = () => {
        // Reconnect manually with exponential backoff: EventSource's built-in
        // retry has no delay and hammers the rate limiter when the stream is
        // refused with 429, starving every other request from the same IP.
        es?.close()
        if (closed) return
        timer = setTimeout(connect, retryDelay)
        retryDelay = Math.min(retryDelay * 2, 15_000)
      }
    }

    connect()

    return () => {
      closed = true
      if (timer) clearTimeout(timer)
      es?.close()
    }
  }, [qc, token])
}
