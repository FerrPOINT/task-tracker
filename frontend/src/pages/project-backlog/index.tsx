import { PriorityBadge } from '@/features/issues/ui/PriorityBadge'
import { CreateTaskAction } from '@/features/issues/ui/CreateTaskAction'
import { ResourceListDescription, ResourceListTitle,
  ProjectAvatar,
  PageHeader,
  PageNavigation,
  CardContent,
  Card,
  CardHeader,
  SectionHeader,
  ResourceList,
  ResourceListRow,
  EmptyState,
  Button,
  ErrorState,
  LoadingState,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
  DropdownMenuSeparator,
  UserAvatar,
} from '@pdlc/ui/ui'
import { useEffect, useState } from 'react'
import { ProjectLink as Link } from '@/shared/lib/project-navigation'
import { useParams, useSearchParams } from 'react-router'
import { Plus, MoreHorizontal, Play, CheckCircle2, Pencil, ArrowRightLeft, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import {
  useBacklog,
  useSprints,
  useCreateSprint,
  useUpdateSprint,
  useStartSprint,
  useCloseSprint,
  useMoveIssueToSprint,
  useRemoveIssueFromSprint,
} from '@/shared/api/hooks'
import { SprintFormDialog } from '@/features/sprints/ui/SprintFormDialog'

import type { components } from '@/api/generated'
import type { Sprint, CreateSprintRequest, UpdateSprintRequest } from '@/api/sprint'

type Issue = components['schemas']['IssueResponse']

function IssueRow({ issue, action }: { issue: Issue; action?: React.ReactNode }) {
  return (
    <div className="group flex min-w-0 items-center gap-2">
      <ResourceListRow asChild interactive>
        <Link
          to={`/issues/${issue.id}`}
          title={issue.summary}
          className="flex min-w-0 flex-1 flex-col gap-1 sm:flex-row sm:items-center sm:gap-3"
        >
          <span className="flex min-w-0 flex-1 flex-col gap-1 sm:flex-row sm:items-center sm:gap-3">
            <ResourceListDescription asChild className="shrink-0 sm:w-20"><span >{issue.key}</span></ResourceListDescription>
            <ResourceListTitle asChild className="line-clamp-2 min-w-0 sm:line-clamp-1 sm:flex-1"><span >
              {issue.summary}
            </span></ResourceListTitle>
          </span>
          <span className="flex items-center gap-2 sm:shrink-0">
            <PriorityBadge priority={issue.priority} />
            <UserAvatar name={issue.assignee_name} userId={issue.assignee_id} />
          </span>
        </Link>
      </ResourceListRow>
      <div className="mr-3 shrink-0">{action}</div>
    </div>
  )
}

function MoveIssueAction({
  issue,
  projectKey,
  activeSprint,
  futureSprints,
  removeFromSprintId,
}: {
  issue: Issue
  projectKey: string
  activeSprint?: Sprint | null
  futureSprints: Sprint[]
  removeFromSprintId?: string
}) {
  const { t } = useTranslation()
  const moveTo = useMoveIssueToSprint(projectKey)
  const removeFrom = useRemoveIssueFromSprint(projectKey)
  const isLoading = moveTo.isPending || removeFrom.isPending

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon"

          aria-label={t('backlog.issueActions', { key: issue.key })}
        >
          <MoreHorizontal className="h-4 w-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {removeFromSprintId && (
          <DropdownMenuItem
            disabled={isLoading}
            onClick={() => removeFrom.mutate({ sprintId: removeFromSprintId, issueId: issue.id })}
          >
            <X className="mr-2 h-4 w-4" />
            {t('backlog.removeFromSprint')}
          </DropdownMenuItem>
        )}
        {(activeSprint || futureSprints.length > 0) && (
          <>
            {removeFromSprintId && <DropdownMenuSeparator />}
            <DropdownMenuLabel className="flex items-center gap-2">
              <ArrowRightLeft className="h-4 w-4" />
              {t('backlog.moveToSprint')}
            </DropdownMenuLabel>
            {activeSprint && issue.sprint_id !== activeSprint.id && (
              <DropdownMenuItem
                disabled={isLoading}
                onClick={() => moveTo.mutate({ sprintId: activeSprint.id, issueId: issue.id })}
              >
                {activeSprint.name}
              </DropdownMenuItem>
            )}
            {futureSprints.map(
              (s) =>
                s.id !== issue.sprint_id && (
                  <DropdownMenuItem
                    key={s.id}
                    disabled={isLoading}
                    onClick={() => moveTo.mutate({ sprintId: s.id, issueId: issue.id })}
                  >
                    {s.name}
                  </DropdownMenuItem>
                ),
            )}
          </>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function BacklogSection({
  title,
  action,
  items,
  renderItem,
  emptyText,
}: {
  title: string
  action?: React.ReactNode
  items: Issue[]
  renderItem: (item: Issue) => React.ReactNode
  emptyText?: string
}) {
  return (
    <Card asChild className="mb-5">
      <section aria-label={title}>
        <CardHeader>
          <SectionHeader title={title} actions={action} />
        </CardHeader>
        <ResourceList>
          {items.length === 0 && emptyText && <EmptyState message={emptyText} />}
          {items.map(renderItem)}
        </ResourceList>
      </section>
    </Card>
  )
}

const BACKLOG_PAGE_SIZE = 100

function parseBacklogOffset(value: string | null) {
  if (value === null) return 0

  const offset = Number(value)
  return Number.isSafeInteger(offset) && offset >= 0 ? offset : 0
}

export function ProjectBacklogPage() {
  const { t } = useTranslation()
  const { projectKey } = useParams<{ projectKey?: string }>()
  const key = projectKey ?? ''
  const [searchParams, setSearchParams] = useSearchParams()
  const offsetParam = searchParams.get('offset')
  const backlogOffset = parseBacklogOffset(offsetParam)
  const {
    data: backlog,
    isLoading: backlogLoading,
    error: backlogError,
  } = useBacklog(key, backlogOffset, BACKLOG_PAGE_SIZE)
  const { data: sprints, isLoading: sprintsLoading } = useSprints(key)
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editingSprint, setEditingSprint] = useState<Sprint | null>(null)

  const createSprint = useCreateSprint(key)
  const updateSprint = useUpdateSprint(key, editingSprint?.id ?? '')
  const startSprint = useStartSprint(key)
  const closeSprint = useCloseSprint(key)

  useEffect(() => {
    if (offsetParam === null || offsetParam === String(backlogOffset)) return

    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current)
        if (backlogOffset > 0) next.set('offset', String(backlogOffset))
        else next.delete('offset')
        return next
      },
      { replace: true },
    )
  }, [backlogOffset, offsetParam, setSearchParams])

  const isLoading = backlogLoading || sprintsLoading
  const error = backlogError

  if (isLoading) return <LoadingState message={t('issue.loading')} />
  if (error || !backlog) return <ErrorState message={error?.message ?? t('issue.notFound')} />

  const { sprint: activeSprint, sprint_issues, backlog_issues } = backlog
  const hasActiveSprint = activeSprint.id !== 'none'
  const backlogTotal = backlog.backlog_total ?? backlog_issues.length
  const currentOffset = backlog.backlog_offset ?? backlogOffset
  const pageSize = backlog.backlog_limit ?? BACKLOG_PAGE_SIZE
  const hasPrev = currentOffset > 0
  const hasNext = currentOffset + backlog_issues.length < backlogTotal
  const futureSprints =
    sprints?.filter((s) => s.id !== activeSprint.id && s.state !== 'closed') ?? []
  const activeFromList = hasActiveSprint
    ? sprints?.find((s) => s.id === activeSprint.id)
    : undefined
  const activeSprintName = activeFromList?.name ?? activeSprint.name
  const activeSprintIssues = activeFromList
    ? sprint_issues.filter((issue) => issue.sprint_id === activeFromList.id)
    : []

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

  function updateBacklogOffset(nextOffset: number) {
    const next = new URLSearchParams(searchParams)
    if (nextOffset > 0) next.set('offset', String(nextOffset))
    else next.delete('offset')
    setSearchParams(next)
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('backlog.title', { projectName: key })}
        leading={<ProjectAvatar projectKey={key ?? ''} />}
        context={
          <>
            {t('backlog.velocity', { velocity: activeSprint.velocity ?? '-' })} ·{' '}
            {t('backlog.backlogCount', { count: backlogTotal })}
            {hasNext &&
              ` · ${t('backlog.windowed', {
                from: currentOffset + 1,
                to: currentOffset + backlog_issues.length,
                total: backlogTotal,
              })}`}
          </>
        }
        actions={
          <>
            <Button size="sm" onClick={openCreate}>
              <Plus className="h-4 w-4" />
              {t('backlog.createSprint')}
            </Button>
            <CreateTaskAction
              projectKey={key}
              aria-label={t('backlog.createIssue')}
              variant="outline"
              size="sm"
            >
              <span>{t('backlog.createIssue')}</span>
            </CreateTaskAction>
          </>
        }
      />

      <BacklogSection
        title={
          hasActiveSprint
            ? t('backlog.activeSprint', {
                name: activeSprintName,
                velocity: activeSprint.velocity,
                remaining: activeSprint.remaining_days ?? t('backlog.notAvailable'),
              })
            : t('backlog.noActiveSprint')
        }
        action={
          activeFromList ? (
            <div className="flex items-center gap-2">
              <Button
                size="sm"

                onClick={() => startSprint.mutate(activeSprint.id)}
                disabled={activeFromList?.state === 'active'}
              >
                <Play className="h-3 w-3" />
                {t('backlog.startSprint')}
              </Button>
              <Button
                size="sm"
                variant="outline"

                onClick={() => closeSprint.mutate(activeSprint.id)}
                disabled={activeFromList?.state !== 'active'}
              >
                <CheckCircle2 className="h-3 w-3" />
                {t('backlog.closeSprint')}
              </Button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"

                    aria-label={t('common.edit')}
                  >
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
            </div>
          ) : undefined
        }
        items={activeSprintIssues}
        renderItem={(issue) => (
          <IssueRow
            key={issue.id}
            issue={issue}
            action={
              <MoveIssueAction
                issue={issue}
                projectKey={key}
                activeSprint={activeFromList}
                futureSprints={futureSprints}
                removeFromSprintId={activeSprint.id !== 'none' ? activeSprint.id : undefined}
              />
            }
          />
        )}
        emptyText={t('backlog.emptySprint')}
      />

      {futureSprints.map((sprint) => (
        <BacklogSection
          key={sprint.id}
          title={t('backlog.futureSprint', { name: sprint.name })}
          action={
            <div className="flex items-center gap-2">
              <Button
                size="sm"

                onClick={() => startSprint.mutate(sprint.id)}
              >
                <Play className="h-3 w-3" />
                {t('backlog.startSprint')}
              </Button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"

                    aria-label={t('common.edit')}
                  >
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
          items={sprint_issues.filter((issue) => issue.sprint_id === sprint.id)}
          renderItem={(issue) => (
            <IssueRow
              key={issue.id}
              issue={issue}
              action={
                <MoveIssueAction
                  issue={issue}
                  projectKey={key}
                  activeSprint={activeFromList}
                  futureSprints={futureSprints}
                  removeFromSprintId={sprint.id}
                />
              }
            />
          )}
          emptyText={t('backlog.emptySprint')}
        />
      ))}

      <BacklogSection
        title={t('backlog.backlogSection', { count: backlog_issues.length })}
        action={
          <CreateTaskAction projectKey={key} variant="outline" size="sm">
            {t('navigation.create')}
          </CreateTaskAction>
        }
        items={backlog_issues}
        renderItem={(issue) => (
          <IssueRow
            key={issue.id}
            issue={issue}
            action={
              <MoveIssueAction
                issue={issue}
                projectKey={key}
                activeSprint={activeFromList}
                futureSprints={futureSprints}
              />
            }
          />
        )}
        emptyText={t('backlog.emptyBacklog')}
      />

      {(hasPrev || hasNext) && (
        <Card>
          <CardContent>
            <PageNavigation
              page={Math.ceil(currentOffset / pageSize) + 1}
              hasNext={hasNext}
              pending={backlogLoading}
              label={t('backlog.title', { projectName: key })}
              previousLabel={t('backlog.prevPage')}
              nextLabel={t('backlog.nextPage')}
              onPageChange={(page) =>
                updateBacklogOffset(
                  page < Math.ceil(currentOffset / pageSize) + 1
                    ? Math.max(0, currentOffset - pageSize)
                    : currentOffset + pageSize,
                )
              }
              summary={t('backlog.pageInfo', {
                from: currentOffset + 1,
                to: currentOffset + backlog_issues.length,
                total: backlogTotal,
              })}
            />
          </CardContent>
        </Card>
      )}

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
