import { Link, useParams } from 'react-router'
import { Plus, Filter, MoreHorizontal, List } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useState } from 'react'
import { Button } from '@/shared/ui/button'
import { ErrorState } from '@/shared/ui/async-states'
import { useBoard, useMoveIssue } from '@/shared/api/hooks'
import { ProjectMembersPanel } from '@/features/project-members/ui/ProjectMembersPanel'
import type { components } from '@/api/generated'

export type Issue = components['schemas']['IssueResponse']

type DragState = {
  issueId: string | null
  sourceColumnId: string | null
  dragging: boolean
}

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

function IssueCard({
  issue,
  columnId,
  onDragStart,
}: {
  issue: Issue
  columnId: string
  onDragStart: (issueId: string, columnId: string) => void
}) {
  function handleDragStart(e: React.DragEvent) {
    onDragStart(issue.id, columnId)
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/plain', issue.id)
  }

  return (
    <Link
      key={issue.id}
      to={`/issues/${issue.id}`}
      draggable
      onDragStart={handleDragStart}
      className="block cursor-grab rounded-md border border-border bg-surface-raised p-3 hover:border-border-strong active:cursor-grabbing"
    >
      <div className="text-xs text-text-muted">{issue.key}</div>
      <div className="my-1 text-sm font-medium">{issue.summary}</div>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <PriorityBadge priority={issue.priority} />
          <span className="rounded bg-border px-1.5 py-0.5 text-[10px] text-text-secondary">
            {issue.issue_type}
          </span>
        </div>
        <Avatar name={issue.assignee_name ?? '?'} />
      </div>
    </Link>
  )
}

export function ProjectBoardPage() {
  const { t } = useTranslation()
  const { projectKey } = useParams<{ projectKey?: string }>()
  const key = projectKey ?? 'TT'
  const { data: board, isLoading, error } = useBoard(key)
  const move = useMoveIssue(key)
  const [drag, setDrag] = useState<DragState>({
    issueId: null,
    sourceColumnId: null,
    dragging: false,
  })
  const [dropTarget, setDropTarget] = useState<string | null>(null)

  if (isLoading) return <div className="p-4 text-text-muted">{t('issue.loading')}</div>
  if (error || !board)
    return <ErrorState message={error?.message ?? t('issue.notFound')} />

  const { columns, issues, sprint } = board

  function issuesByColumn(columnId: string) {
    return issues.filter((i) => columns.find((c) => c.id === columnId)?.issue_ids.includes(i.id))
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
    if (issueId && targetColumnId && targetColumnId !== drag.sourceColumnId) {
      move.mutate({ issue_id: issueId, status_id: targetColumnId })
    }
    setDrag({ issueId: null, sourceColumnId: null, dragging: false })
    setDropTarget(null)
  }

  function handleDragLeave() {
    setDropTarget(null)
  }

  return (
    <div className="flex flex-col md:h-[calc(100vh-10rem)] md:max-h-[800px]">
      <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="min-w-0">
          <div className="truncate text-lg font-bold sm:text-xl">
            {t('board.title', { projectName: key, sprintName: sprint?.name ?? 'Sprint' })}
          </div>
          <div className="text-sm text-text-muted">
            {t('board.subtitle', { backlog: 42, remainingDays: sprint?.remaining_days ?? '-' })}
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="outline" size="sm" className="gap-1">
            <Filter className="h-4 w-4" />
            <span className="hidden sm:inline">{t('board.filters')}</span>
          </Button>
          <Button variant="outline" size="sm" className="gap-1" asChild>
            <Link to={`/projects/${key}/backlog`}>
              <List className="h-4 w-4" />
              <span className="hidden sm:inline">{t('board.backlog')}</span>
            </Link>
          </Button>
          {board?.project_id && <ProjectMembersPanel projectId={board.project_id} />}
          <Button size="sm" className="gap-1">
            <Plus className="h-4 w-4" />
            <span className="hidden sm:inline">{t('board.addColumn')}</span>
          </Button>
        </div>
      </div>

      {/* Desktop horizontal board */}
      <div className="hidden flex-1 gap-4 overflow-x-auto pb-2 md:flex">
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
              className={`flex min-w-[260px] flex-1 flex-col rounded-lg border bg-surface transition-colors ${
                isDropTarget ? 'border-accent ring-1 ring-accent' : 'border-border'
              }`}
            >
              <div className="flex items-center justify-between border-b border-border p-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">{column.name}</div>
                  <div className="text-xs text-text-muted">
                    {colIssues.length} · {t('board.wip')}: {wipLimit ?? '—'}
                    {overLimit && (
                      <span className="ml-1 text-amber-500">{t('board.wipWarning')}</span>
                    )}
                  </div>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7 shrink-0"
                  aria-label={t('board.columnActions')}
                >
                  <MoreHorizontal className="h-4 w-4" />
                </Button>
              </div>

              <div className="flex-1 space-y-2 overflow-y-auto p-2">
                {colIssues.map((issue) => (
                  <IssueCard
                    key={issue.id}
                    issue={issue}
                    columnId={column.id}
                    onDragStart={handleDragStart}
                  />
                ))}
              </div>

              <button className="m-2 rounded-md border border-dashed border-border-strong py-1.5 text-sm text-text-muted hover:border-text-muted hover:text-text-secondary">
                + {t('board.create')}
              </button>
            </div>
          )
        })}
      </div>

      {/* Mobile stacked board */}
      <div className="flex flex-1 flex-col gap-4 overflow-y-auto pb-2 md:hidden">
        {columns.map((column) => {
          const wipLimit = column.wip_limit ?? null
          const colIssues = issuesByColumn(column.id)
          const overLimit = wipLimit !== null && colIssues.length >= wipLimit
          return (
            <div key={column.id} className="rounded-lg border border-border bg-surface">
              <div className="flex items-center justify-between border-b border-border p-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">{column.name}</div>
                  <div className="text-xs text-text-muted">
                    {colIssues.length} · {t('board.wip')}: {wipLimit ?? '—'}
                    {overLimit && (
                      <span className="ml-1 text-amber-500">{t('board.wipWarning')}</span>
                    )}
                  </div>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7 shrink-0"
                  aria-label={t('board.columnActions')}
                >
                  <MoreHorizontal className="h-4 w-4" />
                </Button>
              </div>

              <div className="space-y-2 p-2">
                {colIssues.map((issue) => (
                  <IssueCard
                    key={issue.id}
                    issue={issue}
                    columnId={column.id}
                    onDragStart={handleDragStart}
                  />
                ))}
              </div>

              <button className="m-2 rounded-md border border-dashed border-border-strong py-1.5 text-sm text-text-muted hover:border-text-muted hover:text-text-secondary">
                + {t('board.create')}
              </button>
            </div>
          )
        })}
      </div>
    </div>
  )
}
