import { Link, useParams, useSearchParams } from 'react-router'
import { List, MoreHorizontal } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useState } from 'react'
import {
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@sdlc/ui/ui'
import { ErrorState, LoadingState } from '@sdlc/ui/ui'
import { useBoard, useMoveIssue, useTransitions } from '@/shared/api/hooks'
import { ProjectMembersPanel } from '@/features/project-members/ui/ProjectMembersPanel'
import { UserAvatar } from '@/shared/ui/user-avatar'
import type { components } from '@/api/generated'
import { toast } from 'sonner'
import { statusLabel } from '@/shared/lib/status-label'

export type Issue = components['schemas']['IssueResponse']

type DragState = {
  issueId: string | null
  sourceColumnId: string | null
  dragging: boolean
}

function PriorityBadge({ priority }: { priority: string }) {
  const normalizedPriority = priority.toLowerCase()
  const color =
    normalizedPriority === 'high' || normalizedPriority === 'highest'
      ? 'bg-danger'
      : normalizedPriority === 'medium'
        ? 'bg-warning'
        : 'bg-success'
  const labels: Record<string, string> = {
    highest: 'Наивысший',
    high: 'Высокий',
    medium: 'Средний',
    low: 'Низкий',
    lowest: 'Наинизший',
  }
  return (
    <span className="inline-flex items-center gap-1 text-xs font-medium text-text-primary">
      <span aria-hidden className={`h-2 w-2 rounded-full ${color}`} />
      {labels[normalizedPriority] ?? priority}
    </span>
  )
}

function IssueCard({
  issue,
  columnId,
  onDragStart,
  destinations,
  onMove,
  isMoving,
}: {
  issue: Issue
  columnId: string
  onDragStart: (issueId: string, columnId: string) => void
  destinations: Array<{ id: string; name: string }>
  onMove: (issueId: string, statusId: string) => void
  isMoving: boolean
}) {
  const { t } = useTranslation()
  function handleDragStart(e: React.DragEvent) {
    onDragStart(issue.id, columnId)
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/plain', issue.id)
  }

  return (
    <article
      draggable
      onDragStart={handleDragStart}
      className="cursor-grab rounded-md border border-border bg-surface-raised p-3 hover:border-border-strong active:cursor-grabbing"
    >
      <div className="flex items-start gap-2">
        <Link
          to={`/issues/${issue.id}`}
          className="min-w-0 flex-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          <div className="text-xs text-text-muted">{issue.key}</div>
          <div className="my-1 break-words text-sm font-medium">{issue.summary}</div>
        </Link>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="h-10 w-10 shrink-0"
              aria-label={`Изменить статус ${issue.key}`}
              disabled={isMoving}
            >
              <MoreHorizontal className="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-52">
            {destinations.length > 0 ? (
              destinations.map((status) => (
                <DropdownMenuItem key={status.id} onClick={() => onMove(issue.id, status.id)}>
                  {t('board.moveTo', { status: statusLabel(status.name, t) })}
                </DropdownMenuItem>
              ))
            ) : (
              <DropdownMenuItem disabled>{t('board.noTransitions')}</DropdownMenuItem>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <PriorityBadge priority={issue.priority} />
          <span className="rounded bg-border px-1.5 py-0.5 text-[10px] text-text-secondary">
            {t(`issueType.${issue.issue_type.toLowerCase()}`, { defaultValue: issue.issue_type })}
          </span>
        </div>
        <UserAvatar name={issue.assignee_name} userId={issue.assignee_id} />
      </div>
    </article>
  )
}

export function ProjectBoardPage() {
  const { t } = useTranslation()
  const { projectKey } = useParams<{ projectKey?: string }>()
  const [searchParams, setSearchParams] = useSearchParams()
  const key = projectKey ?? ''
  const { data: board, isLoading, error } = useBoard(key)
  const move = useMoveIssue(key)
  // Workflow transitions gate DnD client-side; the backend still enforces
  // them atomically, this only prevents obviously-invalid drops.
  const { data: transitions = [] } = useTransitions()
  const [drag, setDrag] = useState<DragState>({
    issueId: null,
    sourceColumnId: null,
    dragging: false,
  })
  const [dropTarget, setDropTarget] = useState<string | null>(null)

  if (isLoading) return <LoadingState message={t('issue.loading')} />
  if (error || !board) return <ErrorState message={error?.message ?? t('issue.notFound')} />

  const { columns, issues, sprint } = board
  const requestedColumn = searchParams.get('status')
  const activeColumnId = columns.some((column) => column.id === requestedColumn)
    ? requestedColumn!
    : columns[0]?.id

  function issuesByColumn(columnId: string) {
    return issues.filter((i) => columns.find((c) => c.id === columnId)?.issue_ids.includes(i.id))
  }

  function transitionAllowed(fromStatusId: string, toStatusId: string): boolean {
    if (fromStatusId === toStatusId) return false
    return transitions.some(
      (tr) => tr.from_status_id === fromStatusId && tr.to_status_id === toStatusId,
    )
  }

  function statusIdOfColumn(columnId: string): string | undefined {
    // Column ids ARE status ids on this board (see board DTO).
    return columnId
  }

  function handleDragStart(issueId: string, columnId: string) {
    setDrag({ issueId, sourceColumnId: columnId, dragging: true })
  }

  function handleDragOver(e: React.DragEvent, columnId: string) {
    e.preventDefault()
    if (columnId === drag.sourceColumnId) return
    setDropTarget(columnId)
  }

  function handleDrop(e: React.DragEvent, targetColumnId: string) {
    e.preventDefault()
    const issueId = e.dataTransfer.getData('text/plain') || drag.issueId
    const sourceColumnId = drag.sourceColumnId
    if (issueId && targetColumnId && targetColumnId !== sourceColumnId) {
      const issue = issues.find((i) => i.id === issueId)
      const fromStatusId = issue
        ? issue.status_id
        : statusIdOfColumn(sourceColumnId ?? targetColumnId)
      if (fromStatusId && transitionAllowed(fromStatusId, targetColumnId)) {
        moveIssue(issueId, targetColumnId)
      }
    }
    setDrag({ issueId: null, sourceColumnId: null, dragging: false })
    setDropTarget(null)
  }

  function handleDragLeave() {
    setDropTarget(null)
  }

  function moveIssue(issueId: string, statusId: string) {
    move.mutate(
      { issue_id: issueId, status_id: statusId },
      {
        onSuccess: () => toast.success(t('board.statusChanged')),
        onError: (moveError) => toast.error(moveError.message),
      },
    )
  }

  return (
    <div className="min-w-0">
      <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="min-w-0">
          <h1 className="truncate text-lg font-bold sm:text-xl">
            {t('board.title', { projectName: key, sprintName: sprint?.name ?? t('board.backlog') })}
          </h1>
          <div className="text-sm text-text-muted">
            {t('board.subtitle', {
              backlog: board.backlog_total,
              remainingDays: sprint?.remaining_days ?? t('board.notAvailable'),
            })}
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            className="gap-1"
            asChild
            aria-label={t('board.backlog')}
          >
            <Link to={`/projects/${key}/backlog`}>
              <List className="h-4 w-4" />
              <span className="hidden sm:inline">{t('board.backlog')}</span>
            </Link>
          </Button>
          <ProjectMembersPanel projectKey={key} />
        </div>
      </div>

      <div
        className="mb-3 grid grid-cols-2 gap-1 rounded-md border border-border bg-surface p-1 md:hidden"
        aria-label="Статус задач"
      >
        {columns.map((column) => (
          <button
            key={column.id}
            type="button"
            aria-pressed={activeColumnId === column.id}
            onClick={() => {
              const next = new URLSearchParams(searchParams)
              next.set('status', column.id)
              setSearchParams(next, { replace: true })
            }}
            className={`min-h-10 min-w-0 rounded px-2 text-sm font-medium ${
              activeColumnId === column.id
                ? 'bg-accent text-accent-foreground'
                : 'text-text-secondary hover:bg-surface-raised'
            }`}
          >
            <span className="block truncate">{statusLabel(column.name, t)}</span>
          </button>
        ))}
      </div>

      <div
        role="region"
        aria-label={t('board.columns')}
        tabIndex={0}
        className="grid min-w-0 grid-cols-1 gap-4 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus md:auto-cols-[minmax(16rem,22rem)] md:grid-flow-col md:grid-cols-none md:overflow-x-auto md:pb-2"
      >
        {columns.map((column) => {
          const wipLimit = column.wip_limit ?? null
          const colIssues = issuesByColumn(column.id)
          const overLimit = wipLimit !== null && colIssues.length >= wipLimit
          const isDropTarget = dropTarget === column.id && drag.dragging
          return (
            <div
              key={column.id}
              onDragOver={(e) => handleDragOver(e, column.id)}
              onDrop={(e) => handleDrop(e, column.id)}
              onDragLeave={handleDragLeave}
              className={`${activeColumnId === column.id ? 'flex' : 'hidden'} min-w-0 flex-col rounded-lg border bg-surface transition-colors md:flex ${
                isDropTarget ? 'border-accent ring-1 ring-accent' : 'border-border'
              }`}
            >
              <div className="flex items-center justify-between border-b border-border p-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">
                    {statusLabel(column.name, t)}
                  </div>
                  <div className="text-xs text-text-muted">
                    {colIssues.length} · {t('board.wip')}: {wipLimit ?? '—'}
                    {overLimit && <span className="ml-1 text-danger">{t('board.wipWarning')}</span>}
                  </div>
                </div>
              </div>

              <div className="min-h-24 flex-1 space-y-2 p-2">
                {colIssues.map((issue) => (
                  <IssueCard
                    key={issue.id}
                    issue={issue}
                    columnId={column.id}
                    onDragStart={handleDragStart}
                    destinations={columns.filter((candidate) =>
                      transitionAllowed(issue.status_id, candidate.id),
                    )}
                    onMove={moveIssue}
                    isMoving={move.isPending}
                  />
                ))}
                {colIssues.length === 0 && (
                  <p className="px-2 py-6 text-center text-sm text-text-muted">
                    {t('board.emptyStatus')}
                  </p>
                )}
              </div>

              <Link
                to="/issues/create"
                state={{ project_key: key }}
                className="m-2 flex min-h-10 items-center justify-center rounded-md border border-dashed border-border-strong px-2 text-center text-sm text-text-muted hover:border-text-muted hover:text-text-secondary"
              >
                + {t('board.create')}
              </Link>
            </div>
          )
        })}
      </div>
    </div>
  )
}
