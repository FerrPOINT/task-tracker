import { Link } from 'react-router'
import { Plus, Search, LayoutGrid, List, Star, MoreHorizontal } from 'lucide-react'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Card, CardContent } from '@/shared/ui/card'
import { useProjects } from '@/shared/api/hooks'

function ProjectAvatar({ projectKey }: { projectKey: string }) {
  const colors = ['bg-accent', 'bg-emerald-500', 'bg-amber-500', 'bg-rose-500']
  const color = colors[projectKey.charCodeAt(0) % colors.length]
  return (
    <div className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-md text-sm font-bold text-white sm:h-12 sm:w-12 ${color}`}>
      {projectKey.slice(0, 2).toUpperCase()}
    </div>
  )
}

export function ProjectsPage() {
  const { data: projects, isLoading, error } = useProjects()

  if (isLoading) return <div className="p-4 text-text-muted">Loading projects…</div>
  if (error) return <div className="p-4 text-rose-500">{error.message}</div>

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h1 className="text-xl font-bold sm:text-2xl">Проекты</h1>
        <Button size="sm" className="gap-1">
          <Plus className="h-4 w-4" />
          <span className="hidden sm:inline">Создать проект</span>
          <span className="sm:hidden">Создать</span>
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-3">
        <select className="h-9 rounded-md border border-border-strong bg-surface-raised px-2 text-sm text-text-primary">
          <option>Все проекты</option>
          <option>Активные</option>
          <option>Архивные</option>
        </select>
        <select className="h-9 rounded-md border border-border-strong bg-surface-raised px-2 text-sm text-text-primary">
          <option>Любой тип</option>
          <option>Scrum</option>
          <option>Kanban</option>
          <option>Basic</option>
        </select>
        <div className="relative flex-1 basis-full sm:basis-auto">
          <Search className="absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-text-muted" />
          <Input type="text" placeholder="Поиск проектов..." className="h-9 w-full pl-9 sm:w-64"></Input>
        </div>
        <div className="ml-auto flex items-center gap-1">
          <Button variant="secondary" size="icon" className="h-8 w-8">
            <LayoutGrid className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="icon" className="h-8 w-8">
            <List className="h-4 w-4" />
          </Button>
        </div>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {projects?.map((project) => (
          <Link key={project.id} to={`/projects/${project.key}/board`}>
            <Card className="group transition-colors hover:border-border-strong">
              <CardContent className="p-4">
                <div className="mb-3 flex items-start justify-between gap-3">
                  <div className="flex min-w-0 items-center gap-3">
                    <ProjectAvatar projectKey={project.key} />
                    <div className="min-w-0">
                      <div className="truncate font-semibold">{project.name}</div>
                      <div className="text-xs text-text-muted">{project.key} · Lead: {project.owner_id} · {project.todo_count + project.in_progress_count + project.done_count} issues</div>
                    </div>
                  </div>
                  <div className="hidden shrink-0 items-center gap-1 sm:flex">
                    <Button variant="ghost" size="icon" className="h-7 w-7 opacity-0 group-hover:opacity-100">
                      <Star className="h-4 w-4" />
                    </Button>
                    <Button variant="ghost" size="icon" className="h-7 w-7 opacity-0 group-hover:opacity-100">
                      <MoreHorizontal className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
                <div className="grid grid-cols-3 gap-2 text-center text-xs sm:text-sm">
                  <div className="rounded bg-surface-raised py-1">
                    <div className="text-text-muted">Todo</div>
                    <div className="font-medium">{project.todo_count}</div>
                  </div>
                  <div className="rounded bg-surface-raised py-1">
                    <div className="text-text-muted">In Progress</div>
                    <div className="font-medium">{project.in_progress_count}</div>
                  </div>
                  <div className="rounded bg-surface-raised py-1">
                    <div className="text-text-muted">Done</div>
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
