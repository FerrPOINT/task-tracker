import { useActiveProjects as useProjects } from '@/widgets/active-projects'
import { CreateProjectInSpacesDialog } from '@/features/projects/ui/CreateProjectInSpacesDialog'
import {
  ProjectCardActionItems,
  ProjectCardLink,
  ProjectCardState,
} from '@/widgets/project-actions'
import {
  ProjectAvatar,
  ResourceList,
  ResourceListRow,
  Button,
  ErrorState,
  LoadingState,
  Input,
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
} from '@pdlc/ui/ui'

import { Plus, Search, MoreHorizontal } from 'lucide-react'
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
      <div className="flex gap-2">
        <Button
          variant="outline"
          disabled={offset === 0}
          onClick={() => setOffset(Math.max(0, offset - 50))}
        >
          Назад
        </Button>
        <Button
          variant="outline"
          disabled={spaceEnabled ? !catalog?.has_more : unfilteredCount < 50}
          onClick={() => setOffset(offset + 50)}
        >
          Далее
        </Button>
      </div>
      {import.meta.env.VITE_SPACE_ACCESS_ENABLED === 'true' ? (
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
      )}

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
            <p role="alert" className="text-sm text-danger">
              {t('projects.deleteError')}
            </p>
          )}
          <div className="flex justify-end gap-2 pt-2">
            <AlertDialogCancel disabled={remove.isPending}>{t('common.cancel')}</AlertDialogCancel>
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
          </div>
        </AlertDialogContent>
      </AlertDialog>

      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-xl font-bold sm:text-2xl">{t('projects.title')}</h1>
        <Button size="sm" className="min-h-10 gap-1" onClick={() => setFormOpen(true)}>
          <Plus className="h-4 w-4" />
          <span className="hidden sm:inline">{t('projects.create')}</span>
          <span className="sm:hidden">{t('navigation.create')}</span>
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-3">
        <div className="relative flex-1 basis-full sm:basis-auto">
          <Search className="absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-text-muted" />
          <Input
            type="text"
            aria-label={t('projects.search')}
            placeholder={t('projects.search')}
            className="h-9 w-full pl-9 sm:w-64"
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
        <div className="rounded-md border border-dashed border-border p-10 text-center text-sm text-text-muted">
          {t('projects.empty')}
        </div>
      ) : filteredProjects?.length === 0 ? (
        <div className="rounded-md border border-dashed border-border p-10 text-center text-sm text-text-muted">
          {t('projects.noMatches')}
        </div>
      ) : (
        <ResourceList asChild variant="bordered">
          <ul>
            {filteredProjects?.map((project) => (
              <ResourceListRow asChild key={project.id}>
                <li className="grid grid-cols-[minmax(0,1fr)_2.5rem] items-center gap-x-3 gap-y-1 sm:grid-cols-[minmax(0,1fr)_16rem_2.5rem]">
                  <ProjectCardLink
                    project={project}
                    className="row-start-1 flex min-h-10 min-w-0 items-center gap-3 rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                  >
                    <ProjectAvatar projectKey={project.key} size="md" />
                    <span className="min-w-0">
                      <span
                        className="line-clamp-2 font-semibold hover:text-accent hover:underline sm:line-clamp-1"
                        title={project.name}
                      >
                        {project.name}
                      </span>
                      <span className="block truncate text-xs text-text-muted">
                        {project.key} · {t('projects.lead')}:{' '}
                        {project.owner_name || project.owner_id}
                      </span>
                      <ProjectCardState projectId={project.id} />
                    </span>
                  </ProjectCardLink>
                  <div className="col-span-2 col-start-1 row-start-2 grid grid-cols-3 gap-1 text-center text-xs sm:col-span-1 sm:col-start-2 sm:row-start-1">
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
                  </div>
                  <div className="col-start-2 row-start-1 sm:col-start-3">
                    <DropdownMenu modal={false}>
                      <DropdownMenuTrigger asChild>
                        <Button
                          variant="ghost"
                          size="icon"
                          className="h-10 w-10"
                          aria-label={t('projects.moreActionsFor', { name: project.name })}
                        >
                          <MoreHorizontal className="h-4 w-4" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <ProjectCardActionItems
                          project={project}
                          onDelete={() => {
                            remove.reset()
                            setDeletingProject(project)
                          }}
                        />
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                </li>
              </ResourceListRow>
            ))}
          </ul>
        </ResourceList>
      )}
    </div>
  )
}
