import { useEffect, useState } from 'react'
import { Link } from 'react-router'
import { Plus, Filter, Users, MoreHorizontal } from 'lucide-react'
import { Button } from '@/shared/ui/button'
import { getBoard, type BoardColumn, type Issue, type Sprint } from '@/api/board'

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

export function ProjectBoardPage() {
  const [columns, setColumns] = useState<BoardColumn[]>([])
  const [issues, setIssues] = useState<Issue[]>([])
  const [sprint, setSprint] = useState<Sprint | null>(null)

  useEffect(() => {
    getBoard().then((data) => {
      setColumns(data.columns)
      setIssues(data.issues)
      setSprint(data.sprint)
    })
  }, [])

  function issuesByColumn(columnId: string) {
    return issues.filter((i) => columns.find((c) => c.id === columnId)?.issueIds.includes(i.id))
  }

  return (
    <div className="flex flex-col md:h-[calc(100vh-10rem)] md:max-h-[800px]">
      <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="min-w-0">
          <div className="truncate text-lg font-bold sm:text-xl">TT Kanban · {sprint?.name ?? 'Sprint'}</div>
          <div className="text-sm text-text-muted">Backlog 42 · Active · {sprint?.remainingDays ?? '-'} days left</div>
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
          const colIssues = issuesByColumn(column.id)
          const overLimit = column.wipLimit !== null && colIssues.length >= column.wipLimit
          return (
            <div
              key={column.id}
              className="flex min-w-[260px] flex-1 flex-col rounded-lg border border-border bg-surface"
            >
              <div className="flex items-center justify-between border-b border-border p-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">{column.name}</div>
                  <div className="text-xs text-text-muted">
                    {colIssues.length} · WIP: {column.wipLimit ?? '—'}
                    {overLimit && <span className="ml-1 text-amber-500">⚠️</span>}
                  </div>
                </div>
                <Button variant="ghost" size="icon" className="h-7 w-7 shrink-0">
                  <MoreHorizontal className="h-4 w-4" />
                </Button>
              </div>

              <div className="flex-1 space-y-2 overflow-y-auto p-2">
                {colIssues.map((issue) => (
                  <Link
                    key={issue.id}
                    to={`/issues/${issue.id}`}
                    className="block rounded-md border border-border bg-surface-raised p-3 hover:border-border-strong"
                  >
                    <div className="text-xs text-text-muted">{issue.key}</div>
                    <div className="my-1 text-sm font-medium">{issue.summary}</div>
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <PriorityBadge priority={issue.priority} />
                        <span className="rounded bg-border px-1.5 py-0.5 text-[10px] text-text-secondary">Task</span>
                      </div>
                      <Avatar name={issue.assigneeName} />
                    </div>
                  </Link>
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
          const colIssues = issuesByColumn(column.id)
          const overLimit = column.wipLimit !== null && colIssues.length >= column.wipLimit
          return (
            <div key={column.id} className="rounded-lg border border-border bg-surface">
              <div className="flex items-center justify-between border-b border-border p-3">
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold">{column.name}</div>
                  <div className="text-xs text-text-muted">
                    {colIssues.length} · WIP: {column.wipLimit ?? '—'}
                    {overLimit && <span className="ml-1 text-amber-500">⚠️</span>}
                  </div>
                </div>
                <Button variant="ghost" size="icon" className="h-7 w-7 shrink-0">
                  <MoreHorizontal className="h-4 w-4" />
                </Button>
              </div>

              <div className="space-y-2 p-2">
                {colIssues.map((issue) => (
                  <Link
                    key={issue.id}
                    to={`/issues/${issue.id}`}
                    className="block rounded-md border border-border bg-surface-raised p-3 hover:border-border-strong"
                  >
                    <div className="text-xs text-text-muted">{issue.key}</div>
                    <div className="my-1 text-sm font-medium">{issue.summary}</div>
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <PriorityBadge priority={issue.priority} />
                        <span className="rounded bg-border px-1.5 py-0.5 text-[10px] text-text-secondary">Task</span>
                      </div>
                      <Avatar name={issue.assigneeName} />
                    </div>
                  </Link>
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
