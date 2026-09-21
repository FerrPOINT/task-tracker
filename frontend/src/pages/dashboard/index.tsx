import { Link } from 'react-router'
import { useTranslation } from 'react-i18next'
import { Button } from '@sdlc/ui/ui'
import { ErrorState } from '@sdlc/ui/ui'
import { useDashboard, useProjects } from '@/shared/api/hooks'
import { statusLabel } from '@/shared/lib/status-label'
import { useAuthStore } from '@/shared/auth/store'

const PREVIEW_LIMIT = 5

export function DashboardPage() {
  const { t } = useTranslation()
  const {
    data: dashboard,
    isLoading: dashboardLoading,
    error: dashboardError,
    refetch: refetchDashboard,
  } = useDashboard()
  const {
    data: projects,
    isLoading: projectsLoading,
    error: projectsError,
    refetch: refetchProjects,
  } = useProjects()
  const userId = useAuthStore((state) => state.userId)

  const assigned = dashboard?.assigned_issues ?? []
  const assignedSearch = userId ? `/search?assignee_id=${encodeURIComponent(userId)}` : '/search'

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h1 className="text-xl font-bold sm:text-2xl">{t('dashboard.title')}</h1>
        <Button size="sm" className="min-h-11 gap-1 sm:min-h-9" asChild>
          <Link to="/issues/create">{t('navigation.create')}</Link>
        </Button>
      </div>

      <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.5fr)]">
        <section className="min-w-0 border-t border-border pt-4">
          <div className="mb-3 flex items-center justify-between gap-3">
            <h2 className="text-sm font-semibold">{t('dashboard.assignedToMe')}</h2>
            {assigned.length > PREVIEW_LIMIT && (
              <Link
                to={assignedSearch}
                className="text-sm text-accent hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
              >
                {t('dashboard.viewAllAssigned')}
              </Link>
            )}
          </div>
          {dashboardLoading ? (
            <p className="text-sm text-text-muted">{t('dashboard.loadingAssigned')}</p>
          ) : dashboardError ? (
            <ErrorState
              message={t('dashboard.loadError')}
              onRetry={() => void refetchDashboard()}
            />
          ) : assigned.length === 0 ? (
            <p className="text-sm text-text-muted">{t('dashboard.noAssigned')}</p>
          ) : (
            <div className="divide-y divide-border rounded-md border border-border">
              {assigned.slice(0, PREVIEW_LIMIT).map((item) => (
                <Link
                  key={item.id}
                  to={`/issues/${item.id}`}
                  className="grid min-h-11 gap-1.5 rounded-sm px-3 py-2 text-sm hover:bg-surface-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center"
                >
                  <span className="flex min-w-0 flex-col gap-0.5 sm:flex-row sm:items-center sm:gap-2">
                    <span className="shrink-0 font-mono text-xs text-text-muted">{item.key}</span>
                    <span className="line-clamp-2 break-words sm:line-clamp-1">{item.summary}</span>
                  </span>
                  <span className="w-fit rounded bg-surface-raised px-2 py-0.5 text-xs text-text-secondary">
                    {statusLabel(item.status, t)}
                  </span>
                </Link>
              ))}
            </div>
          )}
        </section>

        <section className="min-w-0 border-t border-border pt-4">
          <div className="mb-3 flex items-center justify-between gap-3">
            <h2 className="text-sm font-semibold">
              {t('dashboard.projects')}
              {projects ? ` · ${projects.length}` : ''}
            </h2>
            {(projects?.length ?? 0) > PREVIEW_LIMIT && (
              <Link
                to="/projects"
                className="text-sm text-accent hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
              >
                {t('projects.all')}
              </Link>
            )}
          </div>
          {projectsLoading ? (
            <p className="text-sm text-text-muted">{t('dashboard.loadingProjects')}</p>
          ) : projectsError ? (
            <ErrorState
              message={t('dashboard.projectsError')}
              onRetry={() => void refetchProjects()}
            />
          ) : !projects?.length ? (
            <p className="text-sm text-text-muted">{t('dashboard.noProjects')}</p>
          ) : (
            <div className="divide-y divide-border rounded-md border border-border">
              {projects.slice(0, PREVIEW_LIMIT).map((project) => (
                <Link
                  key={project.id}
                  to={`/projects/${project.key}/board`}
                  className="block min-h-11 rounded-sm p-3 hover:bg-surface-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                >
                  <div className="mb-1 break-words text-sm font-medium text-text-primary">
                    {project.key} · {project.name}
                  </div>
                  <div className="flex flex-wrap gap-1.5 text-xs text-text-muted">
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
          )}
        </section>
      </div>
    </div>
  )
}
