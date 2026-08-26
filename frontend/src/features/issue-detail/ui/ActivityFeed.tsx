import { format } from 'date-fns'
import { useTranslation } from 'react-i18next'
import { MessageSquare, Clock } from 'lucide-react'
import type { Comment } from '@/entities/comment/model'
import type { Worklog } from '@/entities/worklog/model'
import { formatDuration } from '@/shared/lib/time'

type ActivityItem =
  { type: 'comment'; id: string; data: Comment } | { type: 'worklog'; id: string; data: Worklog }

interface ActivityFeedProps {
  comments: Comment[]
  worklogs: Worklog[]
}

function buildActivity(comments: Comment[], worklogs: Worklog[]): ActivityItem[] {
  const items: ActivityItem[] = [
    ...comments.map((c) => ({ type: 'comment' as const, id: `comment-${c.id}`, data: c })),
    ...worklogs.map((w) => ({ type: 'worklog' as const, id: `worklog-${w.id}`, data: w })),
  ]
  return items.sort(
    (a, b) => new Date(b.data.createdAt).getTime() - new Date(a.data.createdAt).getTime(),
  )
}

export function ActivityFeed({ comments, worklogs }: ActivityFeedProps) {
  const { t } = useTranslation()
  const activity = buildActivity(comments, worklogs)

  if (activity.length === 0) {
    return <p className="text-sm text-text-muted">{t('issue.noActivity')}</p>
  }

  return (
    <div className="space-y-4">
      {activity.map((item) => {
        if (item.type === 'comment') {
          const c = item.data
          return (
            <div
              key={item.id}
              className="flex gap-3 rounded-lg border border-border bg-surface p-4"
            >
              <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-accent/20">
                <MessageSquare className="h-4 w-4 text-accent" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="mb-1 flex flex-wrap items-center gap-2 text-sm">
                  <span className="font-medium text-text-primary">
                    {c.authorName ?? t('comments.unknown')}
                  </span>
                  <span className="text-text-muted">{t('issue.commented')}</span>
                  <span className="text-xs text-text-muted">
                    {format(new Date(c.createdAt), 'yyyy-MM-dd HH:mm')}
                  </span>
                </div>
                <div className="whitespace-pre-wrap text-sm text-text-secondary">{c.body}</div>
              </div>
            </div>
          )
        }

        const w = item.data
        return (
          <div key={item.id} className="flex gap-3 rounded-lg border border-border bg-surface p-4">
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-emerald-500/20">
              <Clock className="h-4 w-4 text-emerald-500" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="mb-1 flex flex-wrap items-center gap-2 text-sm">
                <span className="font-medium text-text-primary">{w.userDisplayName}</span>
                <span className="text-text-muted">
                  {t('issue.loggedTime', { duration: formatDuration(w.timeSpentSeconds) })}
                </span>
                <span className="text-xs text-text-muted">
                  {format(new Date(w.createdAt), 'yyyy-MM-dd HH:mm')}
                </span>
              </div>
              {w.comment && (
                <div className="whitespace-pre-wrap text-sm text-text-secondary">{w.comment}</div>
              )}
            </div>
          </div>
        )
      })}
    </div>
  )
}
