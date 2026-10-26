import { useApplicationAccess } from '@pdlc/ui/auth'
import { useActiveProjects as useProjects } from '@/widgets/active-projects'
import { CreateProjectInSpacesDialog } from '@/features/projects/ui/CreateProjectInSpacesDialog'
import {
  ProjectCardActionItems,
  ProjectCardLink,
  ProjectCardState,
} from '@/widgets/project-actions'
import {
  ActionGroup,
  PageNavigation,
  EmptyState,
  PageHeader,
  ProjectAvatar,
  ResourceList,
  ResourceListRow,
  Button,
  ErrorState,
  LoadingState,
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogHeader,
  AlertDialogTitle,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
  SearchInput,
  Notice,
} from '@pdlc/ui/ui'

import { Plus, MoreHorizontal } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useState } from 'react'

import { useCreateProject, useDeleteProject } from '@/shared/api/hooks'
import { ProjectFormDialog } from '@/features/projects/ui/ProjectFormDialog'

import type { Project } from '@/api/project'

export function ProjectsPage() {
  const { t } = useTranslation()
  const [offset, setOffset] = useState(0)
  const [search, setSearch] = useState('')
  const spaceEnabled = import.meta.env.VITE_SPACE_ACCESS_ENABLED === 'true'
  const {
    data: projects,
    isLoading,
    error,
    refetch,
    catalog,
    unfilteredCount,
  } = useProjects(offset, 50, spaceEnabled ? search : '')
  const [formOpen, setFormOpen] = useState(false)
  const [deletingProject, setDeletingProject] = useState<Project | null>(null)

  const canAdmin = useApplicationAccess()?.permits([], true) === true
  const create = useCreateProject()
  const remove = useDeleteProject()

  if (isLoading) return <LoadingState message={t('issue.loading')} />
  if (error) return <ErrorState message={error.message} onRetry={() => void refetch()} />

  const normalizedSearch = search.trim().toLocaleLowerCase()
  const filteredProjects = spaceEnabled
    ? projects
    : projects?.filter((project) =>
        `${project.name} ${project.key}`.toLocaleLowerCase().includes(normalizedSearch),
      )

  return (
    <div className="space-y-4">
      <PageNavigation
        page={Math.ceil(offset / 50) + 1}
        hasNext={spaceEnabled ? !!catalog?.has_more : unfilteredCount >= 50}
        onPageChange={(nextPage) =>
          setOffset(nextPage < Math.ceil(offset / 50) + 1 ? Math.max(0, offset - 50) : offset + 50)
        }
        label="Страницы проектов"
        previousLabel="Назад"
        nextLabel="Далее"
      />
      {canAdmin &&
        (import.meta.env.VITE_SPACE_ACCESS_ENABLED === 'true' ? (
          <CreateProjectInSpacesDialog open={formOpen} onOpenChange={setFormOpen} />
        ) : (
          <ProjectFormDialog
            open={formOpen}
            onOpenChange={(open) => {
              setFormOpen(open)
            }}
            onSubmit={(values) => {
              create.mutate(values as import('@/api/project').CreateProjectRequest, {
                onSuccess: () => setFormOpen(false),
              })
            }}
            isPending={create.isPending}
            error={create.error}
          />
        ))}

      <AlertDialog
        open={!!deletingProject}
        onOpenChange={(open) => {
          if (!open && !remove.isPending) {
            setDeletingProject(null)
            remove.reset()
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('projects.deleteTitle')}</AlertDialogTitle>
            <AlertDialogDescription>
              {t('projects.deleteDescription', { name: deletingProject?.name })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          {remove.error && (
            <Notice role="alert" tone="danger" appearance="inline">
              {t('projects.deleteError')}
            </Notice>
          )}
          <div className="pt-2">
            <ActionGroup align="end">
              <AlertDialogCancel disabled={remove.isPending}>
                {t('common.cancel')}
              </AlertDialogCancel>
              <AlertDialogAction
                onClick={(event) => {
                  event.preventDefault()
                  if (deletingProject) {
                    remove.mutate(deletingProject.key, {
                      onSuccess: () => setDeletingProject(null),
                    })
                  }
                }}
                disabled={remove.isPending}
              >
                {remove.isPending ? t('common.loading') : t('common.delete')}
              </AlertDialogAction>
            </ActionGroup>
          </div>
        </AlertDialogContent>
      </AlertDialog>

      <PageHeader
        title={t('projects.title')}
        actions={
          canAdmin && (
            <Button size="sm" onClick={() => setFormOpen(true)}>
              <Plus className="h-4 w-4" />
              <span className="hidden sm:inline">{t('projects.create')}</span>
              <span className="sm:hidden">{t('navigation.create')}</span>
            </Button>
          )
        }
      />

      <div className="flex flex-wrap items-center gap-3">
        <div className="flex-1 basis-full sm:basis-auto">
          <SearchInput
            aria-label={t('projects.search')}
            placeholder={t('projects.search')}
            className="w-full sm:w-64"
            value={search}
            onChange={(e) => {
              setSearch(
                spaceEnabled ? Array.from(e.target.value).slice(0, 200).join('') : e.target.value,
              )
              if (spaceEnabled) setOffset(0)
            }}
          />
        </div>
      </div>

      {projects?.length === 0 ? (
        <EmptyState message={t('projects.empty')} />
      ) : filteredProjects?.length === 0 ? (
        <EmptyState message={t('projects.noMatches')} />
      ) : (
        <ResourceList asChild variant="bordered">
          <ul>
            {filteredProjects?.map((project) => (
              <li
                key={project.id}
                className="grid grid-cols-[minmax(0,1fr)_auto] items-stretch sm:grid-cols-[minmax(0,1fr)_16rem_auto]"
              >
                <ProjectCardLink
                  project={project}
                  className="row-start-1 flex min-w-0 items-center gap-3"
                >
                  <ProjectAvatar projectKey={project.key} size="md" />
                  <span className="min-w-0">
                    <span
                      className="line-clamp-2 font-semibold sm:line-clamp-1"
                      title={project.name}
                    >
                      {project.name}
                    </span>
                    <span className="block truncate text-xs text-text-muted">
                      {project.key} · {t('projects.lead')}: {project.owner_name || project.owner_id}
                    </span>
                    <ProjectCardState projectId={project.id} />
                  </span>
                </ProjectCardLink>
                <ResourceListRow className="col-span-2 col-start-1 row-start-2 grid grid-cols-3 items-center gap-1 text-center text-xs sm:col-span-1 sm:col-start-2 sm:row-start-1">
                  <div className="min-w-0">
                    <span className="block truncate text-text-muted">{t('projects.todo')}</span>
                    <span className="font-medium">{project.todo_count}</span>
                  </div>
                  <div className="min-w-0">
                    <span className="block truncate text-text-muted">
                      {t('projects.inProgress')}
                    </span>
                    <span className="font-medium">{project.in_progress_count}</span>
                  </div>
                  <div className="min-w-0">
                    <span className="block truncate text-text-muted">{t('projects.done')}</span>
                    <span className="font-medium">{project.done_count}</span>
                  </div>
                </ResourceListRow>
                <ResourceListRow className="col-start-2 row-start-1 flex items-center sm:col-start-3">
                  <DropdownMenu modal={false}>
                    <DropdownMenuTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon"

                        aria-label={t('projects.moreActionsFor', { name: project.name })}
                      >
                        <MoreHorizontal className="h-4 w-4" />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <ProjectCardActionItems
                        project={project}
                        onDelete={
                          canAdmin
                            ? () => {
                                remove.reset()
                                setDeletingProject(project)
                              }
                            : undefined
                        }
                      />
                    </DropdownMenuContent>
                  </DropdownMenu>
                </ResourceListRow>
              </li>
            ))}
          </ul>
        </ResourceList>
      )}
    </div>
  )
}
