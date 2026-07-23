import { useEffect, useState } from 'react'
import { Link } from 'react-router'
import { Search, Save, Folder, CircleDot, User, Flag } from 'lucide-react'
import { Button } from '@/shared/ui/button'
import { searchIssues, type SearchResult } from '@/api/search'

function PriorityBadge({ priority }: { priority: string }) {
  const color =
    priority === 'High'
      ? 'text-rose-500'
      : priority === 'Medium'
        ? 'text-amber-500'
        : 'text-emerald-500'
  return <span className={`rounded-full bg-surface-raised px-2 py-0.5 text-xs font-medium ${color}`}>{priority}</span>
}

export function SearchPage() {
  const [query, setQuery] = useState('project = TT AND status != Done')
  const [result, setResult] = useState<SearchResult | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    setLoading(true)
    searchIssues(query)
      .then((data) => {
        setResult(data)
        setError(null)
      })
      .catch((e) => setError(e instanceof Error ? e.message : 'failed to search'))
      .finally(() => setLoading(false))
  }, [query])

  const results = result?.issues ?? []

  return (
    <div className="space-y-4">
      <h1 className="text-xl font-bold sm:text-2xl">Поиск задач</h1>

      <div className="rounded-lg border border-border bg-surface p-4">
        <textarea
          className="min-h-[72px] w-full rounded-md border border-border-strong bg-background p-3 font-mono text-sm text-text-primary"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="mt-3 flex flex-wrap gap-2">
          {[
            { icon: Folder, label: 'Task Tracker' },
            { icon: CircleDot, label: 'Status: open' },
            { icon: User, label: 'Assignee: Ivan' },
            { icon: Flag, label: 'Priority: Medium+' },
          ].map((chip, i) => (
            <button
              key={i}
              className="flex items-center gap-1 rounded-full border border-border-strong bg-surface-raised px-2.5 py-1 text-xs text-text-secondary hover:border-text-muted"
            >
              <chip.icon className="h-3 w-3" />
              {chip.label}
            </button>
          ))}
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          <Button size="sm" className="gap-1">
            <Search className="h-4 w-4" />
            Искать
          </Button>
          <Button variant="outline" size="sm" className="gap-1">
            <Save className="h-4 w-4" />
            <span className="hidden sm:inline">Сохранить фильтр</span>
            <span className="sm:hidden">Сохранить</span>
          </Button>
          <Button variant="outline" size="sm">Сбросить</Button>
        </div>
      </div>

      {loading && <div className="p-4 text-text-muted">Searching…</div>}
      {error && <div className="p-4 text-rose-500">{error}</div>}

      <div className="overflow-x-auto rounded-lg border border-border bg-surface">
        <div className="min-w-[560px]">
          <div className="grid grid-cols-[80px_1fr_120px_80px_80px] gap-3 border-b border-border px-4 py-2 text-xs font-semibold uppercase tracking-wider text-text-muted">
            <span>KEY</span>
            <span>SUMMARY</span>
            <span>STATUS</span>
            <span>ASSIGNEE</span>
            <span>PRIORITY</span>
          </div>
          {results.map((issue) => (
            <Link
              key={issue.id}
              to={`/issues/${issue.id}`}
              className="grid grid-cols-[80px_1fr_120px_80px_80px] items-center gap-3 border-b border-border px-4 py-3 text-sm hover:bg-surface-raised"
            >
              <span className="text-text-muted">{issue.key}</span>
              <span className="truncate">{issue.summary}</span>
              <span className="rounded-full bg-surface-raised px-2 py-0.5 text-xs text-text-secondary">{issue.status}</span>
              <span className="truncate">{issue.assignee_name ?? '—'}</span>
              <PriorityBadge priority={issue.priority} />
            </Link>
          ))}
        </div>
      </div>
    </div>
  )
}
