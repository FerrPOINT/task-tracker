import { PriorityBadge } from '@/features/issues/ui/PriorityBadge'
import { CreateTaskAction } from '@/features/issues/ui/CreateTaskAction'
import {
  RadioGroup,
  TextLink,
  Card,
  CardContent,
  Badge,
  PageHeader,
  ProjectAvatar,
  Button,
  ErrorState,
  LoadingState,
  UserAvatar,
} from '@pdlc/ui/ui'
import { ProjectLink as Link } from '@/shared/lib/project-navigation'
import { useParams, useSearchParams } from 'react-router'
import { List } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import { useBoard } from '@/shared/api/hooks'

import type { components } from '@/api/generated'

export type Issue = components['schemas']['IssueResponse']

function IssueCard({ issue, compact, status }: { issue: Issue; compact: boolean; status?: string }) {
  const { t } = useTranslation()
  return (
    <Card asChild surface="raised">
      <article>
        <CardContent density={compact ? 'compact' : 'default'}>
          <div className="flex items-start gap-2">
            <TextLink asChild className="min-w-0 flex-1">
              <Link to={`/issues/${issue.id}`}>
                <div className="text-xs text-text-muted">{issue.key}</div>
                <div
                  className={`${compact ? 'my-0.5' : 'my-0.5 md:my-1'} break-words text-sm font-medium`}
                >
                  {issue.summary}
                </div>
              </Link>
            </TextLink>
          </div>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <PriorityBadge priority={issue.priority} />
              <Badge tone="muted">
                {t(`issueType.${issue.issue_type.toLowerCase()}`, {
                  defaultValue: issue.issue_type,
                })}
              </Badge>
            </div>
            {status && <Badge tone="muted">{status}</Badge>}
            <UserAvatar name={issue.assignee_name} userId={issue.assignee_id} />
          </div>
        </CardContent>
      </article>
    </Card>
  )
}

function SdlcBoard({
  projectKey,
  sdlc,
  issues,
  compact,
}: {
  projectKey: string
  sdlc: components['schemas']['SdlcBoardResponse']
  issues: Issue[]
  compact: boolean
}) {
  const { t } = useTranslation()
  const tasks = new Map(sdlc.tasks.map((task) => [task.issue_id, task]))
  return (
    <>
      <div
        role="region"
        aria-label={t('board.columns')}
        className="grid min-w-0 gap-3 md:grid-cols-2 xl:grid-cols-3"
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
                      compact={compact}
                      status={status ? t(`board.sdlcStatus.${status}`) : undefined}
                    />
                  )
                })}
                {stageIssues.length === 0 && (
                  <p className="text-xs text-text-muted">{t('board.emptyStatus')}</p>
                )}
              </div>
              {stage === 'Draft' && (
                <CreateTaskAction projectKey={projectKey} variant="dashed" className="m-2">
                  {t('board.create')}
                </CreateTaskAction>
              )}
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
  const [searchParams, setSearchParams] = useSearchParams()
  const key = projectKey ?? ''
  const { data: board, isLoading, error } = useBoard(key)
  if (isLoading) return <LoadingState message={t('issue.loading')} />
  if (error || !board) return <ErrorState message={error?.message ?? t('issue.notFound')} />

  const { issues, sprint } = board
  const density = searchParams.get('density') === 'compact' ? 'compact' : 'comfortable'

  function setDensity(nextDensity: 'comfortable' | 'compact') {
    const next = new URLSearchParams(searchParams)
    if (nextDensity === 'compact') next.set('density', 'compact')
    else next.delete('density')
    setSearchParams(next, { replace: true })
  }

  const header = (
    <div className="mb-4">
      <PageHeader
        title={t('board.title', {
          projectName: key,
          sprintName: sprint?.name ?? t('board.backlog'),
        })}
        leading={<ProjectAvatar projectKey={key ?? ''} />}
        context={t('board.subtitle', {
          backlog: board.backlog_total,
          remainingDays: sprint?.remaining_days ?? t('board.notAvailable'),
        })}
        actions={
          <>
            <div className="hidden md:block">
              <RadioGroup
                appearance="segmented"
                label={t('board.density')}
                value={density}
                onValueChange={(value) => setDensity(value as 'comfortable' | 'compact')}
                options={(['comfortable', 'compact'] as const).map((value) => ({
                  value,
                  label: t(`board.density${value === 'compact' ? 'Compact' : 'Comfortable'}`),
                }))}
              />
            </div>
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
          </>
        }
      />
    </div>
  )

  return (
    <div className="min-w-0">
      {header}
      <SdlcBoard projectKey={key} sdlc={board.sdlc} issues={issues} compact={density === 'compact'} />
    </div>
  )
}
