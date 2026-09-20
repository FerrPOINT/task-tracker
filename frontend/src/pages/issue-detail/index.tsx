import { useParams } from 'react-router'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Copy, UserPlus, MoreHorizontal } from 'lucide-react'
import { toast } from 'sonner'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@sdlc/ui/ui'
import { Card, CardContent, CardHeader, CardTitle } from '@sdlc/ui/ui'
import { Button } from '@sdlc/ui/ui'
import { ConfirmDialog } from '@sdlc/ui/ui'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  LoadingState,
  ErrorState,
} from '@sdlc/ui/ui'
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
import { IssueMetaEditor } from '@/features/issue-detail/ui/IssueMetaEditor'
import { IssueDescriptionEditor } from '@/features/issue-detail/ui/IssueDescriptionEditor'
import { useBoard, useUpdateIssue, useDeleteIssue, useSprints, useIssue } from '@/shared/api/hooks'

export function IssueDetailPage() {
  const { id = '' } = useParams()
  const { t } = useTranslation()
  const currentUserId = useAuthStore((s) => s.userId)
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editingWorklog, setEditingWorklog] = useState<Worklog | undefined>(undefined)
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false)

  const issueQuery = useIssue(id)
  const boardQuery = useBoard(issueQuery.data?.project_key)
  const sprintsQuery = useSprints(issueQuery.data?.project_key)
  const updateIssue = useUpdateIssue(id)
  const deleteIssueMutation = useDeleteIssue()
  const worklogsQuery = useWorklogs(id)
  const commentsQuery = useComments(id)
  const create = useCreateWorklog(id)
  const update = useUpdateWorklog(id)
  const remove = useDeleteWorklog(id)

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

  const copyKey = () => {
    navigator.clipboard.writeText(issue.key)
    toast.success(t('issue.copyKey'))
  }

  return (
    <div className="min-w-0 bg-background">
      <div>
        {issueQuery.error && (
          <ErrorState message={t('issue.refreshError')} onRetry={() => void issueQuery.refetch()} />
        )}
        <div className="mb-2 text-sm text-text-muted">
          {issue.project_name} / {issue.key}
        </div>

        <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
          <span className="rounded bg-accent/20 px-2 py-0.5 text-xs font-medium text-text-primary">
            {t(`issueType.${issue.issue_type.toLowerCase()}`, { defaultValue: issue.issue_type })}
          </span>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant="secondary"
              size="sm"
              className="h-10 lg:h-8"
              disabled={updateIssue.isPending || currentUserId === issue.assignee_id}
              onClick={() => currentUserId && updateIssue.mutate({ assignee_id: currentUserId })}
            >
              <UserPlus className="h-4 w-4" />
              {t('issue.assignToMe')}
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="secondary"
                  size="icon"
                  className="h-10 w-10 lg:h-9 lg:w-9"
                  aria-label={t('issue.actions')}
                >
                  <MoreHorizontal className="h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem onClick={copyKey} className="gap-2">
                  <Copy className="h-4 w-4" />
                  {t('issue.copyKey')}
                </DropdownMenuItem>
                <DropdownMenuItem
                  onClick={() => setDeleteConfirmOpen(true)}
                  className="gap-2 text-danger"
                >
                  {t('issue.delete')}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>

        <div className="grid min-w-0 grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_320px] lg:grid-rows-[auto_auto_1fr] lg:gap-x-6">
          <section className="min-w-0 border-t border-border pt-5 lg:col-start-1 lg:row-start-1">
            <IssueDescriptionEditor
              issue={issue}
              disabled={updateIssue.isPending}
              onSubmit={async (patch) => {
                await updateIssue.mutateAsync(patch)
                toast.success(t('common.saved'))
              }}
            />
          </section>

          <Card className="lg:sticky lg:top-16 lg:col-start-2 lg:row-start-1 lg:row-span-2 lg:self-start">
            <CardHeader>
              <CardTitle className="text-sm">{t('issue.details')}</CardTitle>
            </CardHeader>
            <CardContent className="text-sm">
              <IssueMetaEditor
                issue={issue}
                columns={boardQuery.data?.columns ?? []}
                sprints={sprintsQuery.data ?? []}
                disabled={updateIssue.isPending}
                onChange={(patch) =>
                  updateIssue.mutate(patch, { onError: (error) => toast.error(error.message) })
                }
              />
            </CardContent>
          </Card>

          <div className="min-w-0 space-y-6 lg:col-start-1 lg:row-start-2 lg:row-span-2 lg:self-start">
            <Tabs defaultValue="activity">
              <TabsList className="grid h-auto w-full grid-cols-2 gap-1 sm:grid-cols-4">
                <TabsTrigger className="min-h-10" value="activity">
                  {t('issue.activity')}
                </TabsTrigger>
                <TabsTrigger className="min-h-10" value="comments">
                  {t('issue.comments')}
                </TabsTrigger>
                <TabsTrigger className="min-h-10" value="worklog">
                  {t('timeTracking.worklog.title')}
                </TabsTrigger>
                <TabsTrigger className="min-h-10" value="attachments">
                  {t('attachments.title')}
                </TabsTrigger>
              </TabsList>
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
                  <p className="py-4 text-sm text-text-muted">{t('issue.loadingActivity')}</p>
                ) : hasActivityData || !activityError ? (
                  <ActivityFeed comments={commentsQuery.data ?? []} worklogs={worklogs} />
                ) : null}
              </TabsContent>
              <TabsContent value="comments">
                <CommentsPanel issueId={id} currentUserId={currentUserId ?? undefined} />
              </TabsContent>
              <TabsContent value="worklog">
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
                  />
                )}
              </TabsContent>
              <TabsContent value="attachments">
                <AttachmentPanel issueId={id} />
              </TabsContent>
            </Tabs>

            <div className="grid gap-4 md:grid-cols-2">
              <Card>
                <CardContent className="pt-5">
                  <LabelEditor issueId={id} projectKey={issue.project_key} />
                </CardContent>
              </Card>
              <Card>
                <CardHeader>
                  <CardTitle className="text-sm">{t('customFields.title')}</CardTitle>
                </CardHeader>
                <CardContent>
                  <CustomFieldsPanel issueId={id} projectKey={issue.project_key} />
                </CardContent>
              </Card>
              <Card className="md:col-span-2">
                <CardContent className="pt-5">
                  <LinkEditor issueId={id} currentKey={issue.key} />
                </CardContent>
              </Card>
            </div>
          </div>

          <aside className="space-y-4 lg:col-start-2 lg:row-start-3 lg:self-start">
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

            <Card>
              <CardHeader>
                <CardTitle className="text-sm">{t('engagement.title')}</CardTitle>
              </CardHeader>
              <CardContent>
                <IssueEngagementPanel
                  issueId={id}
                  projectKey={issue.project_key}
                  currentUserId={currentUserId}
                  reporterId={issue.reporter_id}
                />
              </CardContent>
            </Card>
          </aside>
        </div>
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
        onOpenChange={setDeleteConfirmOpen}
        isPending={deleteIssueMutation.isPending}
        error={deleteIssueMutation.error?.message}
        title={t('issue.delete')}
        description={t('issue.deleteConfirm')}
        onConfirm={() => {
          deleteIssueMutation.mutate(id, {
            onSuccess: () => setDeleteConfirmOpen(false),
            onError: (error) => toast.error(error.message),
          })
        }}
      />
    </div>
  )
}
