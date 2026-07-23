import { Link, useParams } from 'react-router'
import { Plus, Filter, Users, MoreHorizontal } from 'lucide-react'
import { Button } from '@/shared/ui/button'
import { useBoard, useMoveIssue } from '@/shared/api/hooks'
import type { components } from '@/api/generated'

export type Issue = components['schemas']['IssueResponse']

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
    <div className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-[10px] font-semibold text-white ${color}`}>
      {name.charAt(0).toUpperCase()}
    </div>
  )
}

function IssueCard({ issue, columnId, onMove }: { issue: Issue; columnId: string; onMove?: (issueId: string, targetColumnId: string) => void }) {
  function handleClick(e: React.MouseEvent) {
    // Move to next/previous column by wheel click or ctrl+click
    if (e.ctrlKey || e.button === 1) {
      e.preventDefault()
      onMove?.(issue.id, columnId)
    }
  }
  return (
    <Link
      key={issue.id}
      to={`/issues/${issue.id}`}
      onClick={handleClick}
      onAuxClick={handleClick}
      className="block rounded-md border border-border bg-surface-raised p-3 hover:border-border-strong"
    >
      <div className="text-xs text-text-muted">{issue.key}</div>
      <div className="my-1 text-sm font-medium">{issue.summary}</div>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <PriorityBadge priority={issue.priority} />
          <span className="rounded bg-border px-1.5 py-0.5 text-[10px] text-text-secondary">{issue.issue_type}</span>
        </div>
        <Avatar name={issue.assignee_name ?? '?'} />
      </div>
    </Link>
  )
}

export function ProjectBoardPage() {
  const { projectKey } = useParams<{ projectKey?: string }>()
  const key = projectKey ?? 'TT'
  const { data: board, isLoading, error } = useBoard(key)
  const move = useMoveIssue(key)

  if (isLoading) return <div className="p-4 text-text-muted">Loading board…</div>
  if (error || !board) return <div className="p-4 text-rose-500">{error?.message ?? 'Board not found'}</div>

  const { columns, issues, sprint } = board

  function issuesByColumn(columnId: string) {
    return issues.filter((i) => columns.find((c) => c.id === columnId)?.issue_ids.includes(i.id))
  }

  function handleMove(issueId: string, fromColumnId: string) {
    const fromIndex = columns.findIndex((c) => c.id === fromColumnId)
    const toIndex = fromIndex + 1
    if (toIndex >= columns.length) return
    const target = columns[toIndex]
    if (!target) return
    move.mutate({ issue_id: issueId, status_id: target.id })
  }

  return (
    <div className="flex flex-col md:h-[calc(100vh-10rem)] md:max-h-[800px]">
      <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="min-w-0">
          <div className="truncate text-lg font-bold sm:text-xl">{key} Kanban · {sprint?.name ?? 'Sprint'}</div>
          <div className="text-sm text-text-muted">Backlog 42 · Active · {sprint?.remaining_days ?? '-'} days left</div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="outline" size="sm" className="gap-1">
            <Filter className="h-4 w-4" />
            <span className="hidden sm:inline">Фильтры</span>
          </Button>
          <Button variant="outline" size="sm" className="gap-1">
            <Users className="h-4 w-4" />
            <span className="hidden sm:inline">Участники</span>
          </Button>
          <Button size="sm" className="gap-1">
            <Plus className="h-4 w-4" />
            <span className="hidden sm:inline">Колонка</span>
          </Button>
        </div>
      </div>

      {/* Desktop horizontal board */}
      <div className="hidden flex-1 gap-4 overflow-x-auto pb-2 md:flex">
        {columns.map((column) => {
          const wipLimit = column.wip_limit ?? null
          const colIssues = issuesByColumn(column.id)
          const overLimit = wipLimit !== null && colIssues.length >= wipLimit
          return (
            <div
              key={column.id}
              className="flex min-w-[260px] flex-1 flex-col rounded-lg border border-border bg-surface"
            >
              <div className="flex items-center justify-between border-b border-border p-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">{column.name}</div>
                  <div className="text-xs text-text-muted">
                    {colIssues.length} · WIP: {wipLimit ?? '—'}
                    {overLimit && <span className="ml-1 text-amber-500">⚠️</span>}
                  </div>
                </div>
                <Button variant="ghost" size="icon" className="h-7 w-7 shrink-0">
                  <MoreHorizontal className="h-4 w-4" />
                </Button>
              </div>

              <div className="flex-1 space-y-2 overflow-y-auto p-2">
                {colIssues.map((issue) => (
                  <IssueCard key={issue.id} issue={issue} columnId={column.id} onMove={handleMove} />
                ))}
              </div>

              <button className="m-2 rounded-md border border-dashed border-border-strong py-1.5 text-sm text-text-muted hover:border-text-muted hover:text-text-secondary">
                + Create
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
                    {colIssues.length} · WIP: {wipLimit ?? '—'}
                    {overLimit && <span className="ml-1 text-amber-500">⚠️</span>}
                  </div>
                </div>
                <Button variant="ghost" size="icon" className="h-7 w-7 shrink-0">
                  <MoreHorizontal className="h-4 w-4" />
                </Button>
              </div>

              <div className="space-y-2 p-2">
                {colIssues.map((issue) => (
                  <IssueCard key={issue.id} issue={issue} columnId={column.id} onMove={handleMove} />
                ))}
              </div>

              <button className="m-2 rounded-md border border-dashed border-border-strong py-1.5 text-sm text-text-muted hover:border-text-muted hover:text-text-secondary">
                + Create
              </button>
            </div>
          )
        })}
      </div>
    </div>
  )
}
