import { useParams } from 'react-router'
import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { Copy, MessageSquare, UserPlus, MoreHorizontal, Pencil } from 'lucide-react'
import { Toaster, toast } from 'sonner'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/shared/ui/tabs'
import { Card, CardContent, CardHeader, CardTitle } from '@/shared/ui/card'
import { Button } from '@/shared/ui/button'
import { getIssue } from '@/api/issue'
import {
  useWorklogs,
  useCreateWorklog,
  useUpdateWorklog,
  useDeleteWorklog,
  totalTimeSpent,
  latestRemainingEstimate,
} from '@/features/time-tracking/model/use-worklogs'
import { TimeTrackingPanel } from '@/features/time-tracking/ui/TimeTrackingPanel'
import { WorklogTab } from '@/features/time-tracking/ui/WorklogTab'
import { LogWorkDialog } from '@/features/time-tracking/ui/LogWorkDialog'
import { CommentsPanel } from '@/features/comments/ui/CommentList'
import { ThemeToggle } from '@/shared/ui/theme-toggle'
import type { Worklog, LogWorkInput } from '@/entities/worklog/model'
import { useAuthStore } from '@/shared/auth/store'
import { IssueMetaEditor } from '@/features/issue-detail/ui/IssueMetaEditor'
import { IssueDescriptionEditor } from '@/features/issue-detail/ui/IssueDescriptionEditor'
import { useBoard } from '@/shared/api/hooks'
import { useUpdateIssue } from '@/shared/api/hooks'

export function IssueDetailPage() {
  const { id = '' } = useParams()
  const { t } = useTranslation()
  const currentUserId = useAuthStore((s) => s.userId)
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editingWorklog, setEditingWorklog] = useState<Worklog | undefined>(undefined)

  const issueQuery = useQuery({
    queryKey: ['issue', id],
    queryFn: () => getIssue(id),
    refetchOnWindowFocus: false,
    staleTime: 0,
  })
  const boardQuery = useBoard(issueQuery.data?.project_key)
  const updateIssue = useUpdateIssue(id)
  const worklogsQuery = useWorklogs(id)
  const create = useCreateWorklog(id)
  const update = useUpdateWorklog(id)
  const remove = useDeleteWorklog(id)

  if (issueQuery.isLoading || worklogsQuery.isLoading) {
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
  const worklogs = worklogsQuery.data ?? []
  const timeSpent = totalTimeSpent(worklogs)
  const remainingEstimate = latestRemainingEstimate(worklogs)

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
      <header className="flex h-12 items-center justify-between border-b border-border bg-surface px-4">
        <div className="flex items-center gap-4">
          <span className="font-bold text-text-primary">≡ TaskTracker</span>
          <span className="text-sm text-text-secondary">{issue.project_name}</span>
        </div>
        <ThemeToggle />
      </header>

      <main className="mx-auto max-w-6xl p-4 md:p-6">
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
            <Button variant="secondary" size="sm">
              <Pencil className="h-4 w-4" />
              {t('issue.edit')}
            </Button>
            <Button variant="secondary" size="sm">
              <MessageSquare className="h-4 w-4" />
              {t('issue.comment')}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              disabled={updateIssue.isPending || currentUserId === issue.assignee_id}
              onClick={() =>
                currentUserId && updateIssue.mutate({ assignee_id: currentUserId })
              }
            >
              <UserPlus className="h-4 w-4" />
              {t('issue.assignToMe')}
            </Button>
            <Button variant="secondary" size="icon">
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

            <Tabs defaultValue="worklog">
              <TabsList>
                <TabsTrigger value="comments">{t('issue.comments')}</TabsTrigger>
                <TabsTrigger value="activity">{t('issue.activity')}</TabsTrigger>
                <TabsTrigger value="worklog">{t('timeTracking.worklog.title')}</TabsTrigger>
                <TabsTrigger value="history">{t('issue.history')}</TabsTrigger>
              </TabsList>
              <TabsContent value="comments">
                <CommentsPanel issueId={id} currentUserId={currentUserId ?? undefined} />
              </TabsContent>
              <TabsContent value="activity">
                <p className="text-sm text-text-muted">No activity yet.</p>
              </TabsContent>
              <TabsContent value="worklog">
                <WorklogTab
                  worklogs={worklogs}
                  onEdit={handleEdit}
                  onDelete={handleDelete}
                  currentUserId="user-1"
                />
              </TabsContent>
              <TabsContent value="history">
                <p className="text-sm text-text-muted">No history yet.</p>
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
                  timeSpentSeconds={timeSpent}
                  originalEstimateSeconds={0}
                  remainingEstimateSeconds={remainingEstimate}
                  onLogWork={handleLogWork}
                />
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
                  disabled={updateIssue.isPending}
                  onChange={(patch) => updateIssue.mutate(patch)}
                />
              </CardContent>
            </Card>
          </div>
        </div>
      </main>

      <LogWorkDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        onSubmit={handleSubmit}
        worklog={editingWorklog}
      />
      <Toaster position="top-center" richColors />
    </div>
  )
}
