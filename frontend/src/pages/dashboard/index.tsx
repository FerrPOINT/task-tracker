import { Link } from 'react-router'
import { useTranslation } from 'react-i18next'
import { Button } from '@sdlc/ui/ui'
import { ErrorState } from '@sdlc/ui/ui'
import { useDashboard, useProjects } from '@/shared/api/hooks'
import { LoadingState } from '@sdlc/ui/ui'
import { statusLabel } from '@/shared/lib/status-label'

export function DashboardPage() {
  const { t } = useTranslation()
  const { data: dashboard, isLoading: dashboardLoading, error: dashboardError } = useDashboard()
  const { data: projects, isLoading: projectsLoading } = useProjects()

  if (dashboardLoading || projectsLoading) return <LoadingState message={t('issue.loading')} />
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

      <div className="grid gap-8 lg:grid-cols-[minmax(0,1fr)_minmax(0,2fr)]">
        <section className="min-w-0 border-t border-border pt-4">
          <h2 className="mb-3 text-sm font-semibold">{t('dashboard.assignedToMe')}</h2>
          <div className="space-y-3">
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
                  {statusLabel(item.status, t)}
                </span>
              </Link>
            ))}
          </div>
        </section>

        <section className="min-w-0 border-t border-border pt-4">
          <h2 className="mb-3 text-sm font-semibold">
            {t('dashboard.projects')} · {projects?.length ?? 0}
          </h2>
          <div className="space-y-3">
            {projects?.length === 0 && (
              <p className="text-sm text-text-muted">{t('dashboard.noProjects')}</p>
            )}
            <div className="divide-y divide-border rounded-md border border-border">
              {projects?.map((project) => (
                <Link
                  key={project.id}
                  to={`/projects/${project.key}/board`}
                  className="block p-3 hover:bg-surface-raised"
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
          </div>
        </section>
      </div>
    </div>
  )
}
