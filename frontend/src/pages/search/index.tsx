import { PROJECT_CHOICE_PAGE_SIZE, useProjectChoiceCatalog } from '@/widgets/project-choice-catalog'
import { useDebouncedValue } from '@pdlc/ui/lib'
import {
  EmptyState,
  LoadingState,
  RadioGroup,
  TextLink,
  PageNavigation,
  Badge,
  PageHeader,
  ResourceList,
  ResourceListRow,
  Button,
  Input,
  Card,
  CardContent,
  ErrorState,
  SearchInput,
  SearchableSelect,
  Select,
  Notice,
} from '@pdlc/ui/ui'
import { ProjectLink as Link } from '@/shared/lib/project-navigation'
import { useSearchParams } from 'react-router'
import { X, Filter } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useState, useMemo, useEffect } from 'react'

import { useIssues, useUsers } from '@/shared/api/hooks'
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
  const [projectQuery, setProjectQuery] = useState({ q: '', offset: 0 })
  const [assigneeQuery, setAssigneeQuery] = useState({ q: '', offset: 0 })
  const projectSearch = useDebouncedValue(projectQuery.q, 200)
  const projectChoices = useProjectChoiceCatalog(
    projectKey ?? '',
    projectQuery.offset,
    projectSearch,
  )
  const users = useUsers()
  const userLabel = (user: NonNullable<typeof users.data>[number]) =>
    user.username && user.username !== user.display_name
      ? `${user.display_name} · ${user.username}`
      : user.display_name
  const matchingUsers = (users.isError ? [] : (users.data ?? [])).filter((user) =>
    `${user.display_name} ${user.username}`
      .toLowerCase()
      .includes(assigneeQuery.q.trim().toLowerCase()),
  )
  const selectedUser = users.isError
    ? undefined
    : users.data?.find((user) => user.id === assigneeId)

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
    setSearchParams((previous) => {
      const next = new URLSearchParams(previous)
      for (const key of [
        'q',
        'mode',
        'jql',
        'project_key',
        'status',
        'assignee_id',
        'priority',
        'sort',
        'page',
      ])
        next.delete(key)
      return next
    })
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

  return (
    <div className="py-6">
      <div className="mb-6">
        <PageHeader
          title={t('search.title')}
          actions={
            <Button variant="outline" asChild>
              <Link to="/projects">{t('search.backToProjects')}</Link>
            </Button>
          }
        />
      </div>

      <RadioGroup
        appearance="segmented"
        className="mb-4"
        label={t('search.mode')}
        value={mode}
        onValueChange={(value) => setMode(value as 'simple' | 'jql')}
        options={[
          { value: 'simple', label: t('search.modeSimple') },
          { value: 'jql', label: 'JQL' },
        ]}
      />

      {mode === 'simple' ? (
        <Card className="mb-6">
          <CardContent className="space-y-3">
            <div className="min-w-0">
              <SearchInput
                aria-label={t('search.placeholder')}
                placeholder={t('search.placeholder')}
                value={query}
                onChange={(e) => updateQuery(e.target.value)}
              />
            </div>
            <Button
              variant="outline"
              className="w-full justify-between sm:hidden"
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
              <div className="min-w-0">
                <SearchableSelect
                  id="search-project"
                  label={t('search.project')}
                  value={projectKey ?? ''}
                  selectedLabel={
                    projectChoices.selectedPending
                      ? t('search.checkingProject')
                      : projectChoices.selected
                        ? `${projectChoices.selected.name} · ${projectChoices.selected.key}`
                        : undefined
                  }
                  placeholder={t('search.allProjects')}
                  options={(projectChoices.data ?? []).map((project) => ({
                    value: project.key,
                    label: `${project.name} · ${project.key}`,
                  }))}
                  search={projectQuery.q}
                  onSearch={
                    projectChoices.searchable
                      ? (q) =>
                          setProjectQuery({ q: Array.from(q).slice(0, 200).join(''), offset: 0 })
                      : undefined
                  }
                  offset={projectQuery.offset}
                  pageSize={PROJECT_CHOICE_PAGE_SIZE}
                  total={projectChoices.catalog?.total}
                  hasNext={projectChoices.hasMore}
                  onPage={(offset) => setProjectQuery((previous) => ({ ...previous, offset }))}
                  loading={projectChoices.isPending || projectQuery.q !== projectSearch}
                  error={projectChoices.isError ? t('search.projectsError') : undefined}
                  onRetry={() => void projectChoices.refetch()}
                  aria-invalid={projectChoices.selectedUnavailable || undefined}
                  aria-describedby={
                    projectChoices.selectedUnavailable ? 'search-project-error' : undefined
                  }
                  onChange={(value) => setFilter('project_key', value || undefined)}
                />
                {projectChoices.selectedUnavailable && (
                  <div id="search-project-error">
                    <ErrorState
                      scope="local"
                      message={t('search.projectUnavailable')}
                      onRetry={() => void projectChoices.retrySelected()}
                    />
                  </div>
                )}
              </div>
              <Select
                aria-label={t('search.status')}
                value={status ?? ''}
                options={[
                  { value: '', label: t('search.allStatuses') },
                  ...STATUS_OPTIONS.map((value) => ({ value, label: t(`status.${value}`) })),
                ]}
                onValueChange={(value) => setFilter('status', value || undefined)}
              />
              <Select
                aria-label={t('search.priority')}
                value={priority ?? ''}
                options={[
                  { value: '', label: t('search.allPriorities') },
                  ...PRIORITY_OPTIONS.map((value) => ({
                    value: titleCasePriority(value),
                    label: t(`priority.${value}`),
                  })),
                ]}
                onValueChange={(value) => setFilter('priority', value || undefined)}
              />
              <SearchableSelect
                id="search-assignee"
                label={t('search.assignee')}
                value={assigneeId ?? ''}
                selectedLabel={selectedUser ? userLabel(selectedUser) : undefined}
                placeholder={t('search.allAssignees')}
                options={matchingUsers
                  .slice(assigneeQuery.offset, assigneeQuery.offset + 20)
                  .map((user) => ({ value: user.id, label: userLabel(user) }))}
                search={assigneeQuery.q}
                onSearch={(q) => setAssigneeQuery({ q, offset: 0 })}
                offset={assigneeQuery.offset}
                pageSize={20}
                total={matchingUsers.length}
                onPage={(offset) => setAssigneeQuery((previous) => ({ ...previous, offset }))}
                loading={users.isPending}
                error={users.isError ? t('search.usersError') : undefined}
                onRetry={() => void users.refetch()}
                onChange={(value) => setFilter('assignee_id', value || undefined)}
              />
              <Select
                aria-label={t('search.sort')}
                value={sort}
                options={SORT_OPTIONS.map((option) => ({
                  value: option.value,
                  label: t(option.labelKey),
                }))}
                onValueChange={(value) => setFilter('sort', value)}
              />

              {hasFilters && mode === 'simple' && (
                <Button
                  variant="ghost"
                  size="icon"
                  className="w-full justify-start sm:justify-center"
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
          <CardContent>
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
                className="min-w-0 flex-1 font-mono text-sm"
              />
              <Button type="submit">{t('jql.execute')}</Button>
            </form>
            {invalidJql ? (
              <Notice role="alert" className="mt-2" tone="danger" appearance="inline">
                {t('jql.invalid')}
              </Notice>
            ) : (
              <p className="mt-2 text-xs text-text-muted">{t('jql.help')}</p>
            )}
          </CardContent>
        </Card>
      ) : null}

      {isLoading ? (
        <LoadingState scope="local" message={t('common.loading')} />
      ) : error ? (
        invalidJql ? null : (
          <ErrorState message={t('search.loadError')} onRetry={() => void refetch()} />
        )
      ) : visibleIssues.length === 0 ? (
        <EmptyState message={t('search.noResults')} />
      ) : (
        <ResourceList asChild variant="bordered">
          <ul>
            {visibleIssues.map((issue: Issue) => (
              <SearchResultRow key={issue.id} issue={issue} />
            ))}
          </ul>
        </ResourceList>
      )}
      {!isLoading && !error && (page > 1 || hasNext) && (
        <PageNavigation
          className="mt-5"
          page={page}
          hasNext={hasNext}
          onPageChange={setPage}
          label={t('search.pages')}
          previousLabel={t('search.previous')}
          nextLabel={t('search.next')}
          summary={t('search.page', { page })}
        />
      )}
    </div>
  )
}

function SearchResultRow({ issue }: { issue: Issue }) {
  const { t } = useTranslation()
  const normalizedStatus = issue.status.toLowerCase().replaceAll(' ', '_')
  const normalizedPriority = issue.priority.toLowerCase()
  return (
    <ResourceListRow asChild>
      <li className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between sm:gap-4">
        <div className="flex min-w-0 flex-1 items-start gap-2 sm:items-center sm:gap-3">
          <Badge className="mt-2 shrink-0 sm:mt-0">{issue.key}</Badge>
          <TextLink asChild>
            <Link
              to={`/issues/${issue.id}`}
              title={issue.summary}
              className="line-clamp-2 min-w-0 font-medium sm:line-clamp-1"
            >
              {issue.summary}
            </Link>
          </TextLink>
        </div>
        <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-sm text-text-muted sm:max-w-[45%] sm:flex-nowrap">
          <Badge tone="muted" className="shrink-0">
            {t(`status.${normalizedStatus}`, issue.status)}
          </Badge>
          <span className="shrink-0">{t(`priority.${normalizedPriority}`, issue.priority)}</span>
          <span className="min-w-0 truncate">{issue.assignee_name ?? t('issue.unassigned')}</span>
        </div>
      </li>
    </ResourceListRow>
  )
}
