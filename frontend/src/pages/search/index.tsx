import { Link, useSearchParams } from 'react-router'
import { Search, X, ArrowUpDown, Filter, ChevronLeft, ChevronRight } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useState, useMemo, useEffect } from 'react'
import { Button } from '@sdlc/ui/ui'
import { Input } from '@sdlc/ui/ui'
import { Card, CardContent, ErrorState } from '@sdlc/ui/ui'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@sdlc/ui/ui'
import { useIssues, useProjects, useUsers } from '@/shared/api/hooks'
import { SearchRequestError, type Issue } from '@/api/search'

const PAGE_SIZE = 25

const SORT_OPTIONS = [
  { value: 'created_desc', labelKey: 'search.sortNewest' },
  { value: 'created_asc', labelKey: 'search.sortOldest' },
  { value: 'updated_desc', labelKey: 'search.sortUpdated' },
  { value: 'priority_desc', labelKey: 'search.sortPriority' },
]

// Values must match backend priority values exactly (Title-Case in DB).
const PRIORITY_OPTIONS = ['lowest', 'low', 'medium', 'high', 'highest']

// i18n keys stay lowercase; the API expects Title-Case values.
function titleCasePriority(p: string): string {
  return p.charAt(0).toUpperCase() + p.slice(1)
}
const STATUS_OPTIONS = ['todo', 'in_progress', 'review', 'done']

export default function SearchPage() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const query = searchParams.get('q') ?? ''
  const committedJql = searchParams.get('jql') ?? ''
  const [debouncedQuery, setDebouncedQuery] = useState(query.trim())
  const [jqlDraft, setJqlDraft] = useState(committedJql)
  const [showMobileFilters, setShowMobileFilters] = useState(false)
  const mode = searchParams.get('mode') === 'jql' || searchParams.has('jql') ? 'jql' : 'simple'
  const pageValue = Number(searchParams.get('page'))
  const page =
    Number.isSafeInteger(pageValue) &&
    pageValue > 0 &&
    Number.isSafeInteger((pageValue - 1) * PAGE_SIZE)
      ? pageValue
      : 1

  const projectKey = searchParams.get('project_key') ?? undefined
  const status = searchParams.get('status') ?? undefined
  const assigneeId = searchParams.get('assignee_id') ?? undefined
  const priority = searchParams.get('priority') ?? undefined
  const sort = searchParams.get('sort') ?? 'created_desc'

  useEffect(() => {
    const timeout = setTimeout(() => setDebouncedQuery(query.trim()), 350)
    return () => clearTimeout(timeout)
  }, [query])

  useEffect(() => {
    setJqlDraft(committedJql)
  }, [committedJql])

  const filters = useMemo(
    () => ({
      q: mode === 'simple' ? debouncedQuery || undefined : undefined,
      project_key: mode === 'simple' ? projectKey : undefined,
      status: mode === 'simple' ? status : undefined,
      assignee_id: mode === 'simple' ? assigneeId : undefined,
      priority: mode === 'simple' ? priority : undefined,
      sort_by: mode === 'simple' ? sort.split('_')[0] : undefined,
      sort_order: mode === 'simple' ? (sort.split('_')[1] ?? 'desc') : undefined,
      jql: mode === 'jql' ? committedJql || undefined : undefined,
      limit: PAGE_SIZE + 1,
      offset: (page - 1) * PAGE_SIZE,
    }),
    [debouncedQuery, projectKey, status, assigneeId, priority, sort, committedJql, mode, page],
  )

  const { data: issues, isLoading, error, refetch } = useIssues(filters)
  const { data: projects } = useProjects()
  const { data: users } = useUsers()

  const visibleIssues = issues?.slice(0, PAGE_SIZE) ?? []
  const hasNext = (issues?.length ?? 0) > PAGE_SIZE
  const invalidJql = mode === 'jql' && error instanceof SearchRequestError && error.status === 400

  const updateQuery = (value: string) => {
    setSearchParams(
      (previous) => {
        const next = new URLSearchParams(previous)
        if (value) next.set('q', value)
        else next.delete('q')
        next.delete('page')
        return next
      },
      { replace: true },
    )
  }

  const setFilter = (key: string, value: string | undefined) => {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev)
      if (value) next.set(key, value)
      else next.delete(key)
      next.delete('page')
      return next
    })
  }

  const clearFilters = () => {
    setDebouncedQuery('')
    setJqlDraft('')
    setSearchParams(new URLSearchParams())
  }

  const setMode = (nextMode: 'simple' | 'jql') => {
    if (nextMode === mode) return
    setDebouncedQuery('')
    setJqlDraft('')
    setSearchParams((previous) => {
      const next = new URLSearchParams(previous)
      next.set('mode', nextMode)
      next.delete('page')
      if (nextMode === 'jql') {
        for (const key of ['q', 'project_key', 'status', 'assignee_id', 'priority'])
          next.delete(key)
        next.delete('sort')
      } else {
        next.delete('jql')
      }
      return next
    })
  }

  const submitJql = (event: React.FormEvent) => {
    event.preventDefault()
    setSearchParams((previous) => {
      const next = new URLSearchParams(previous)
      next.set('mode', 'jql')
      if (jqlDraft.trim()) next.set('jql', jqlDraft.trim())
      else next.delete('jql')
      next.delete('page')
      return next
    })
  }

  const setPage = (nextPage: number) => {
    setSearchParams((previous) => {
      const next = new URLSearchParams(previous)
      if (nextPage <= 1) next.delete('page')
      else next.set('page', String(nextPage))
      return next
    })
  }

  const hasFilters =
    projectKey || status || assigneeId || priority || sort !== 'created_desc' || query
  const activeFilterCount =
    [projectKey, status, assigneeId, priority].filter(Boolean).length +
    Number(sort !== 'created_desc')

  const projectName =
    projects?.find((p) => p.key === projectKey)?.name ?? projectKey ?? t('search.project')
  const statusLabel = status ? t(`status.${status}`) : t('search.status')
  const priorityLabel = priority ? t(`priority.${priority.toLowerCase()}`) : t('search.priority')
  const assigneeName =
    users?.find((u) => u.id === assigneeId)?.display_name ?? assigneeId ?? t('search.assignee')
  const sortLabel = SORT_OPTIONS.find((o) => o.value === sort)
    ? t(SORT_OPTIONS.find((o) => o.value === sort)!.labelKey)
    : t('search.sort')

  return (
    <div className="py-6">
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-2xl font-semibold">{t('search.title')}</h1>
        <Button variant="outline" asChild className="h-10">
          <Link to="/projects">{t('search.backToProjects')}</Link>
        </Button>
      </div>

      <div className="mb-4 inline-grid grid-cols-2 rounded-md border border-border bg-surface p-1">
        <button
          type="button"
          onClick={() => setMode('simple')}
          aria-pressed={mode === 'simple'}
          className={`min-h-10 rounded px-4 text-sm ${mode === 'simple' ? 'bg-surface-raised font-medium text-text-primary' : 'text-text-muted'}`}
        >
          {t('search.modeSimple')}
        </button>
        <button
          type="button"
          onClick={() => setMode('jql')}
          aria-pressed={mode === 'jql'}
          className={`min-h-10 rounded px-4 text-sm ${mode === 'jql' ? 'bg-surface-raised font-medium text-text-primary' : 'text-text-muted'}`}
        >
          JQL
        </button>
      </div>

      {mode === 'simple' ? (
        <Card className="mb-6">
          <CardContent className="space-y-3 pt-6">
            <div className="relative min-w-0">
              <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-text-muted" />
              <Input
                aria-label={t('search.placeholder')}
                placeholder={t('search.placeholder')}
                className="min-h-10 pl-9"
                value={query}
                onChange={(e) => updateQuery(e.target.value)}
              />
            </div>
            <Button
              variant="outline"
              className="h-10 w-full justify-between sm:hidden"
              aria-expanded={showMobileFilters}
              aria-controls="search-filters"
              onClick={() => setShowMobileFilters((shown) => !shown)}
            >
              <span className="flex items-center gap-2">
                <Filter className="h-4 w-4" />
                {t('search.filters')}
              </span>
              {activeFilterCount > 0 && <span>{activeFilterCount}</span>}
            </Button>
            <div
              id="search-filters"
              className={`${showMobileFilters ? 'grid' : 'hidden'} gap-3 sm:grid sm:grid-cols-2 xl:grid-cols-6`}
            >
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline" className="h-10 w-full justify-between">
                    {projectName}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start" className="w-56">
                  <DropdownMenuItem
                    className="min-h-10"
                    onClick={() => setFilter('project_key', undefined)}
                  >
                    {t('search.allProjects')}
                  </DropdownMenuItem>
                  {projects?.map((p) => (
                    <DropdownMenuItem
                      className="min-h-10"
                      key={p.key}
                      onClick={() => setFilter('project_key', p.key)}
                    >
                      {p.key} — {p.name}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>

              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline" className="h-10 w-full justify-between">
                    {statusLabel}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start">
                  <DropdownMenuItem
                    className="min-h-10"
                    onClick={() => setFilter('status', undefined)}
                  >
                    {t('search.allStatuses')}
                  </DropdownMenuItem>
                  {STATUS_OPTIONS.map((s) => (
                    <DropdownMenuItem
                      className="min-h-10"
                      key={s}
                      onClick={() => setFilter('status', s)}
                    >
                      {t(`status.${s}`)}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>

              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline" className="h-10 w-full justify-between">
                    {priorityLabel}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start">
                  <DropdownMenuItem
                    className="min-h-10"
                    onClick={() => setFilter('priority', undefined)}
                  >
                    {t('search.allPriorities')}
                  </DropdownMenuItem>
                  {PRIORITY_OPTIONS.map((p) => (
                    <DropdownMenuItem
                      className="min-h-10"
                      key={p}
                      onClick={() => setFilter('priority', titleCasePriority(p))}
                    >
                      {t(`priority.${p}`)}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>

              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline" className="h-10 w-full justify-between">
                    {assigneeName}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start" className="w-56">
                  <DropdownMenuItem
                    className="min-h-10"
                    onClick={() => setFilter('assignee_id', undefined)}
                  >
                    {t('search.allAssignees')}
                  </DropdownMenuItem>
                  {users?.map((u) => (
                    <DropdownMenuItem
                      className="min-h-10"
                      key={u.id}
                      onClick={() => setFilter('assignee_id', u.id)}
                    >
                      {u.display_name}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>

              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline" className="h-10 w-full justify-between">
                    <ArrowUpDown className="mr-2 h-4 w-4" />
                    {sortLabel}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="start">
                  {SORT_OPTIONS.map((o) => (
                    <DropdownMenuItem
                      className="min-h-10"
                      key={o.value}
                      onClick={() => setFilter('sort', o.value)}
                    >
                      {t(o.labelKey)}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>

              {hasFilters && mode === 'simple' && (
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-10 w-full justify-start sm:w-10 sm:justify-center"
                  onClick={clearFilters}
                  aria-label={t('search.clear')}
                >
                  <X className="h-4 w-4" />
                  <span className="sm:hidden">{t('search.clear')}</span>
                </Button>
              )}
            </div>
          </CardContent>
        </Card>
      ) : null}

      {mode === 'jql' ? (
        <Card className="mb-6">
          <CardContent className="pt-6">
            <div className="flex items-center gap-2">
              <Filter className="h-4 w-4 text-text-muted" />
              <span className="text-sm font-medium">{t('jql.title')}</span>
            </div>
            <form onSubmit={submitJql} className="mt-3 flex flex-col gap-2 sm:flex-row">
              <Input
                aria-label={t('jql.placeholder')}
                placeholder={t('jql.placeholder')}
                value={jqlDraft}
                onChange={(e) => setJqlDraft(e.target.value)}
                aria-invalid={invalidJql}
                className="min-h-10 min-w-0 flex-1 font-mono text-sm"
              />
              <Button type="submit" className="h-10">
                {t('jql.execute')}
              </Button>
            </form>
            {invalidJql ? (
              <p role="alert" className="mt-2 text-sm text-destructive">
                {t('jql.invalid')}
              </p>
            ) : (
              <p className="mt-2 text-xs text-text-muted">{t('jql.help')}</p>
            )}
          </CardContent>
        </Card>
      ) : null}

      {isLoading ? (
        <div className="space-y-3">
          {Array.from({ length: 5 }).map((_, i) => (
            <div key={i} className="h-16 w-full animate-pulse rounded-md bg-surface-raised" />
          ))}
        </div>
      ) : error ? (
        invalidJql ? null : (
          <ErrorState message={t('search.loadError')} onRetry={() => void refetch()} />
        )
      ) : visibleIssues.length === 0 ? (
        <div className="py-12 text-center text-text-muted">{t('search.noResults')}</div>
      ) : (
        <ul className="divide-y divide-border rounded-md border border-border bg-surface">
          {visibleIssues.map((issue: Issue) => (
            <SearchResultRow key={issue.id} issue={issue} />
          ))}
        </ul>
      )}
      {!isLoading && !error && (page > 1 || hasNext) && (
        <nav
          aria-label={t('search.pages')}
          className="mt-5 flex items-center justify-between gap-3"
        >
          <Button
            variant="outline"
            className="h-10"
            disabled={page === 1}
            onClick={() => setPage(page - 1)}
          >
            <ChevronLeft className="h-4 w-4" />
            {t('search.previous')}
          </Button>
          <span className="text-sm text-text-muted">{t('search.page', { page })}</span>
          <Button
            variant="outline"
            className="h-10"
            disabled={!hasNext}
            onClick={() => setPage(page + 1)}
          >
            {t('search.next')}
            <ChevronRight className="h-4 w-4" />
          </Button>
        </nav>
      )}
    </div>
  )
}

function SearchResultRow({ issue }: { issue: Issue }) {
  const { t } = useTranslation()
  const normalizedStatus = issue.status.toLowerCase().replaceAll(' ', '_')
  const normalizedPriority = issue.priority.toLowerCase()
  return (
    <li className="flex flex-col gap-1 px-3 py-2 hover:bg-surface-raised sm:flex-row sm:items-center sm:justify-between sm:gap-4">
      <div className="flex min-w-0 flex-1 items-start gap-2 sm:items-center sm:gap-3">
        <span className="mt-2 shrink-0 rounded border px-2 py-0.5 text-xs font-medium sm:mt-0">
          {issue.key}
        </span>
        <Link
          to={`/issues/${issue.id}`}
          title={issue.summary}
          className="line-clamp-2 min-h-10 min-w-0 rounded-sm py-2 font-medium hover:text-accent hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus sm:line-clamp-1"
        >
          {issue.summary}
        </Link>
      </div>
      <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-sm text-text-muted sm:max-w-[45%] sm:flex-nowrap">
        <span className="shrink-0 rounded bg-surface-raised px-2 py-0.5 text-xs">
          {t(`status.${normalizedStatus}`, issue.status)}
        </span>
        <span className="shrink-0">{t(`priority.${normalizedPriority}`, issue.priority)}</span>
        <span className="min-w-0 truncate">{issue.assignee_name ?? t('issue.unassigned')}</span>
      </div>
    </li>
  )
}
