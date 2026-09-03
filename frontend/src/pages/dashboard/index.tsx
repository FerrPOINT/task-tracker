import { Link } from 'react-router'
import { useTranslation } from 'react-i18next'
import { Button } from '@sdlc/ui/ui'
import { ErrorState } from '@sdlc/ui/ui'
import { Card, CardContent, CardHeader, CardTitle } from '@sdlc/ui/ui'
import { useDashboard, useProjects } from '@/shared/api/hooks'

export function DashboardPage() {
  const { t } = useTranslation()
  const { data: dashboard, isLoading: dashboardLoading, error: dashboardError } = useDashboard()
  const { data: projects, isLoading: projectsLoading } = useProjects()

  if (dashboardLoading || projectsLoading)
    return <div className="p-4 text-text-muted">{t('issue.loading')}</div>
  if (dashboardError) return <ErrorState message={dashboardError.message} />

  const assigned = dashboard?.assigned_issues ?? []

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h1 className="text-xl font-bold sm:text-2xl">{t('dashboard.title')}</h1>
        <Button size="sm" className="gap-1" asChild>
          <Link to="/issues/create">{t('navigation.create')}</Link>
        </Button>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <Card className="sm:col-span-2 lg:col-span-1">
          <CardHeader className="pb-2">
            <CardTitle className="text-base">{t('dashboard.assignedToMe')}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {assigned.length === 0 && (
              <p className="text-sm text-text-muted">{t('dashboard.noAssigned')}</p>
            )}
            {assigned.map((item) => (
              <Link
                key={item.id}
                to={`/issues/${item.id}`}
                className="flex flex-col gap-1 text-sm hover:text-accent sm:flex-row sm:items-center sm:justify-between"
              >
                <span className="min-w-0 truncate">
                  {item.key} {item.summary}
                </span>
                <span className="shrink-0 self-start rounded bg-surface-raised px-2 py-0.5 text-xs text-text-secondary">
                  {item.status}
                </span>
              </Link>
            ))}
          </CardContent>
        </Card>

        <Card className="sm:col-span-2">
          <CardHeader className="pb-2">
            <CardTitle className="text-base">
              {t('dashboard.projects')} · {projects?.length ?? 0}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {projects?.length === 0 && (
              <p className="text-sm text-text-muted">{t('dashboard.noProjects')}</p>
            )}
            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {projects?.map((project) => (
                <Link
                  key={project.id}
                  to={`/projects/${project.key}/board`}
                  className="rounded-md border border-border p-3 hover:bg-surface-raised"
                >
                  <div className="mb-1 text-sm font-medium text-text-primary">
                    {project.key} · {project.name}
                  </div>
                  <div className="flex gap-2 text-xs text-text-muted">
                    <span className="rounded bg-surface-raised px-1.5 py-0.5">
                      {t('board.todo')}: {project.todo_count ?? 0}
                    </span>
                    <span className="rounded bg-surface-raised px-1.5 py-0.5">
                      {t('board.inProgress')}: {project.in_progress_count ?? 0}
                    </span>
                    <span className="rounded bg-surface-raised px-1.5 py-0.5">
                      {t('board.done')}: {project.done_count ?? 0}
                    </span>
                  </div>
                </Link>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
