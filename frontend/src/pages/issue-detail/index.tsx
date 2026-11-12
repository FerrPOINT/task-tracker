import { useClipboard, useMediaQuery } from '@pdlc/ui/lib'
import {
  Disclosure,
  ActionGroup,
  TextLink,
  ProjectAvatar,
  toast,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
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
import { AttachmentPanel } from '@/features/issue-detail/ui/AttachmentPanel'
import { LabelEditor } from '@/features/issue-detail/ui/LabelEditor'
import { LinkEditor } from '@/features/issue-detail/ui/LinkEditor'
import { CustomFieldsPanel } from '@/features/issue-detail/ui/CustomFieldsPanel'
import { IssueEngagementPanel } from '@/features/issue-detail/ui/IssueEngagementPanel'
import type { Worklog, LogWorkInput } from '@/entities/worklog/model'
import { useAuthStore } from '@/shared/auth/store'
import { IssueMetaEditor } from '@/features/issue-detail/ui/IssueMetaEditor'
import { IssueDescriptionEditor } from '@/features/issue-detail/ui/IssueDescriptionEditor'
import {
  useBoard,
  useUpdateIssue,
  useDeleteIssue,
  useSprints,
  useIssue,
  useProjectCustomFields,
} from '@/shared/api/hooks'
import { TaskDocumentsPanel } from '@/features/issue-detail/ui/TaskDocumentsPanel'
import { TaskRepositoriesPanel } from '@/features/issue-detail/ui/TaskRepositoriesPanel'
import { DecompositionPanel } from '@/features/decomposition/panel'
import { SdlcPanel } from '@/features/sdlc/ui/SdlcPanel'
import { useSdlcSnapshot } from '@/features/sdlc/model/use-sdlc'

const issueTabs = ['comments', 'worklog', 'decomposition', 'sdlc'] as const
const issueTabPanelStyle = { marginTop: 'var(--shell-page-block-gap)' }
type IssueTab = (typeof issueTabs)[number]

function parseIssueTab(value: string | null): IssueTab {
  return issueTabs.find((tab) => tab === value) ?? issueTabs[0]
}

type IssueDetailPageProps = { decompositionPanel?: ReactNode }

export function IssueDetailPage(props: IssueDetailPageProps = {}) {
  const { id = '' } = useParams()
  return <IssueDetail key={id} id={id} {...props} />
}

function IssueDetail({ id, decompositionPanel }: IssueDetailPageProps & { id: string }) {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const tabParam = searchParams.get('tab')
  const activeTab = parseIssueTab(tabParam)
  const [decompositionVisited, setDecompositionVisited] = useState(activeTab === 'decomposition')
  useEffect(() => {
    if (activeTab === 'decomposition') setDecompositionVisited(true)
  }, [activeTab])
  const latestTab = useRef(activeTab)
  const currentUserId = useAuthStore((s) => s.userId)
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editingWorklog, setEditingWorklog] = useState<Worklog | undefined>(undefined)
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false)
  const issueActionsRef = useRef<HTMLButtonElement | null>(null)
  const worklogTabRef = useRef<HTMLButtonElement | null>(null)

  const issueQuery = useIssue(id)
  const issueClipboard = useClipboard(issueQuery.data?.key ?? '', {
    onCopied: () => toast.success(t('issue.keyCopied')),
    onCopyError: () => toast.error(t('issue.copyFailed')),
  })
  const boardQuery = useBoard(issueQuery.data?.project_key)
  const sprintsQuery = useSprints(issueQuery.data?.project_key)
  const customFieldsQuery = useProjectCustomFields(issueQuery.data?.project_key ?? '')
  const updateIssue = useUpdateIssue(id)
  const deleteIssueMutation = useDeleteIssue()
  // Historical issue URLs may contain a key; owner APIs use the loaded UUID.
  const taskId = issueQuery.data?.id ?? ''
  const { snapshot: taskSnapshot } = useSdlcSnapshot(taskId)
  const worklogsQuery = useWorklogs(taskId)
  const create = useCreateWorklog(taskId)
  const update = useUpdateWorklog(taskId)
  const remove = useDeleteWorklog(taskId)

  useEffect(() => {
    latestTab.current = activeTab
  }, [activeTab])

  useEffect(() => {
    const isCanonical = tabParam === null || (activeTab !== issueTabs[0] && tabParam === activeTab)
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
  const lifecycle = boardQuery.data?.sdlc?.tasks.find((task) => task.issue_id === issue.id)
  const showCustomFields = customFieldsQuery.data?.length !== 0 || customFieldsQuery.isError

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
    const nextTab = parseIssueTab(value)
    if (nextTab === latestTab.current) return
    latestTab.current = nextTab
    const next = new URLSearchParams(searchParams)
    if (nextTab === issueTabs[0]) next.delete('tab')
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
            {lifecycle && (
              <p aria-label={t('issue.status')} className="text-sm font-medium text-text-primary">
                {t(`board.sdlcStage.${lifecycle.stage}`)}
                {lifecycle.status && ` · ${t(`board.sdlcStatus.${lifecycle.status}`)}`}
              </p>
            )}
          </div>
          <ActionGroup className="ml-auto">
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
              {t('issue.assignResponsibility')}
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
          description={
            <section className="grid min-w-0 gap-[var(--shell-page-block-gap)] border-t border-border pt-5 lg:col-start-1 lg:row-start-1">
              {taskSnapshot.error && (
                <ErrorState
                  message={t('issue.requirementsLoadError')}
                  onRetry={() => void taskSnapshot.refetch()}
                />
              )}
              {taskSnapshot.isLoading ? (
                <LoadingState message={t('sdlc.loading')} />
              ) : (
                <IssueDescriptionEditor
                  issue={issue}
                  requirements={taskSnapshot.error ? undefined : taskSnapshot.data?.requirements}
                  disabled={updateIssue.isPending}
                  onSubmit={async (patch) => {
                    await updateIssue.mutateAsync(patch)
                    toast.success(t('common.saved'))
                  }}
                />
              )}
              <section
                aria-label={t('attachments.title')}
                className="min-w-0 border-t border-border pt-5"
              >
                <AttachmentPanel issueId={issue.id} />
              </section>
            </section>
          }
          details={
            <Card>
              <CardHeader>
                <CardTitle className="text-sm">{t('issue.details')}</CardTitle>
              </CardHeader>
              <CardContent className="grid gap-[var(--shell-page-block-gap)] text-sm">
                <IssueMetaEditor
                  issue={issue}
                  sprints={sprintsQuery.data ?? []}
                  disabled={updateIssue.isPending}
                  onChange={(patch) =>
                    updateIssue.mutate(patch, { onError: (error) => toast.error(error.message) })
                  }
                />
                <LabelEditor issueId={issue.id} projectKey={issue.project_key} />
              </CardContent>
            </Card>
          }
          activity={
            <div className="flex min-w-0 flex-col gap-[var(--shell-page-block-gap)] lg:col-start-1 lg:row-start-2">
              <Tabs value={activeTab} onValueChange={updateActiveTab}>
                <TabsList className="grid h-auto w-full grid-cols-2 gap-1 sm:grid-cols-4">
                  <TabsTrigger value="comments">{t('issue.discussion')}</TabsTrigger>
                  <TabsTrigger ref={worklogTabRef} value="worklog">
                    {t('timeTracking.worklog.title')}
                  </TabsTrigger>
                  <TabsTrigger value="decomposition">Декомпозиция</TabsTrigger>
                  <TabsTrigger value="sdlc">{t('sdlc.title')}</TabsTrigger>
                </TabsList>
                <TabsContent value="comments" style={issueTabPanelStyle}>
                  <CommentsPanel issueId={issue.id} currentUserId={currentUserId ?? undefined} />
                </TabsContent>
                <TabsContent value="worklog" style={issueTabPanelStyle}>
                  <ActionGroup className="mb-[var(--shell-page-block-gap)]">
                    <Button size="sm" onClick={handleLogWork}>
                      {t('timeTracking.logWork')}
                    </Button>
                  </ActionGroup>
                  {worklogsQuery.error ? (
                    <ErrorState
                      message={t('timeTracking.worklog.loadError')}
                      onRetry={() => void worklogsQuery.refetch()}
                    />
                  ) : worklogsQuery.isLoading ? (
                    <p className="py-4 text-sm text-text-muted">{t('common.loading')}</p>
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
                <TabsContent
                  key={issue.id}
                  value="decomposition"
                  forceMount
                  hidden={activeTab !== 'decomposition'}
                  style={issueTabPanelStyle}
                >
                  {(activeTab === 'decomposition' || decompositionVisited) &&
                    (decompositionPanel ?? <DecompositionPanel issueId={issue.id} />)}
                </TabsContent>
                <TabsContent value="sdlc" style={issueTabPanelStyle}>
                  <SdlcPanel key={issue.id} issueId={issue.id} />
                </TabsContent>
              </Tabs>
            </div>
          }
          supplemental={
            <>
              {showCustomFields && (
                <Card>
                  <CardHeader>
                    <CardTitle className="text-sm">{t('customFields.title')}</CardTitle>
                  </CardHeader>
                  <CardContent>
                    <CustomFieldsPanel issueId={issue.id} projectKey={issue.project_key} />
                  </CardContent>
                </Card>
              )}
              <Card>
                <CardContent className="grid gap-4">
                  <LinkEditor issueId={issue.id} currentKey={issue.key} />
                  {import.meta.env.VITE_NAMESPACE_ENABLED === 'true' && (
                    <TaskDocumentsPanel taskId={issue.id} projectKey={issue.project_key} />
                  )}
                  {import.meta.env.VITE_NAMESPACE_ENABLED === 'true' && (
                    <TaskRepositoriesPanel taskId={issue.id} projectKey={issue.project_key} />
                  )}
                </CardContent>
              </Card>
              <Card>
                <CardHeader>
                  <CardTitle className="text-sm">{t('timeTracking.title')}</CardTitle>
                </CardHeader>
                <CardContent>
                  <TimeTrackingPanel
                    timeSpentSeconds={issue.time_spent_seconds}
                    originalEstimateSeconds={issue.original_estimate_seconds ?? null}
                    remainingEstimateSeconds={issue.remaining_estimate_seconds ?? null}
                    onLogWork={handleLogWork}
                  />
                </CardContent>
              </Card>

              <Disclosure summary={t('engagement.title')}>
                <IssueEngagementPanel
                  issueId={issue.id}
                  projectKey={issue.project_key}
                  currentUserId={currentUserId}
                  reporterId={issue.reporter_id}
                />
              </Disclosure>
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
  activity,
  supplemental,
}: {
  description: ReactNode
  details: ReactNode
  activity: ReactNode
  supplemental: ReactNode
}) {
  const { t } = useTranslation()
  const desktop = useMediaQuery(desktopLayoutQuery)

  return (
    <div
      className={
        desktop
          ? 'grid min-w-0 grid-cols-[minmax(0,1fr)_320px] grid-rows-[min-content_minmax(0,1fr)] gap-[var(--shell-page-block-gap)]'
          : 'grid min-w-0 gap-[var(--shell-page-block-gap)]'
      }
    >
      {description}
      {desktop ? (
        <aside className="sticky top-16 col-start-2 row-start-1 row-span-2 grid self-start gap-[var(--shell-page-block-gap)]">
          {details}
          {supplemental}
        </aside>
      ) : (
        <Disclosure summary={t('issue.properties')}>
          <div className="grid gap-[var(--shell-page-block-gap)]">
            {details}
            {supplemental}
          </div>
        </Disclosure>
      )}
      {activity}
    </div>
  )
}
