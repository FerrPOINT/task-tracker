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
import { ThemeToggle } from '@/shared/ui/theme-toggle'
import type { Worklog, LogWorkInput } from '@/entities/worklog/model'

export function IssueDetailPage() {
  const { id = '' } = useParams()
  const { t } = useTranslation()
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editingWorklog, setEditingWorklog] = useState<Worklog | undefined>(undefined)

  const issueQuery = useQuery({
    queryKey: ['issue', id],
    queryFn: () => getIssue(id),
    refetchOnWindowFocus: false,
    staleTime: 0,
  })
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

  const renderDescription = (text: string) => {
    const lines = text.split('\n')
    return (
      <div className="space-y-3 text-sm text-text-secondary">
        {lines.map((line, idx) => {
          if (line.startsWith('· ')) {
            return (
              <ul key={idx} className="ml-5 list-disc">
                <li>{line.slice(2)}</li>
              </ul>
            )
          }
          if (line.trim() === '') return <div key={idx} className="h-2" />
          return <p key={idx}>{line}</p>
        })}
      </div>
    )
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
            <span className="mt-1 rounded bg-accent/20 px-2 py-0.5 text-xs font-medium text-accent">Task</span>
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
            <Button variant="secondary" size="sm">
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
              <CardHeader>
                <CardTitle className="text-sm">{t('issue.description')}</CardTitle>
              </CardHeader>
              <CardContent>
                {issue.description ? renderDescription(issue.description) : <p className="text-sm text-text-muted">{t('issue.noDescription')}</p>}
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
                <p className="text-sm text-text-muted">No comments yet.</p>
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
              <CardContent className="space-y-3 text-sm">
                <DetailRow label={t('issue.status')} value={issue.status} />
                <DetailRow label={t('issue.assignee')} value={issue.assignee_name ?? '—'} />
                <DetailRow label={t('issue.reporter')} value={issue.reporter_name ?? '—'} />
                <DetailRow label={t('issue.priority')} value={issue.priority} />
                {issue.labels.length > 0 && (
                  <DetailRow
                    label={t('issue.labels')}
                    value={
                      <div className="flex flex-wrap gap-1">
                        {issue.labels.map((l) => (
                          <span key={l} className="rounded bg-surface-raised px-2 py-0.5 text-xs text-text-secondary">
                            {l}
                          </span>
                        ))}
                      </div>
                    }
                  />
                )}
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

function DetailRow({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-4">
      <span className="text-text-muted">{label}</span>
      <span className="text-right text-text-primary">{value}</span>
    </div>
  )
}
