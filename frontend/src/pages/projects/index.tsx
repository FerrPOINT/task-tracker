import { Link } from 'react-router'
import { Plus, Search, Pencil, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useState } from 'react'
import { Button } from '@/shared/ui/button'
import { ErrorState, LoadingState } from '@/shared/ui/async-states'
import { Input } from '@/shared/ui/input'
import { Card, CardContent } from '@/shared/ui/card'
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
} from '@/shared/ui/alert-dialog'
import type { Project } from '@/api/project'

function ProjectAvatar({ projectKey }: { projectKey: string }) {
  const colors = ['bg-accent', 'bg-emerald-500', 'bg-amber-500', 'bg-rose-500']
  const color = colors[projectKey.charCodeAt(0) % colors.length]
  return (
    <div
      className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-md text-sm font-bold text-white sm:h-12 sm:w-12 ${color}`}
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
            update.mutate(values as import('@/api/project').UpdateProjectRequest)
            if (!update.isPending) setFormOpen(false)
          } else {
            create.mutate(values as import('@/api/project').CreateProjectRequest)
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
                  remove.mutate(deletingProject.key)
                  setDeletingProject(null)
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

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {projects
          ?.filter((project) =>
            search
              ? `${project.name} ${project.key}`.toLowerCase().includes(search.toLowerCase())
              : true,
          )
          .map((project) => (
            <Link key={project.id} to={`/projects/${project.key}/board`}>
              <Card className="group transition-colors hover:border-border-strong">
                <CardContent className="p-4">
                  <div className="mb-3 flex items-start justify-between gap-3">
                    <div className="flex min-w-0 items-center gap-3">
                      <ProjectAvatar projectKey={project.key} />
                      <div className="min-w-0">
                        <div className="truncate font-semibold">{project.name}</div>
                        <div className="text-xs text-text-muted">
                          {project.key} · {t('projects.lead')}: {project.owner_name || project.owner_id} ·{' '}
                          {project.todo_count + project.in_progress_count + project.done_count}{' '}
                          {t('projects.issues', {
                            count:
                              project.todo_count + project.in_progress_count + project.done_count,
                          })}
                        </div>
                      </div>
                    </div>
                    <div className="flex shrink-0 items-center gap-1">
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7 opacity-0 group-hover:opacity-100"
                        onClick={(e) => {
                          e.preventDefault()
                          setEditingProject(project)
                          setFormOpen(true)
                        }}
                        aria-label={t('common.edit')}
                      >
                        <Pencil className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7 opacity-0 group-hover:opacity-100"
                        onClick={(e) => {
                          e.preventDefault()
                          setDeletingProject(project)
                        }}
                        aria-label={t('common.delete')}
                      >
                        <Trash2 className="h-4 w-4 text-rose-500" />
                      </Button>
                    </div>
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
            </Link>
          ))}
      </div>
    </div>
  )
}
