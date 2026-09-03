import { useParams } from 'react-router'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Copy, UserPlus, MoreHorizontal } from 'lucide-react'
import { Toaster, toast } from 'sonner'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@sdlc/ui/ui'
import { Card, CardContent, CardHeader, CardTitle } from '@sdlc/ui/ui'
import { Button } from '@sdlc/ui/ui'
import { ConfirmDialog } from '@sdlc/ui/ui'
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
  const { data: worklogsData, isLoading: worklogsLoading } = useWorklogs(id)
  const commentsQuery = useComments(id)
  const create = useCreateWorklog(id)
  const update = useUpdateWorklog(id)
  const remove = useDeleteWorklog(id)

  if (issueQuery.isLoading || worklogsLoading) {
    return (
      <div className="flex h-screen items-center justify-center bg-background">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-border-strong border-t-accent"></div>
      </div>
    )
  }

  if (!issueQuery.data) {
    return (
      <div className="flex h-screen items-center justify-center bg-background text-text-secondary">
        {t('issue.notFound')}
      </div>
    )
  }

  const issue = issueQuery.data
  const worklogs = worklogsData ?? []

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
      update.mutate({ id: editingWorklog.id, input })
    } else {
      create.mutate(input)
    }
  }

  const handleDelete = (worklogId: string) => {
    remove.mutate(worklogId)
  }

  const copyKey = () => {
    navigator.clipboard.writeText(issue.key)
    toast.success(t('issue.copyKey'))
  }

  return (
    <div className="min-h-screen bg-background">
      <div>
        <div className="mb-2 text-sm text-text-muted">
          {issue.project_name} / {issue.key}
        </div>

        <div className="mb-4 flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
          <div className="flex items-start gap-3">
            <span className="mt-1 rounded bg-accent/20 px-2 py-0.5 text-xs font-medium text-accent">
              Task
            </span>
            <div>
              <h1 className="text-2xl font-semibold text-text-primary">
                {issue.key} {issue.summary}
              </h1>
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="secondary" size="sm" onClick={copyKey}>
              <Copy className="h-4 w-4" />
              {t('issue.copyKey')}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              disabled={updateIssue.isPending || currentUserId === issue.assignee_id}
              onClick={() => currentUserId && updateIssue.mutate({ assignee_id: currentUserId })}
            >
              <UserPlus className="h-4 w-4" />
              {t('issue.assignToMe')}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              disabled={deleteIssueMutation.isPending}
              onClick={() => setDeleteConfirmOpen(true)}
            >
              {t('issue.delete')}
            </Button>
            <Button variant="secondary" size="icon" aria-label={t('issue.actions')}>
              <MoreHorizontal className="h-4 w-4" />
            </Button>
          </div>
        </div>

        <div className="grid grid-cols-1 gap-6 lg:grid-cols-[1fr_300px]">
          <div className="space-y-6">
            <Card>
              <CardContent className="pt-6">
                <IssueDescriptionEditor
                  issue={issue}
                  disabled={updateIssue.isPending}
                  onSubmit={(patch) => updateIssue.mutate(patch)}
                />
              </CardContent>
            </Card>

            <Tabs defaultValue="activity">
              <TabsList>
                <TabsTrigger value="activity">{t('issue.activity')}</TabsTrigger>
                <TabsTrigger value="comments">{t('issue.comments')}</TabsTrigger>
                <TabsTrigger value="worklog">{t('timeTracking.worklog.title')}</TabsTrigger>
                <TabsTrigger value="attachments">{t('attachments.title')}</TabsTrigger>
              </TabsList>
              <TabsContent value="activity">
                <ActivityFeed comments={commentsQuery.data ?? []} worklogs={worklogs} />
              </TabsContent>
              <TabsContent value="comments">
                <CommentsPanel issueId={id} currentUserId={currentUserId ?? undefined} />
              </TabsContent>
              <TabsContent value="worklog">
                <WorklogTab
                  worklogs={worklogs}
                  onEdit={handleEdit}
                  onDelete={handleDelete}
                  currentUserId={currentUserId ?? ''}
                />
              </TabsContent>
              <TabsContent value="attachments">
                <AttachmentPanel issueId={id} />
              </TabsContent>
            </Tabs>
          </div>

          <div className="space-y-4">
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

            <Card>
              <CardHeader>
                <CardTitle className="text-sm">{t('labels.title')}</CardTitle>
              </CardHeader>
              <CardContent>
                <LabelEditor issueId={id} projectKey={issueQuery.data?.project_key ?? ''} />
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

            <Card>
              <CardHeader>
                <CardTitle className="text-sm">{t('links.title')}</CardTitle>
              </CardHeader>
              <CardContent>
                <LinkEditor issueId={id} currentKey={issueQuery.data?.key ?? ''} />
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="text-sm">{t('issue.details')}</CardTitle>
              </CardHeader>
              <CardContent className="text-sm">
                <IssueMetaEditor
                  issue={issue}
                  columns={boardQuery.data?.columns ?? []}
                  sprints={sprintsQuery.data ?? []}
                  disabled={updateIssue.isPending}
                  onChange={(patch) => updateIssue.mutate(patch)}
                />
              </CardContent>
            </Card>
          </div>
        </div>
      </div>

      <LogWorkDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        onSubmit={handleSubmit}
        worklog={editingWorklog}
      />
      <ConfirmDialog
        open={deleteConfirmOpen}
        onOpenChange={setDeleteConfirmOpen}
        title={t('issue.delete')}
        description={t('issue.deleteConfirm')}
        onConfirm={() => {
          deleteIssueMutation.mutate(id)
          setDeleteConfirmOpen(false)
        }}
      />
      <Toaster position="top-center" richColors />
    </div>
  )
}
