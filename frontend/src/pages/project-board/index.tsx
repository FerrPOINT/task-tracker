import { PriorityBadge } from '@/features/issues/ui/PriorityBadge'
import { CreateTaskAction } from '@/features/issues/ui/CreateTaskAction'
import {
  Card,
  CardContent,
  CardTitle,
  PageHeader,
  ProjectAvatar,
  Button,
  ErrorState,
  LoadingState,
  UserAvatar,
} from '@pdlc/ui/ui'
import { ProjectLink as Link } from '@/shared/lib/project-navigation'
import { useParams } from 'react-router'
import { List } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import { useBoard } from '@/shared/api/hooks'

import type { components } from '@/api/generated'

export type Issue = components['schemas']['IssueResponse']

function IssueCard({ issue, status }: { issue: Issue; status?: string }) {
  return (
    <Card asChild interactive surface="raised">
      <Link to={`/issues/${issue.id}`} aria-label={`${issue.key} ${issue.summary}`}>
        <article>
          <CardContent density="compact" className="space-y-2">
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs text-text-muted">{issue.key}</span>
              {issue.assignee_id && (
                <UserAvatar name={issue.assignee_name} userId={issue.assignee_id} />
              )}
            </div>
            <CardTitle className="break-words">{issue.summary}</CardTitle>
            <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
              {status && <span className="text-xs text-text-muted">{status}</span>}
              <PriorityBadge priority={issue.priority} />
            </div>
          </CardContent>
        </article>
      </Link>
    </Card>
  )
}

function SdlcBoard({
  sdlc,
  issues,
}: {
  sdlc: components['schemas']['SdlcBoardResponse']
  issues: Issue[]
}) {
  const { t } = useTranslation()
  const tasks = new Map(sdlc.tasks.map((task) => [task.issue_id, task]))
  return (
    <>
      <div
        role="region"
        aria-label={t('board.columns')}
        className="grid min-w-0 gap-3 md:grid-cols-[repeat(auto-fill,minmax(min(100%,16rem),1fr))]"
      >
        {sdlc.stages.map((stage) => {
          const stageIssues = issues.filter((issue) => tasks.get(issue.id)?.stage === stage)
          return (
            <section key={stage} className="min-w-0 rounded-md border border-border bg-surface">
              <div className="flex items-center justify-between p-2">
                <h2 className="text-sm font-semibold">{t(`board.sdlcStage.${stage}`)}</h2>
                <div className="text-xs text-text-muted">{stageIssues.length}</div>
              </div>
              <div className="space-y-2 p-2">
                {stageIssues.map((issue) => {
                  const status = tasks.get(issue.id)?.status
                  return (
                    <IssueCard
                      key={issue.id}
                      issue={issue}
                      status={status ? t(`board.sdlcStatus.${status}`) : undefined}
                    />
                  )
                })}
                {stageIssues.length === 0 && (
                  <p className="text-xs text-text-muted">{t('board.emptyStatus')}</p>
                )}
              </div>
            </section>
          )
        })}
      </div>
    </>
  )
}

export function ProjectBoardPage() {
  const { t } = useTranslation()
  const { projectKey } = useParams<{ projectKey?: string }>()
  const key = projectKey ?? ''
  const { data: board, isLoading, error } = useBoard(key)
  if (isLoading) return <LoadingState message={t('issue.loading')} />
  if (error || !board) return <ErrorState message={error?.message ?? t('issue.notFound')} />

  const { issues, sdlc } = board

  const header = (
    <div className="mb-4">
      <PageHeader
        title={t('board.title', { projectName: key })}
        leading={<ProjectAvatar projectKey={key} />}
        context={t('board.subtitle', { count: sdlc.tasks.length })}
        actions={
          <>
            <Button
              variant="outline"
              size="sm"
              className="gap-1"
              asChild
              aria-label={t('board.backlog')}
            >
              <Link to={`/projects/${key}/backlog`}>
                <List className="h-4 w-4" />
                <span className="hidden sm:inline">{t('board.backlog')}</span>
              </Link>
            </Button>
            <CreateTaskAction projectKey={key} aria-label={t('board.create')} size="sm">
              {t('board.create')}
            </CreateTaskAction>
          </>
        }
      />
    </div>
  )

  return (
    <div className="min-w-0">
      {header}
      <SdlcBoard sdlc={board.sdlc} issues={issues} />
    </div>
  )
}
