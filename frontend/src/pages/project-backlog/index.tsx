import { Link, useParams } from 'react-router'
import { Plus, MoreHorizontal, GripVertical } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import { useBacklog } from '@/shared/api/hooks'
import type { components } from '@/api/generated'

type Issue = components['schemas']['IssueResponse']

function PriorityBadge({ priority }: { priority: string }) {
  const color =
    priority === 'High'
      ? 'text-rose-500'
      : priority === 'Medium'
        ? 'text-amber-500'
        : 'text-emerald-500'
  return <span className={`text-xs font-medium ${color}`}>{priority}</span>
}

function Avatar({ name }: { name: string }) {
  const colors = ['bg-accent', 'bg-emerald-500', 'bg-amber-500', 'bg-rose-500']
  const color = colors[name.charCodeAt(0) % colors.length]
  return (
    <div
      className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-[10px] font-semibold text-white ${color}`}
    >
      {name.charAt(0).toUpperCase()}
    </div>
  )
}

function IssueRow({ issue }: { issue: Issue }) {
  return (
    <Link
      to={`/issues/${issue.id}`}
      className="group flex items-center gap-2 border-b border-border px-3 py-2.5 text-sm hover:bg-surface-raised sm:grid sm:grid-cols-[24px_80px_1fr_90px_40px] sm:gap-3"
    >
      <GripVertical className="h-4 w-4 shrink-0 text-text-muted sm:order-1" />
      <span className="shrink-0 text-text-muted sm:order-2">{issue.key}</span>
      <span className="min-w-0 flex-1 truncate font-medium sm:order-3">{issue.summary}</span>
      <div className="ml-auto flex shrink-0 items-center gap-2 sm:order-4 sm:ml-0">
        <PriorityBadge priority={issue.priority} />
        <Avatar name={issue.assignee_name ?? '?'} />
      </div>
    </Link>
  )
}

function Section({
  title,
  action,
  issues,
}: {
  title: string
  action?: React.ReactNode
  issues: Issue[]
}) {
  return (
    <div className="mb-5">
      <div className="flex flex-col gap-2 rounded-t-lg border border-border bg-surface px-3 py-2.5 sm:flex-row sm:items-center sm:justify-between">
        <span className="font-semibold">{title}</span>
        {action}
      </div>
      <div className="rounded-b-lg border-x border-b border-border bg-surface">
        {issues.map((issue) => (
          <IssueRow key={issue.id} issue={issue} />
        ))}
      </div>
    </div>
  )
}

export function ProjectBacklogPage() {
  const { t } = useTranslation()
  const { projectKey } = useParams<{ projectKey?: string }>()
  const key = projectKey ?? 'TT'
  const { data: backlog, isLoading, error } = useBacklog(key)

  if (isLoading) return <div className="p-4 text-text-muted">{t('issue.loading')}</div>
  if (error || !backlog)
    return <div className="p-4 text-rose-500">{error?.message ?? t('issue.notFound')}</div>

  const { sprint, sprint_issues, backlog_issues } = backlog

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="min-w-0">
          <h1 className="text-xl font-bold sm:text-2xl">
            {t('backlog.title', { projectName: key })}
          </h1>
          <div className="text-sm text-text-muted">
            {t('backlog.velocity', { velocity: sprint.velocity ?? '-' })} ·{' '}
            {t('backlog.backlogCount', { count: backlog_issues.length })}
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button size="sm" className="gap-1">
            <Plus className="h-4 w-4" />
            <span className="hidden sm:inline">{t('backlog.createSprint')}</span>
            <span className="sm:hidden">{t('backlog.createSprint')}</span>
          </Button>
          <Button variant="outline" size="sm" className="gap-1" asChild>
            <Link to="/issues/create">
              <Plus className="h-4 w-4" />
              <span className="hidden sm:inline">{t('backlog.createIssue')}</span>
              <span className="sm:hidden">{t('backlog.createIssue')}</span>
            </Link>
          </Button>
        </div>
      </div>

      <Section
        title={t('backlog.title', {
          projectName: `${sprint.name} · ${sprint.velocity} sp · ${sprint.remaining_days ?? '-'} ${t('issue.details')}`,
        })}
        action={
          <div className="flex items-center gap-2">
            <Button size="sm" className="h-7 px-2.5 text-xs">
              {t('backlog.startSprint')}
            </Button>
            <Button variant="ghost" size="icon" className="h-7 w-7">
              <MoreHorizontal className="h-4 w-4" />
            </Button>
          </div>
        }
        issues={sprint_issues}
      />

      <Section
        title={t('backlog.backlogSection', { count: backlog_issues.length })}
        action={
          <Button variant="outline" size="sm" className="h-7 px-2.5 text-xs" asChild>
            <Link to="/issues/create">
              <Plus className="h-4 w-4" />
              {t('navigation.create')}
            </Link>
          </Button>
        }
        issues={backlog_issues}
      />
    </div>
  )
}
