import { PriorityBadge } from '@/features/issues/ui/PriorityBadge'
import { CreateTaskAction } from '@/features/issues/ui/CreateTaskAction'
import {
  EmptyState,
  RadioGroup,
  TextLink,
  Card,
  CardContent,
  CardHeader,
  Badge,
  PageHeader,
  ProjectAvatar,
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  ErrorState,
  LoadingState,
  UserAvatar,
  toast,
} from '@pdlc/ui/ui'
import { ProjectLink as Link } from '@/shared/lib/project-navigation'
import { useParams, useSearchParams } from 'react-router'
import { List, MoreHorizontal } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useState } from 'react'

import { useBoard, useMoveIssue, useTransitions } from '@/shared/api/hooks'

import type { components } from '@/api/generated'

import { statusLabel } from '@/shared/lib/status-label'

export type Issue = components['schemas']['IssueResponse']

type DragState = {
  issueId: string | null
  sourceColumnId: string | null
  dragging: boolean
}

function IssueCard({
  issue,
  columnId,
  compact,
  onDragStart,
  destinations,
  onMove,
  isMoving,
}: {
  issue: Issue
  columnId: string
  compact: boolean
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
    <Card asChild surface="raised">
      <article draggable onDragStart={handleDragStart}>
        <CardContent density={compact ? 'compact' : 'default'}>
          <div className="flex items-start gap-2">
            <TextLink asChild className="min-w-0 flex-1">
              <Link to={`/issues/${issue.id}`}>
                <div className="text-xs text-text-muted">{issue.key}</div>
                <div
                  className={`${compact ? 'my-0.5' : 'my-0.5 md:my-1'} break-words text-sm font-medium`}
                >
                  {issue.summary}
                </div>
              </Link>
            </TextLink>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="shrink-0"
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
              <Badge tone="muted">
                {t(`issueType.${issue.issue_type.toLowerCase()}`, {
                  defaultValue: issue.issue_type,
                })}
              </Badge>
            </div>
            <UserAvatar name={issue.assignee_name} userId={issue.assignee_id} />
          </div>
        </CardContent>
      </article>
    </Card>
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
  const density = searchParams.get('density') === 'compact' ? 'compact' : 'comfortable'
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

  function setDensity(nextDensity: 'comfortable' | 'compact') {
    const next = new URLSearchParams(searchParams)
    if (nextDensity === 'compact') next.set('density', 'compact')
    else next.delete('density')
    setSearchParams(next, { replace: true })
  }

  return (
    <div className="min-w-0">
      <div className="mb-4">
        <PageHeader
          title={t('board.title', {
            projectName: key,
            sprintName: sprint?.name ?? t('board.backlog'),
          })}
          leading={<ProjectAvatar projectKey={key ?? ''} />}
          context={t('board.subtitle', {
            backlog: board.backlog_total,
            remainingDays: sprint?.remaining_days ?? t('board.notAvailable'),
          })}
          actions={
            <>
              <div className="hidden md:block">
                <RadioGroup
                  appearance="segmented"
                  label={t('board.density')}
                  value={density}
                  onValueChange={(value) => setDensity(value as 'comfortable' | 'compact')}
                  options={(['comfortable', 'compact'] as const).map((value) => ({
                    value,
                    label: t(`board.density${value === 'compact' ? 'Compact' : 'Comfortable'}`),
                  }))}
                />
              </div>
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
            </>
          }
        />
      </div>

      <div className="mb-3 md:hidden">
        <RadioGroup
          appearance="segmented"
          className="w-full"
          label="Статус задач"
          value={activeColumnId}
          onValueChange={(value) => {
            const next = new URLSearchParams(searchParams)
            next.set('status', value)
            setSearchParams(next, { replace: true })
          }}
          options={columns.map((column) => ({
            value: column.id,
            label: statusLabel(column.name, t),
          }))}
        />
      </div>

      <div
        role="region"
        aria-label={t('board.columns')}
        tabIndex={0}
        className={`grid min-w-0 grid-cols-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus md:grid-cols-2 xl:grid-cols-4 ${
          density === 'compact' ? 'gap-2' : 'gap-2 md:gap-4'
        }`}
      >
        {columns.map((column) => {
          const wipLimit = column.wip_limit ?? null
          const colIssues = issuesByColumn(column.id)
          const overLimit = wipLimit !== null && colIssues.length >= wipLimit
          const isDropTarget = dropTarget === column.id && drag.dragging
          return (
            <Card
              key={column.id}
              highlighted={isDropTarget}
              onDragOver={(e) => handleDragOver(e, column.id)}
              onDrop={(e) => handleDrop(e, column.id)}
              onDragLeave={handleDragLeave}
              className={`${activeColumnId === column.id ? 'flex' : 'hidden'} min-w-0 flex-col md:flex`}
            >
              <CardHeader density={density === 'compact' ? 'compact' : 'default'}>
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">
                    {statusLabel(column.name, t)}
                  </div>
                  <div className="text-xs text-text-muted">
                    {colIssues.length} · {t('board.wip')}: {wipLimit ?? '—'}
                    {overLimit && (
                      <Badge appearance="inline" tone="danger" className="ml-1">
                        {t('board.wipWarning')}
                      </Badge>
                    )}
                  </div>
                </div>
              </CardHeader>

              <CardContent
                density="compact"
                className={`min-h-24 flex-1 ${
                  density === 'compact' ? 'space-y-1' : 'space-y-1 md:space-y-2'
                }`}
              >
                {colIssues.map((issue) => (
                  <IssueCard
                    key={issue.id}
                    issue={issue}
                    columnId={column.id}
                    compact={density === 'compact'}
                    onDragStart={handleDragStart}
                    destinations={columns.filter((candidate) =>
                      transitionAllowed(issue.status_id, candidate.id),
                    )}
                    onMove={moveIssue}
                    isMoving={move.isPending}
                  />
                ))}
                {colIssues.length === 0 && (
                  <EmptyState message={t('board.emptyStatus')} />
                )}
              </CardContent>

              <CreateTaskAction
                projectKey={key}
                aria-label={t('board.create')}
                variant="dashed"
                className={density === 'compact' ? 'm-1.5' : 'm-1.5 md:m-2'}
              >
                {t('board.create')}
              </CreateTaskAction>
            </Card>
          )
        })}
      </div>
    </div>
  )
}
