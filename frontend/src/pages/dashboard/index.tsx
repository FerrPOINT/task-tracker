import { ContentSection } from '@pdlc/ui/ui'
import { useActiveProjects as useProjects } from '@/widgets/active-projects'
import { CreateProjectAction } from '@/features/projects/ui/CreateProjectAction'
import {
  ResourceListDescription,
  LoadingState,
  Badge,
  Button,
  PageHeader,
  SectionHeader,
  ProjectAvatar,
  ResourceList,
  ResourceListRow,
  EmptyState,
  ErrorState,
} from '@pdlc/ui/ui'
import { ProjectLink as Link } from '@/shared/lib/project-navigation'

import { useTranslation } from 'react-i18next'

import { useDashboard } from '@/shared/api/hooks'
import { statusLabel } from '@/shared/lib/status-label'
import { useAuthStore } from '@/shared/auth/store'
import { usePlatformReadiness } from '@pdlc/ui/platform'

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
  usePlatformReadiness({
    pending: dashboardLoading || projectsLoading,
    error: dashboardError || projectsError ? 'Не удалось загрузить дашборд Tracker.' : null,
  })

  const assigned = dashboard?.assigned_issues ?? []
  const assignedSearch = userId ? `/search?assignee_id=${encodeURIComponent(userId)}` : '/search'

  return (
    <div className="space-y-4">
      <PageHeader title={t('dashboard.title')} />

      <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.5fr)]">
        <ContentSection asChild dividers="start">
          <section className="min-w-0">
            <SectionHeader
              level={2}
              className="mb-3"
              title={<>{t('dashboard.assignedToMe')}</>}
              actions={
                <>
                  {assigned.length > PREVIEW_LIMIT && (
                    <Button asChild variant="link">
                      <Link to={assignedSearch}>{t('dashboard.viewAllAssigned')}</Link>
                    </Button>
                  )}
                </>
              }
            />
            {dashboardLoading ? (
              <LoadingState scope="local" message={t('dashboard.loadingAssigned')} />
            ) : dashboardError ? (
              <ErrorState
                message={t('dashboard.loadError')}
                onRetry={() => void refetchDashboard()}
              />
            ) : assigned.length === 0 ? (
              <EmptyState message={t('dashboard.noAssigned')} />
            ) : (
              <ResourceList asChild variant="bordered">
                <div>
                  {assigned.slice(0, PREVIEW_LIMIT).map((item) => (
                    <ResourceListRow asChild interactive key={item.id}>
                      <Link
                        to={`/issues/${item.id}`}
                        className="grid gap-1.5 text-sm sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center"
                      >
                        <span className="flex min-w-0 flex-col gap-0.5 sm:flex-row sm:items-center sm:gap-2">
                          <ResourceListDescription asChild technical>
                            <span className="shrink-0">{item.key}</span>
                          </ResourceListDescription>
                          <span className="line-clamp-2 break-words sm:line-clamp-1">
                            {item.summary}
                          </span>
                        </span>
                        <Badge tone="muted" className="w-fit">
                          {statusLabel(item.status, t)}
                        </Badge>
                      </Link>
                    </ResourceListRow>
                  ))}
                </div>
              </ResourceList>
            )}
          </section>
        </ContentSection>

        <ContentSection asChild dividers="start">
          <section className="min-w-0">
            <SectionHeader
              level={2}
              className="mb-3"
              title={
                <>
                  {t('dashboard.projects')}
                  {projects ? ` · ${projects.length}` : ''}
                </>
              }
              actions={
                <>
                  {(projects?.length ?? 0) > PREVIEW_LIMIT && (
                    <Button asChild variant="link">
                      <Link to="/projects">{t('projects.all')}</Link>
                    </Button>
                  )}
                </>
              }
            />
            {projectsLoading ? (
              <LoadingState scope="local" message={t('dashboard.loadingProjects')} />
            ) : projectsError ? (
              <ErrorState
                message={t('dashboard.projectsError')}
                onRetry={() => void refetchProjects()}
              />
            ) : !projects?.length ? (
              <EmptyState message={t('dashboard.noProjects')} action={<CreateProjectAction />} />
            ) : (
              <ResourceList asChild variant="bordered">
                <div>
                  {projects.slice(0, PREVIEW_LIMIT).map((project) => (
                    <ResourceListRow asChild interactive key={project.id}>
                      <Link to={`/projects/${project.key}/board`} className="block">
                        <div className="mb-1 flex items-center gap-2 break-words text-sm font-medium text-text-primary">
                          <ProjectAvatar projectKey={project.key} />
                          <span className="min-w-0">
                            {project.key} · {project.name}
                          </span>
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
                    </ResourceListRow>
                  ))}
                </div>
              </ResourceList>
            )}
          </section>
        </ContentSection>
      </div>
    </div>
  )
}
