import { Link } from 'react-router'
import { Plus, Search, Pencil, Trash2, MoreHorizontal } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useState } from 'react'
import { Button } from '@sdlc/ui/ui'
import { ErrorState, LoadingState } from '@sdlc/ui/ui'
import { Input } from '@sdlc/ui/ui'
import {
  useProjects,
  useCreateProject,
  useUpdateProject,
  useDeleteProject,
} from '@/shared/api/hooks'
import { ProjectFormDialog } from '@/features/projects/ui/ProjectFormDialog'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@sdlc/ui/ui'
import type { Project } from '@/api/project'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@sdlc/ui/ui'

function ProjectAvatar({ projectKey }: { projectKey: string }) {
  const colors = [
    'bg-accent text-accent-foreground',
    'bg-emerald-500 text-zinc-950',
    'bg-amber-500 text-zinc-950',
    'bg-rose-500 text-zinc-950',
  ]
  const color = colors[projectKey.charCodeAt(0) % colors.length]
  return (
    <div
      className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-md text-sm font-bold ${color}`}
    >
      {projectKey.slice(0, 2).toUpperCase()}
    </div>
  )
}

export function ProjectsPage() {
  const { t } = useTranslation()
  const { data: projects, isLoading, error, refetch } = useProjects()
  const [formOpen, setFormOpen] = useState(false)
  const [editingProject, setEditingProject] = useState<Project | null>(null)
  const [deletingProject, setDeletingProject] = useState<Project | null>(null)
  const [search, setSearch] = useState('')

  const create = useCreateProject()
  const update = useUpdateProject(editingProject?.key ?? '')
  const remove = useDeleteProject()

  if (isLoading) return <LoadingState message={t('issue.loading')} />
  if (error) return <ErrorState message={error.message} onRetry={() => void refetch()} />

  const isFormPending = create.isPending || update.isPending
  const formError = create.error ?? update.error
  const normalizedSearch = search.trim().toLocaleLowerCase()
  const filteredProjects = projects?.filter((project) =>
    `${project.name} ${project.key}`.toLocaleLowerCase().includes(normalizedSearch),
  )

  return (
    <div className="space-y-4">
      <ProjectFormDialog
        open={formOpen}
        project={editingProject}
        onOpenChange={(open) => {
          setFormOpen(open)
          if (!open) setEditingProject(null)
        }}
        onSubmit={(values) => {
          if (editingProject) {
            update.mutate(values as import('@/api/project').UpdateProjectRequest, {
              onSuccess: () => setFormOpen(false),
            })
          } else {
            create.mutate(values as import('@/api/project').CreateProjectRequest, {
              onSuccess: () => setFormOpen(false),
            })
          }
        }}
        isPending={isFormPending}
        error={formError as Error | null}
      />

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
            onChange={(e) => setSearch(e.target.value)}
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
        <ul className="divide-y divide-border rounded-md border border-border bg-surface">
          {filteredProjects?.map((project) => (
            <li
              key={project.id}
              className="grid grid-cols-[minmax(0,1fr)_2.5rem] items-center gap-x-3 gap-y-1 px-3 py-2 hover:bg-surface-raised sm:grid-cols-[minmax(0,1fr)_16rem_2.5rem]"
            >
              <Link
                to={`/projects/${project.key}/board`}
                className="row-start-1 flex min-h-10 min-w-0 items-center gap-3 rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
              >
                <ProjectAvatar projectKey={project.key} />
                <span className="min-w-0">
                  <span
                    className="line-clamp-2 font-semibold hover:text-accent hover:underline sm:line-clamp-1"
                    title={project.name}
                  >
                    {project.name}
                  </span>
                  <span className="block truncate text-xs text-text-muted">
                    {project.key} · {t('projects.lead')}: {project.owner_name || project.owner_id}
                  </span>
                </span>
              </Link>
              <div className="col-span-2 col-start-1 row-start-2 grid grid-cols-3 gap-1 text-center text-xs sm:col-span-1 sm:col-start-2 sm:row-start-1">
                <div className="min-w-0">
                  <span className="block truncate text-text-muted">{t('projects.todo')}</span>
                  <span className="font-medium">{project.todo_count}</span>
                </div>
                <div className="min-w-0">
                  <span className="block truncate text-text-muted">{t('projects.inProgress')}</span>
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
                    <DropdownMenuItem
                      onClick={() => {
                        setEditingProject(project)
                        setFormOpen(true)
                      }}
                      className="gap-2"
                    >
                      <Pencil className="h-4 w-4" />
                      {t('common.edit')}
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onClick={() => {
                        remove.reset()
                        setDeletingProject(project)
                      }}
                      className="gap-2 text-danger"
                    >
                      <Trash2 className="h-4 w-4" />
                      {t('common.delete')}
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
