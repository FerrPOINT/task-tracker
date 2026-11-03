import { ContentSection } from '@pdlc/ui/ui'
import { useClipboard, useMediaQuery } from '@pdlc/ui/lib'
import {
  ActionGroup,
  Badge,
  TextLink,
  ProjectAvatar,
  toast,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  SectionHeader,
  Disclosure,
  Button,
  ConfirmDialog,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  LoadingState,
  ErrorState,
} from '@pdlc/ui/ui'
import { ProjectLink as Link } from '@/shared/lib/project-navigation'
import { useParams, useSearchParams } from 'react-router'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Copy, UserPlus, MoreHorizontal } from 'lucide-react'

import {
  useWorklogs,
  useCreateWorklog,
  useUpdateWorklog,
  useDeleteWorklog,
} from '@/features/time-tracking/model/use-worklogs'
import { TimeTrackingPanel } from '@/features/time-tracking/ui/TimeTrackingPanel'
import { WorklogTab } from '@/features/time-tracking/ui/WorklogTab'
import { LogWorkDialog } from '@/features/time-tracking/ui/LogWorkDialog'
import { CommentsPanel } from '@/features/comments/ui/CommentList'
import { useComments } from '@/features/comments/model/use-comments'
import { ActivityFeed } from '@/features/issue-detail/ui/ActivityFeed'
import { AttachmentPanel } from '@/features/issue-detail/ui/AttachmentPanel'
import { LabelEditor } from '@/features/issue-detail/ui/LabelEditor'
import { LinkEditor } from '@/features/issue-detail/ui/LinkEditor'
import { CustomFieldsPanel } from '@/features/issue-detail/ui/CustomFieldsPanel'
import { IssueEngagementPanel } from '@/features/issue-detail/ui/IssueEngagementPanel'
import type { Worklog, LogWorkInput } from '@/entities/worklog/model'
import { useAuthStore } from '@/shared/auth/store'
import { statusLabel } from '@/shared/lib/status-label'
import { IssueMetaEditor } from '@/features/issue-detail/ui/IssueMetaEditor'
import { IssueDescriptionEditor } from '@/features/issue-detail/ui/IssueDescriptionEditor'
import {
  useBoard,
  useUpdateIssue,
  useDeleteIssue,
  useSprints,
  useIssue,
  useStatuses,
} from '@/shared/api/hooks'
import { TaskDocumentsPanel } from '@/features/issue-detail/ui/TaskDocumentsPanel'
import { TaskRepositoriesPanel } from '@/features/issue-detail/ui/TaskRepositoriesPanel'
import { SdlcPanel } from '@/features/sdlc/ui/SdlcPanel'

const issueTabs = [
  'activity',
  'comments',
  'worklog',
  'attachments',
  'decomposition',
  'sdlc',
] as const
type IssueTab = (typeof issueTabs)[number]

function parseIssueTab(value: string | null, hasDecomposition = false): IssueTab {
  return (
    issueTabs.find((tab) => tab === value && (tab !== 'decomposition' || hasDecomposition)) ??
    'activity'
  )
}

export function IssueDetailPage({ decompositionPanel }: { decompositionPanel?: ReactNode } = {}) {
  const { id = '' } = useParams()
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const tabParam = searchParams.get('tab')
  const activeTab = parseIssueTab(tabParam, Boolean(decompositionPanel))
  const latestTab = useRef(activeTab)
  const currentUserId = useAuthStore((s) => s.userId)
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editingWorklog, setEditingWorklog] = useState<Worklog | undefined>(undefined)
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false)
  const issueActionsRef = useRef<HTMLButtonElement | null>(null)
  const worklogTabRef = useRef<HTMLButtonElement | null>(null)
  const tabsViewportRef = useRef<HTMLDivElement | null>(null)

  const issueQuery = useIssue(id)
  const issueClipboard = useClipboard(issueQuery.data?.key ?? '', {
    onCopied: () => toast.success(t('issue.keyCopied')),
    onCopyError: () => toast.error(t('issue.copyFailed')),
  })
  const boardQuery = useBoard(issueQuery.data?.project_key)
  const statusesQuery = useStatuses()
  const sprintsQuery = useSprints(issueQuery.data?.project_key)
  const updateIssue = useUpdateIssue(id)
  const deleteIssueMutation = useDeleteIssue()
  // Historical issue URLs may contain a key; owner APIs use the loaded UUID.
  const taskId = issueQuery.data?.id ?? ''
  const worklogsQuery = useWorklogs(taskId)
  const commentsQuery = useComments(taskId)
  const create = useCreateWorklog(taskId)
  const update = useUpdateWorklog(taskId)
  const remove = useDeleteWorklog(taskId)

  useEffect(() => {
    latestTab.current = activeTab
  }, [activeTab])

  useEffect(() => {
    const revealSelected = () => {
      const viewport = tabsViewportRef.current
      const selected = viewport?.querySelector<HTMLElement>('[role="tab"][data-state="active"]')
      if (!viewport || !selected) return
      const bounds = viewport.getBoundingClientRect()
      const tab = selected.getBoundingClientRect()
      if (tab.left < bounds.left) viewport.scrollLeft -= bounds.left - tab.left
      else if (tab.right > bounds.right) viewport.scrollLeft += tab.right - bounds.right
    }
    revealSelected()
    window.addEventListener('resize', revealSelected)
    return () => window.removeEventListener('resize', revealSelected)
  }, [activeTab, issueQuery.data?.id])

  useEffect(() => {
    const isCanonical = tabParam === null || (activeTab !== 'activity' && tabParam === activeTab)
    if (isCanonical) return

    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current)
        next.delete('tab')
        return next
      },
      { replace: true },
    )
  }, [activeTab, setSearchParams, tabParam])

  if (issueQuery.isLoading) {
    return <LoadingState message={t('issue.loading')} />
  }

  if (issueQuery.error && !issueQuery.data) {
    return <ErrorState message={t('issue.loadError')} onRetry={() => void issueQuery.refetch()} />
  }

  if (!issueQuery.data) {
    return <ErrorState message={t('issue.notFound')} />
  }

  const issue = issueQuery.data
  const worklogs = worklogsQuery.data ?? []
  const activityLoading = commentsQuery.isLoading || worklogsQuery.isLoading
  const activityError = commentsQuery.error || worklogsQuery.error
  const hasActivityData = Boolean(commentsQuery.data?.length || worklogs.length)

  const handleLogWork = () => {
    setEditingWorklog(undefined)
    setDialogOpen(true)
  }

  const handleEdit = (worklog: Worklog) => {
    setEditingWorklog(worklog)
    setDialogOpen(true)
  }

  const handleSubmit = (input: LogWorkInput) => {
    if (editingWorklog) {
      update.mutate(
        { id: editingWorklog.id, input },
        {
          onSuccess: () => {
            setDialogOpen(false)
            toast.success(t('common.saved'))
          },
          onError: (error) => toast.error(error.message),
        },
      )
    } else {
      create.mutate(input, {
        onSuccess: () => {
          setDialogOpen(false)
          toast.success(t('common.saved'))
        },
        onError: (error) => toast.error(error.message),
      })
    }
  }

  const handleDelete = (worklogId: string) => remove.mutateAsync(worklogId)

  const updateActiveTab = (value: string) => {
    const nextTab = parseIssueTab(value, Boolean(decompositionPanel))
    if (nextTab === latestTab.current) return
    latestTab.current = nextTab
    const next = new URLSearchParams(searchParams)
    if (nextTab === 'activity') next.delete('tab')
    else next.set('tab', nextTab)
    setSearchParams(next)
  }

  return (
    <div className="min-w-0 bg-background">
      <div>
        {issueQuery.error && (
          <ErrorState message={t('issue.refreshError')} onRetry={() => void issueQuery.refetch()} />
        )}
        <div className="mb-4 flex flex-wrap items-center justify-between gap-x-3 gap-y-2">
          <div className="flex min-w-0 max-w-full flex-wrap items-center gap-x-2 gap-y-1">
            <div className="min-w-0 max-w-full break-words text-sm text-text-muted">
              <TextLink asChild className="inline-flex max-w-full items-center gap-2">
                <Link to={`/projects/${issue.project_key}/board`}>
                  <ProjectAvatar projectKey={issue.project_key} size="xs" />
                  {issue.project_name}
                </Link>
              </TextLink>{' '}
              / {issue.key}
            </div>
            <Badge tone="info" className="shrink-0">
              {t(`issueType.${issue.issue_type.toLowerCase()}`, { defaultValue: issue.issue_type })}
            </Badge>
          </div>
          <ActionGroup align="start" className="ml-auto">
            <Button
              variant="secondary"
              size="sm"

              disabled={
                updateIssue.isPending || !currentUserId || currentUserId === issue.assignee_id
              }
              onClick={() =>
                currentUserId &&
                updateIssue.mutate(
                  { assignee_id: currentUserId },
                  { onError: () => toast.error(t('issue.assignError')) },
                )
              }
            >
              <UserPlus className="h-4 w-4" />
              {t('issue.assignToMe')}
            </Button>
            <DropdownMenu modal={false}>
              <DropdownMenuTrigger asChild>
                <Button
                  ref={issueActionsRef}
                  variant="secondary"
                  size="icon"

                  aria-label={t('issue.actions')}
                >
                  <MoreHorizontal className="h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem
                  disabled={issueClipboard.pending}
                  onClick={() => void issueClipboard.copy()}
                  className="gap-2"
                >
                  <Copy className="h-4 w-4" />
                  {t('issue.copyKey')}
                </DropdownMenuItem>
                <DropdownMenuItem
                  tone="danger"
                  onClick={() => {
                    deleteIssueMutation.reset()
                    setDeleteConfirmOpen(true)
                  }}
                >
                  {t('issue.delete')}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </ActionGroup>
        </div>

        <IssueDetailLayout
          detailsSummary={`${t('issue.details')} · ${statusLabel(statusesQuery.data?.find((status) => status.id === issue.status_id)?.name ?? issue.status, t)}`}
          description={
            <ContentSection asChild dividers="none">
              <section className="min-w-0 lg:col-start-1 lg:row-start-1">
                <IssueDescriptionEditor
                  issue={issue}
                  disabled={updateIssue.isPending}
                  onSubmit={async (patch) => {
                    await updateIssue.mutateAsync(patch)
                    toast.success(t('common.saved'))
                  }}
                />
              </section>
            </ContentSection>
          }
          details={
            <ContentSection asChild dividers="none">
              <section className="min-w-0" aria-label={t('issue.details')}>
                <IssueMetaEditor
                  issue={issue}
                  columns={boardQuery.data?.columns ?? []}
                  sprints={sprintsQuery.data ?? []}
                  disabled={updateIssue.isPending}
                  onChange={(patch) =>
                    updateIssue.mutate(patch, { onError: (error) => toast.error(error.message) })
                  }
                />
              </section>
            </ContentSection>
          }
          activity={
            <div className="min-w-0 space-y-4 lg:col-start-1 lg:row-start-2">
              <Tabs value={activeTab} onValueChange={updateActiveTab}>
                <div ref={tabsViewportRef} className="min-w-0 overflow-x-auto">
                  <TabsList
                    appearance="underline"
                    style={{
                      flexWrap: 'nowrap',
                      width: 'max-content',
                      maxWidth: 'none',
                    }}
                  >
                    <TabsTrigger value="activity">{t('issue.activity')}</TabsTrigger>
                    <TabsTrigger value="comments">{t('issue.comments')}</TabsTrigger>
                    <TabsTrigger ref={worklogTabRef} value="worklog">
                      {t('timeTracking.worklog.title')}
                    </TabsTrigger>
                    <TabsTrigger value="attachments">{t('attachments.title')}</TabsTrigger>
                    {decompositionPanel && (
                      <TabsTrigger value="decomposition">Декомпозиция</TabsTrigger>
                    )}
                    <TabsTrigger value="sdlc">{t('sdlc.title')}</TabsTrigger>
                  </TabsList>
                </div>
                <TabsContent value="activity">
                  {commentsQuery.error && (
                    <ErrorState
                      message={t('comments.loadError')}
                      onRetry={() => void commentsQuery.refetch()}
                    />
                  )}
                  {worklogsQuery.error && (
                    <ErrorState
                      message={t('timeTracking.worklog.loadError')}
                      onRetry={() => void worklogsQuery.refetch()}
                    />
                  )}
                  {activityLoading ? (
                    <LoadingState scope="local" message={t('issue.loadingActivity')} />
                  ) : hasActivityData || !activityError ? (
                    <ActivityFeed comments={commentsQuery.data ?? []} worklogs={worklogs} />
                  ) : null}
                </TabsContent>
                <TabsContent value="comments">
                  <CommentsPanel issueId={issue.id} currentUserId={currentUserId ?? undefined} />
                </TabsContent>
                <TabsContent value="worklog">
                  {worklogsQuery.error ? (
                    <ErrorState
                      message={t('timeTracking.worklog.loadError')}
                      onRetry={() => void worklogsQuery.refetch()}
                    />
                  ) : worklogsQuery.isLoading ? (
                    <LoadingState scope="local" message={t('common.loading')} />
                  ) : (
                    <WorklogTab
                      worklogs={worklogs}
                      onEdit={handleEdit}
                      onDelete={handleDelete}
                      currentUserId={currentUserId ?? ''}
                      fallbackFocusRef={worklogTabRef}
                    />
                  )}
                </TabsContent>
                <TabsContent value="attachments">
                  <AttachmentPanel issueId={issue.id} />
                </TabsContent>
                {decompositionPanel && (
                  <TabsContent
                    key={issue.id}
                    value="decomposition"
                    forceMount
                    hidden={activeTab !== 'decomposition'}
                  >
                    {decompositionPanel}
                  </TabsContent>
                )}
                <TabsContent value="sdlc">
                  <SdlcPanel key={issue.id} issueId={issue.id} />
                </TabsContent>
              </Tabs>

              <div className="grid items-start gap-x-4 gap-y-2 md:grid-cols-2">
                <ContentSection density="compact">
                  <LabelEditor issueId={issue.id} projectKey={issue.project_key} />
                </ContentSection>
                <ContentSection density="compact">
                  <SectionHeader level={3} title={t('customFields.title')} />
                  <CustomFieldsPanel issueId={issue.id} projectKey={issue.project_key} />
                </ContentSection>
                <ContentSection density="compact" className="md:col-span-2">
                  <LinkEditor issueId={issue.id} currentKey={issue.key} />
                  {import.meta.env.VITE_NAMESPACE_ENABLED === 'true' && (
                    <TaskDocumentsPanel taskId={issue.id} projectKey={issue.project_key} />
                  )}
                  {import.meta.env.VITE_NAMESPACE_ENABLED === 'true' && (
                    <TaskRepositoriesPanel taskId={issue.id} projectKey={issue.project_key} />
                  )}
                </ContentSection>
              </div>
            </div>
          }
          supplemental={
            <>
              <ContentSection density="compact" className="space-y-2">
                <SectionHeader level={3} title={t('timeTracking.title')} />
                <TimeTrackingPanel
                  timeSpentSeconds={issue.time_spent_seconds}
                  originalEstimateSeconds={issue.original_estimate_seconds ?? null}
                  remainingEstimateSeconds={issue.remaining_estimate_seconds ?? null}
                  onLogWork={handleLogWork}
                />
              </ContentSection>

              <ContentSection density="compact" className="space-y-2">
                <SectionHeader level={3} title={t('engagement.title')} />
                <IssueEngagementPanel
                  issueId={issue.id}
                  projectKey={issue.project_key}
                  currentUserId={currentUserId}
                  reporterId={issue.reporter_id}
                />
              </ContentSection>
            </>
          }
        />
      </div>

      <LogWorkDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        onSubmit={handleSubmit}
        worklog={editingWorklog}
        isPending={create.isPending || update.isPending}
        error={(create.error ?? update.error) as Error | null}
      />
      <ConfirmDialog
        open={deleteConfirmOpen}
        returnFocusRef={issueActionsRef}
        onOpenChange={(open) => {
          setDeleteConfirmOpen(open)
          if (!open) deleteIssueMutation.reset()
        }}
        isPending={deleteIssueMutation.isPending}
        error={deleteIssueMutation.error ? t('issue.deleteError') : null}
        title={t('issue.delete')}
        description={t('issue.deleteConfirm')}
        onConfirm={() => deleteIssueMutation.mutate(id)}
      />
    </div>
  )
}

const desktopLayoutQuery = '(min-width: 1024px)'

function IssueDetailLayout({
  description,
  details,
  detailsSummary,
  activity,
  supplemental,
}: {
  description: ReactNode
  details: ReactNode
  detailsSummary: ReactNode
  activity: ReactNode
  supplemental: ReactNode
}) {
  const desktop = useMediaQuery(desktopLayoutQuery)

  return (
    <div
      className={
        desktop
          ? 'grid min-w-0 grid-cols-[minmax(0,1fr)_320px] grid-rows-[min-content_minmax(0,1fr)] gap-6'
          : 'grid min-w-0 gap-2'
      }
    >
      {description}
      {desktop ? (
        <aside className="sticky top-16 col-start-2 row-start-1 row-span-2 self-start space-y-2">
          {details}
          {supplemental}
        </aside>
      ) : (
        <Disclosure appearance="row" summary={detailsSummary}>
          {details}
        </Disclosure>
      )}
      {activity}
      {!desktop && <aside className="space-y-2">{supplemental}</aside>}
    </div>
  )
}
