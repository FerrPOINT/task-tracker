import { useState } from 'react'
import { Link, useParams } from 'react-router'
import {
  Plus,
  MoreHorizontal,
  GripVertical,
  Play,
  CheckCircle2,
  Pencil,
  ArrowRightLeft,
  X,
} from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/shared/ui/button'
import { ErrorState, LoadingState } from '@/shared/ui/async-states'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  DropdownMenuSeparator,
} from '@/shared/ui/dropdown-menu'
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
import { UserAvatar } from '@/shared/ui/user-avatar'
import type { components } from '@/api/generated'
import type { Sprint, CreateSprintRequest, UpdateSprintRequest } from '@/api/sprint'

type Issue = components['schemas']['IssueResponse']

function PriorityBadge({ priority }: { priority: string }) {
  const color =
    priority === 'High'
      ? 'text-rose-500'
      : priority === 'Medium'
        ? 'text-amber-500'
        : 'text-emerald-500'
  return <span className={`text-xs font-medium ${color}`}>{priority}</span>
}

function IssueRow({ issue, action }: { issue: Issue; action?: React.ReactNode }) {
  return (
    <div className="group flex items-center gap-2 border-b border-border px-3 py-2.5 text-sm hover:bg-surface-raised sm:grid sm:grid-cols-[24px_80px_1fr_90px_40px_40px] sm:gap-3">
      <GripVertical className="h-4 w-4 shrink-0 text-text-muted sm:order-1" />
      <Link to={`/issues/${issue.id}`} className="contents">
        <span className="shrink-0 text-text-muted sm:order-2">{issue.key}</span>
        <span className="min-w-0 flex-1 truncate font-medium sm:order-3">{issue.summary}</span>
      </Link>
      <div className="ml-auto flex shrink-0 items-center gap-2 sm:order-4 sm:ml-0">
        <PriorityBadge priority={issue.priority} />
        <UserAvatar name={issue.assignee_name} userId={issue.assignee_id} />
      </div>
      <div className="sm:order-5" />
      <div className="flex justify-end sm:order-6">{action}</div>
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
          className="h-7 w-7 opacity-0 group-hover:opacity-100 focus:opacity-100 data-[state=open]:opacity-100"
          aria-label={t('backlog.issueActions')}
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
            <DropdownMenuItem disabled className="text-text-muted">
              <ArrowRightLeft className="mr-2 h-4 w-4" />
              {t('backlog.moveToSprint')}
            </DropdownMenuItem>
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

function Section<T extends Issue>({
  title,
  action,
  items,
  renderItem,
  emptyText,
}: {
  title: string
  action?: React.ReactNode
  items: T[]
  renderItem: (item: T) => React.ReactNode
  emptyText?: string
}) {
  return (
    <div className="mb-5">
      <div className="flex flex-col gap-2 rounded-t-lg border border-border bg-surface px-3 py-2.5 sm:flex-row sm:items-center sm:justify-between">
        <span className="font-semibold">{title}</span>
        {action}
      </div>
      <div className="rounded-b-lg border-x border-b border-border bg-surface">
        {items.length === 0 && emptyText && (
          <div className="px-3 py-6 text-center text-sm text-text-muted">{emptyText}</div>
        )}
        {items.map(renderItem)}
      </div>
    </div>
  )
}

const BACKLOG_PAGE_SIZE = 100

export function ProjectBacklogPage() {
  const { t } = useTranslation()
  const { projectKey } = useParams<{ projectKey?: string }>()
  const key = projectKey ?? 'TT'
  const [backlogOffset, setBacklogOffset] = useState(0)
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

  const isLoading = backlogLoading || sprintsLoading
  const error = backlogError

  if (isLoading) return <LoadingState message={t('issue.loading')} />
  if (error || !backlog) return <ErrorState message={error?.message ?? t('issue.notFound')} />

  const { sprint: activeSprint, sprint_issues, backlog_issues } = backlog
  const backlogTotal = backlog.backlog_total ?? backlog_issues.length
  const currentOffset = backlog.backlog_offset ?? backlogOffset
  const pageSize = backlog.backlog_limit ?? BACKLOG_PAGE_SIZE
  const hasPrev = currentOffset > 0
  const hasNext = currentOffset + backlog_issues.length < backlogTotal
  const futureSprints =
    sprints?.filter((s) => s.id !== activeSprint.id && s.state !== 'closed') ?? []
  const activeFromList = sprints?.find((s) => s.id === activeSprint.id)
  const activeSprintName = activeFromList?.name ?? activeSprint.name

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

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="min-w-0">
          <h1 className="text-xl font-bold sm:text-2xl">
            {t('backlog.title', { projectName: key })}
          </h1>
          <div className="text-sm text-text-muted">
            {t('backlog.velocity', { velocity: activeSprint.velocity ?? '-' })} ·{' '}
            {t('backlog.backlogCount', { count: backlogTotal })}
            {hasNext &&
              ` · ${t('backlog.windowed', {
                from: currentOffset + 1,
                to: currentOffset + backlog_issues.length,
                total: backlogTotal,
              })}`}
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button size="sm" className="gap-1" onClick={openCreate}>
            <Plus className="h-4 w-4" />
            {t('backlog.createSprint')}
          </Button>
          <Button variant="outline" size="sm" className="gap-1" asChild>
            <Link
              to={`/issues/create?project_key=${encodeURIComponent(key)}`}
              state={{ project_key: key }}
            >
              <Plus className="h-4 w-4" />
              <span className="hidden sm:inline">{t('backlog.createIssue')}</span>
              <span className="sm:hidden">{t('backlog.createIssue')}</span>
            </Link>
          </Button>
        </div>
      </div>

      <Section
        title={t('backlog.activeSprint', {
          name: activeSprintName,
          velocity: activeSprint.velocity,
          remaining: activeSprint.remaining_days ?? '-',
        })}
        action={
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              className="h-7 px-2.5 text-xs"
              onClick={() => startSprint.mutate(activeSprint.id)}
              disabled={activeFromList?.state === 'active'}
            >
              <Play className="mr-1 h-3 w-3" />
              {t('backlog.startSprint')}
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="h-7 px-2.5 text-xs"
              onClick={() => closeSprint.mutate(activeSprint.id)}
              disabled={activeFromList?.state !== 'active'}
            >
              <CheckCircle2 className="mr-1 h-3 w-3" />
              {t('backlog.closeSprint')}
            </Button>
            {activeFromList && (
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7"
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
            )}
          </div>
        }
        items={sprint_issues}
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
        <Section
          key={sprint.id}
          title={t('backlog.futureSprint', { name: sprint.name })}
          action={
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                className="h-7 px-2.5 text-xs"
                onClick={() => startSprint.mutate(sprint.id)}
              >
                <Play className="mr-1 h-3 w-3" />
                {t('backlog.startSprint')}
              </Button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7"
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

      <Section
        title={t('backlog.backlogSection', { count: backlog_issues.length })}
        action={
          <Button variant="outline" size="sm" className="h-7 px-2.5 text-xs" asChild>
            <Link to="/issues/create">
              <Plus className="h-4 w-4" />
              {t('navigation.create')}
            </Link>
          </Button>
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
        <div className="flex items-center justify-between rounded-lg border border-border bg-surface px-3 py-2.5">
          <Button
            variant="outline"
            size="sm"
            disabled={!hasPrev || backlogLoading}
            onClick={() => setBacklogOffset(Math.max(0, currentOffset - pageSize))}
          >
            {t('backlog.prevPage')}
          </Button>
          <span className="text-sm text-text-muted">
            {t('backlog.pageInfo', {
              from: currentOffset + 1,
              to: currentOffset + backlog_issues.length,
              total: backlogTotal,
            })}
          </span>
          <Button
            variant="outline"
            size="sm"
            disabled={!hasNext || backlogLoading}
            onClick={() => setBacklogOffset(currentOffset + pageSize)}
          >
            {t('backlog.nextPage')}
          </Button>
        </div>
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
