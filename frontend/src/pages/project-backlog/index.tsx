import { useState } from 'react'
import { Link, useParams } from 'react-router'
import { Plus, MoreHorizontal, GripVertical, Play, CheckCircle2, Pencil } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/shared/ui/dropdown-menu'
import { useBacklog, useSprints, useCreateSprint, useUpdateSprint, useStartSprint, useCloseSprint } from '@/shared/api/hooks'
import { SprintFormDialog } from '@/features/sprints/ui/SprintFormDialog'
import type { components } from '@/api/generated'
import type { Sprint, CreateSprintRequest, UpdateSprintRequest } from '@/api/sprint'

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

function IssueRow({
  issue,
  action,
}: {
  issue: Issue
  action?: React.ReactNode
}) {
  return (
    <div className="group flex items-center gap-2 border-b border-border px-3 py-2.5 text-sm hover:bg-surface-raised sm:grid sm:grid-cols-[24px_80px_1fr_90px_40px_40px] sm:gap-3">
      <GripVertical className="h-4 w-4 shrink-0 text-text-muted sm:order-1" />
      <Link to={`/issues/${issue.id}`} className="contents">
        <span className="shrink-0 text-text-muted sm:order-2">{issue.key}</span>
        <span className="min-w-0 flex-1 truncate font-medium sm:order-3">{issue.summary}</span>
      </Link>
      <div className="ml-auto flex shrink-0 items-center gap-2 sm:order-4 sm:ml-0">
        <PriorityBadge priority={issue.priority} />
        <Avatar name={issue.assignee_name ?? '?'} />
      </div>
      <div className="sm:order-5" />
      <div className="sm:order-6">{action}</div>
    </div>
  )
}

function Section({
  title,
  action,
  issues,
  emptyText,
}: {
  title: string
  action?: React.ReactNode
  issues: Issue[]
  emptyText?: string
}) {
  return (
    <div className="mb-5">
      <div className="flex flex-col gap-2 rounded-t-lg border border-border bg-surface px-3 py-2.5 sm:flex-row sm:items-center sm:justify-between">
        <span className="font-semibold">{title}</span>
        {action}
      </div>
      <div className="rounded-b-lg border-x border-b border-border bg-surface">
        {issues.length === 0 && emptyText && (
          <div className="px-3 py-6 text-center text-sm text-text-muted">{emptyText}</div>
        )}
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
  const { data: backlog, isLoading: backlogLoading, error: backlogError } = useBacklog(key)
  const { data: sprints, isLoading: sprintsLoading } = useSprints(key)
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editingSprint, setEditingSprint] = useState<Sprint | null>(null)

  const createSprint = useCreateSprint(key)
  const updateSprint = useUpdateSprint(key, editingSprint?.id ?? '')
  const startSprint = useStartSprint(key)
  const closeSprint = useCloseSprint(key)

  const isLoading = backlogLoading || sprintsLoading
  const error = backlogError

  if (isLoading) return <div className="p-4 text-text-muted">{t('issue.loading')}</div>
  if (error || !backlog)
    return <div className="p-4 text-rose-500">{error?.message ?? t('issue.notFound')}</div>

  const { sprint: activeSprint, sprint_issues, backlog_issues } = backlog
  const futureSprints = sprints?.filter((s) => s.id !== activeSprint.id && s.state !== 'closed') ?? []
  const activeFromList = sprints?.find((s) => s.id === activeSprint.id)
  const activeSprintName = activeFromList?.name ?? activeSprint.name

  function openCreate() {
    setEditingSprint(null)
    setDialogOpen(true)
  }

  function openEdit(sprint: Sprint) {
    setEditingSprint(sprint)
    setDialogOpen(true)
  }

  function handleSubmit(values: CreateSprintRequest | UpdateSprintRequest) {
    if (editingSprint) {
      updateSprint.mutate(values as UpdateSprintRequest, {
        onSuccess: () => setDialogOpen(false),
      })
    } else {
      createSprint.mutate(values as CreateSprintRequest, {
        onSuccess: () => setDialogOpen(false),
      })
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="min-w-0">
          <h1 className="text-xl font-bold sm:text-2xl">{t('backlog.title', { projectName: key })}</h1>
          <div className="text-sm text-text-muted">
            {t('backlog.velocity', { velocity: activeSprint.velocity ?? '-' })} ·{' '}
            {t('backlog.backlogCount', { count: backlog_issues.length })}
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button size="sm" className="gap-1" onClick={openCreate}>
            <Plus className="h-4 w-4" />
            {t('backlog.createSprint')}
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
        title={t('backlog.activeSprint', {
          name: activeSprintName,
          velocity: activeSprint.velocity,
          remaining: activeSprint.remaining_days ?? '-',
        })}
        action={
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              className="h-7 px-2.5 text-xs"
              onClick={() => startSprint.mutate(activeSprint.id)}
              disabled={activeFromList?.state === 'active'}
            >
              <Play className="mr-1 h-3 w-3" />
              {t('backlog.startSprint')}
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="h-7 px-2.5 text-xs"
              onClick={() => closeSprint.mutate(activeSprint.id)}
              disabled={activeFromList?.state !== 'active'}
            >
              <CheckCircle2 className="mr-1 h-3 w-3" />
              {t('backlog.closeSprint')}
            </Button>
            {activeFromList && (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="ghost" size="icon" className="h-7 w-7">
                    <MoreHorizontal className="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem onClick={() => openEdit(activeFromList)}>
                    <Pencil className="mr-2 h-4 w-4" />
                    {t('common.edit')}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            )}
          </div>
        }
        issues={sprint_issues}
        emptyText={t('backlog.emptySprint')}
      />

      {futureSprints.map((sprint) => (
        <Section
          key={sprint.id}
          title={t('backlog.futureSprint', { name: sprint.name })}
          action={
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                className="h-7 px-2.5 text-xs"
                onClick={() => startSprint.mutate(sprint.id)}
              >
                <Play className="mr-1 h-3 w-3" />
                {t('backlog.startSprint')}
              </Button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="ghost" size="icon" className="h-7 w-7">
                    <MoreHorizontal className="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuItem onClick={() => openEdit(sprint)}>
                    <Pencil className="mr-2 h-4 w-4" />
                    {t('common.edit')}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          }
          issues={[]}
          emptyText={t('backlog.emptySprint')}
        />
      ))}

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
        emptyText={t('backlog.emptyBacklog')}
      />

      <SprintFormDialog
        open={dialogOpen}
        sprint={editingSprint}
        onOpenChange={setDialogOpen}
        onSubmit={handleSubmit}
        isPending={createSprint.isPending || updateSprint.isPending}
        error={createSprint.error ?? updateSprint.error}
      />
    </div>
  )
}
