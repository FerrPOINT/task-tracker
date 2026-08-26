import { useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { useAuthStore } from '@/shared/auth/store'

type TrackerEvent = {
  type: string
  issue_id?: string
  project_key?: string
  recipient_id?: string
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
    const es = new EventSource(`/api/v1/events?access_token=${token}`)

    es.addEventListener('tracker', (e) => {
      let evt: TrackerEvent | null = null
      try {
        evt = JSON.parse((e as MessageEvent).data)
      } catch {
        return
      }
      if (!evt) return

      const pk = evt.project_key
      switch (evt.type) {
        case 'issue_created':
        case 'issue_updated':
        case 'issue_deleted':
          if (pk) qc.invalidateQueries({ queryKey: ['project', pk] })
          qc.invalidateQueries({ queryKey: ['search'] })
          if (pk) qc.invalidateQueries({ queryKey: ['backlog', pk] })
          if (evt.issue_id) qc.invalidateQueries({ queryKey: ['issue', evt.issue_id] })
          break
        case 'issue_moved':
          if (pk) qc.invalidateQueries({ queryKey: ['project', pk] })
          qc.invalidateQueries({ queryKey: ['search'] })
          if (evt.issue_id) qc.invalidateQueries({ queryKey: ['issue', evt.issue_id] })
          break
        case 'issue_commented':
          if (evt.issue_id) qc.invalidateQueries({ queryKey: ['comments', evt.issue_id] })
          break
        case 'sprint_changed':
          qc.invalidateQueries({ queryKey: ['sprints'] })
          break
        case 'notification_created':
          qc.invalidateQueries({ queryKey: ['notifications'] })
          break
      }
    })

    es.onerror = () => {
      // EventSource auto-reconnects; nothing to do.
    }

    return () => es.close()
  }, [qc, token])
}
