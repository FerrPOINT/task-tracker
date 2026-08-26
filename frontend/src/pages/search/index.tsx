import { Link, useSearchParams } from 'react-router'
import { Search, X, ArrowUpDown, Filter } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useState, useMemo, useEffect } from 'react'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Card, CardContent } from '@/shared/ui/card'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/shared/ui/dropdown-menu'
import { useIssues, useProjects, useUsers } from '@/shared/api/hooks'
import type { Issue } from '@/api/search'

const SORT_OPTIONS = [
  { value: 'created_desc', labelKey: 'search.sortNewest' },
  { value: 'created_asc', labelKey: 'search.sortOldest' },
  { value: 'updated_desc', labelKey: 'search.sortUpdated' },
  { value: 'priority_desc', labelKey: 'search.sortPriority' },
]

const PRIORITY_OPTIONS = ['low', 'medium', 'high', 'urgent']
const STATUS_OPTIONS = ['todo', 'in_progress', 'review', 'done']

export default function SearchPage() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const [query, setQuery] = useState(() => searchParams.get('q') ?? '')
  const [jql, setJql] = useState(() => searchParams.get('jql') ?? '')

  const projectKey = searchParams.get('project_key') ?? undefined
  const status = searchParams.get('status') ?? undefined
  const assigneeId = searchParams.get('assignee_id') ?? undefined
  const priority = searchParams.get('priority') ?? undefined
  const sort = searchParams.get('sort') ?? 'created_desc'

  useEffect(() => {
    const timeout = setTimeout(() => {
      setSearchParams((prev) => {
        if (query) prev.set('q', query)
        else prev.delete('q')
        return prev
      })
    }, 250)
    return () => clearTimeout(timeout)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query])

  useEffect(() => {
    setSearchParams((prev) => {
      if (jql) prev.set('jql', jql)
      else prev.delete('jql')
      return prev
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jql])

  const filters = useMemo(
    () => ({
      q: query || undefined,
      project_key: projectKey,
      status,
      assignee_id: assigneeId,
      priority,
      sort_by: sort.split('_')[0],
      sort_order: sort.split('_')[1] ?? 'desc',
      jql: jql || undefined,
    }),
    [query, projectKey, status, assigneeId, priority, sort, jql],
  )

  const { data: issues, isLoading } = useIssues(filters)
  const { data: projects } = useProjects()
  const { data: users } = useUsers()

  const filtered = useMemo(() => issues ?? [], [issues])

  const setFilter = (key: string, value: string | undefined) => {
    setSearchParams((prev) => {
      if (value) prev.set(key, value)
      else prev.delete(key)
      return prev
    })
  }

  const clearFilters = () => {
    setQuery('')
    setJql('')
    setSearchParams(new URLSearchParams())
  }

  const hasFilters =
    projectKey || status || assigneeId || priority || sort !== 'created_desc' || query || jql

  const projectName =
    projects?.find((p) => p.key === projectKey)?.name ?? projectKey ?? t('search.project')
  const statusLabel = status ?? t('search.status')
  const priorityLabel = priority ? t(`priority.${priority}`) : t('search.priority')
  const assigneeName =
    users?.find((u) => u.id === assigneeId)?.display_name ?? assigneeId ?? t('search.assignee')
  const sortLabel = SORT_OPTIONS.find((o) => o.value === sort)
    ? t(SORT_OPTIONS.find((o) => o.value === sort)!.labelKey)
    : t('search.sort')

  return (
    <div className="container mx-auto px-4 py-6">
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-2xl font-semibold">{t('search.title')}</h1>
        <Button variant="outline" asChild>
          <Link to="/projects">{t('search.backToProjects')}</Link>
        </Button>
      </div>

      <Card className="mb-6">
        <CardContent className="flex flex-col gap-4 pt-6 md:flex-row md:items-end">
          <div className="relative flex-1">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              placeholder={t('search.placeholder')}
              className="pl-9"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" className="w-full md:w-44 justify-between">
                {projectName}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-56">
              <DropdownMenuItem onClick={() => setFilter('project_key', undefined)}>
                {t('search.allProjects')}
              </DropdownMenuItem>
              {projects?.map((p) => (
                <DropdownMenuItem key={p.key} onClick={() => setFilter('project_key', p.key)}>
                  {p.key} — {p.name}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" className="w-full md:w-40 justify-between">
                {statusLabel}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
              <DropdownMenuItem onClick={() => setFilter('status', undefined)}>
                {t('search.allStatuses')}
              </DropdownMenuItem>
              {STATUS_OPTIONS.map((s) => (
                <DropdownMenuItem key={s} onClick={() => setFilter('status', s)}>
                  {t(`status.${s}`)}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" className="w-full md:w-40 justify-between">
                {priorityLabel}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
              <DropdownMenuItem onClick={() => setFilter('priority', undefined)}>
                {t('search.allPriorities')}
              </DropdownMenuItem>
              {PRIORITY_OPTIONS.map((p) => (
                <DropdownMenuItem key={p} onClick={() => setFilter('priority', p)}>
                  {t(`priority.${p}`)}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" className="w-full md:w-44 justify-between">
                {assigneeName}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-56">
              <DropdownMenuItem onClick={() => setFilter('assignee_id', undefined)}>
                {t('search.allAssignees')}
              </DropdownMenuItem>
              {users?.map((u) => (
                <DropdownMenuItem key={u.id} onClick={() => setFilter('assignee_id', u.id)}>
                  {u.display_name}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" className="w-full md:w-44 justify-between">
                <ArrowUpDown className="mr-2 h-4 w-4" />
                {sortLabel}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
              {SORT_OPTIONS.map((o) => (
                <DropdownMenuItem key={o.value} onClick={() => setFilter('sort', o.value)}>
                  {t(o.labelKey)}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>

          {hasFilters && (
            <Button
              variant="ghost"
              size="icon"
              onClick={clearFilters}
              aria-label={t('search.clear')}
            >
              <X className="h-4 w-4" />
            </Button>
          )}
        </CardContent>
      </Card>

      {/* JQL Search Section */}
      <Card className="mb-6">
        <CardContent className="flex flex-col gap-3 pt-6">
          <div className="flex items-center gap-2">
            <Filter className="h-4 w-4 text-muted-foreground" />
            <span className="text-sm font-medium">{t('jql.title')}</span>
          </div>
          <div className="flex gap-2">
            <Input
              placeholder={t('jql.placeholder')}
              value={jql}
              onChange={(e) => setJql(e.target.value)}
              className="flex-1 font-mono text-sm"
            />
          </div>
          <p className="text-xs text-muted-foreground">{t('jql.help')}</p>
        </CardContent>
      </Card>

      {isLoading ? (
        <div className="space-y-3">
          {Array.from({ length: 5 }).map((_, i) => (
            <div key={i} className="h-16 w-full animate-pulse rounded-md bg-muted" />
          ))}
        </div>
      ) : filtered.length === 0 ? (
        <div className="py-12 text-center text-muted-foreground">{t('search.noResults')}</div>
      ) : (
        <div className="space-y-2">
          {filtered.map((issue: Issue) => (
            <SearchResultRow key={issue.id} issue={issue} />
          ))}
        </div>
      )}
    </div>
  )
}

function SearchResultRow({ issue }: { issue: Issue }) {
  return (
    <Card className="hover:bg-muted/50 transition-colors">
      <CardContent className="flex items-center justify-between py-3">
        <div className="flex items-center gap-3">
          <span className="rounded border px-2 py-0.5 text-xs font-medium">{issue.key}</span>
          <Link
            to={`/issues/${issue.id}`}
            className="font-medium hover:text-primary hover:underline"
          >
            {issue.summary}
          </Link>
        </div>
        <div className="flex items-center gap-3 text-sm text-muted-foreground">
          <span className="rounded bg-secondary px-2 py-0.5 text-xs">{issue.status}</span>
          <span>{issue.priority}</span>
          <span>{issue.assignee_name ?? 'Unassigned'}</span>
        </div>
      </CardContent>
    </Card>
  )
}
