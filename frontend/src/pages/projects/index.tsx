import { Link } from 'react-router'
import { Plus, Search, Pencil, Trash2, MoreHorizontal } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useState } from 'react'
import { Button } from '@sdlc/ui/ui'
import { ErrorState, LoadingState } from '@sdlc/ui/ui'
import { Input } from '@sdlc/ui/ui'
import { Card, CardContent } from '@sdlc/ui/ui'
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
      className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-md text-sm font-bold sm:h-12 sm:w-12 ${color}`}
    >
      {projectKey.slice(0, 2).toUpperCase()}
    </div>
  )
}

export function ProjectsPage() {
  const { t } = useTranslation()
  const { data: projects, isLoading, error } = useProjects()
  const [formOpen, setFormOpen] = useState(false)
  const [editingProject, setEditingProject] = useState<Project | null>(null)
  const [deletingProject, setDeletingProject] = useState<Project | null>(null)
  const [search, setSearch] = useState('')

  const create = useCreateProject()
  const update = useUpdateProject(editingProject?.key ?? '')
  const remove = useDeleteProject()

  if (isLoading) return <LoadingState message={t('issue.loading')} />
  if (error) return <ErrorState message={error.message} />

  const isFormPending = create.isPending || update.isPending
  const formError = create.error ?? update.error

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
        onOpenChange={(open) => !open && setDeletingProject(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('projects.deleteTitle')}</AlertDialogTitle>
            <AlertDialogDescription>
              {t('projects.deleteDescription', { name: deletingProject?.name })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div className="flex justify-end gap-2 pt-2">
            <AlertDialogCancel onClick={() => setDeletingProject(null)}>
              {t('common.cancel')}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (deletingProject) {
                  remove.mutate(deletingProject.key, {
                    onSuccess: () => setDeletingProject(null),
                  })
                }
              }}
              disabled={remove.isPending}
            >
              {t('common.delete')}
            </AlertDialogAction>
          </div>
        </AlertDialogContent>
      </AlertDialog>

      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h1 className="text-xl font-bold sm:text-2xl">{t('projects.title')}</h1>
        <Button size="sm" className="gap-1" onClick={() => setFormOpen(true)}>
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
          Проектов пока нет. Создайте первый проект, чтобы начать работу.
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {projects
            ?.filter((project) =>
              search
                ? `${project.name} ${project.key}`.toLowerCase().includes(search.toLowerCase())
                : true,
            )
            .map((project) => (
              <Card key={project.id} className="transition-colors hover:border-border-strong">
                <CardContent className="p-4">
                  <div className="mb-3 flex items-start justify-between gap-3">
                    <div className="flex min-w-0 items-center gap-3">
                      <ProjectAvatar projectKey={project.key} />
                      <div className="min-w-0">
                        <Link
                          to={`/projects/${project.key}/board`}
                          className="block truncate font-semibold hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
                        >
                          {project.name}
                        </Link>
                        <div className="text-xs text-text-muted">
                          {project.key} · {t('projects.lead')}:{' '}
                          {project.owner_name || project.owner_id} ·{' '}
                          {project.todo_count + project.in_progress_count + project.done_count}{' '}
                          {t('projects.issues', {
                            count:
                              project.todo_count + project.in_progress_count + project.done_count,
                          })}
                        </div>
                      </div>
                    </div>
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button
                          variant="ghost"
                          size="icon"
                          className="h-9 w-9 shrink-0"
                          aria-label={t('projects.moreActions')}
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
                          onClick={() => setDeletingProject(project)}
                          className="gap-2 text-danger"
                        >
                          <Trash2 className="h-4 w-4" />
                          {t('common.delete')}
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                  <div className="grid grid-cols-3 gap-2 text-center text-xs sm:text-sm">
                    <div className="rounded bg-surface-raised py-1">
                      <div className="text-text-muted">{t('projects.todo')}</div>
                      <div className="font-medium">{project.todo_count}</div>
                    </div>
                    <div className="rounded bg-surface-raised py-1">
                      <div className="text-text-muted">{t('projects.inProgress')}</div>
                      <div className="font-medium">{project.in_progress_count}</div>
                    </div>
                    <div className="rounded bg-surface-raised py-1">
                      <div className="text-text-muted">{t('projects.done')}</div>
                      <div className="font-medium text-emerald-500">{project.done_count}</div>
                    </div>
                  </div>
                </CardContent>
              </Card>
            ))}
          {projects &&
            projects.length > 0 &&
            projects.filter((project) =>
              search
                ? `${project.name} ${project.key}`.toLowerCase().includes(search.toLowerCase())
                : true,
            ).length === 0 && (
              <div className="col-span-full rounded-md border border-dashed border-border p-10 text-center text-sm text-text-muted">
                По вашему запросу проекты не найдены.
              </div>
            )}
        </div>
      )}
    </div>
  )
}
