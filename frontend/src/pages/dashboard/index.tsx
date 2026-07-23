import { Link } from 'react-router'
import { Plus, Clock } from 'lucide-react'
import { Button } from '@/shared/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/shared/ui/card'
import { useDashboard, useProjects } from '@/shared/api/hooks'

function PriorityBadge({ priority }: { priority: string }) {
  const color =
    priority === 'High'
      ? 'text-rose-500'
      : priority === 'Medium'
        ? 'text-amber-500'
        : 'text-emerald-500'
  return <span className={`text-xs font-medium ${color}`}>{priority}</span>
}

export function DashboardPage() {
  const { data: dashboard, isLoading: dashboardLoading, error: dashboardError } = useDashboard()
  const { data: projects, isLoading: projectsLoading } = useProjects()

  if (dashboardLoading || projectsLoading) return <div className="p-4 text-text-muted">Loading dashboard…</div>
  if (dashboardError) return <div className="p-4 text-rose-500">{dashboardError.message}</div>

  const assigned = dashboard?.assigned_issues ?? []

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h1 className="text-xl font-bold sm:text-2xl">Team Dashboard</h1>
        <Button size="sm" className="gap-1">
          <Plus className="h-4 w-4" />
          <span className="hidden sm:inline">Добавить виджет</span>
          <span className="sm:hidden">Виджет</span>
        </Button>
      </div>

      <div className="grid gap-4 lg:grid-cols-3">
        <Card className="lg:col-span-2">
          <CardHeader className="pb-2">
            <CardTitle className="text-base">Sprint Burndown</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex h-40 items-end gap-1 sm:gap-3">
              {[40, 35, 30, 28, 22, 15, 10].map((v, i) => (
                <div key={i} className="flex flex-1 flex-col items-center gap-1">
                  <div
                    className="w-full rounded bg-accent"
                    style={{ height: `${(v / 40) * 100}%` }}
                  />
                  <span className="text-[10px] text-text-muted sm:text-xs">Day {i + 1}</span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">Open Bugs</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-end gap-2">
              <span className="text-4xl font-bold text-rose-500">7</span>
              <span className="mb-1 text-xs text-text-muted">+2 за эту неделю</span>
            </div>
            <div className="space-y-2">
              <Link to="/issues/bug-14" className="flex items-center justify-between gap-2 rounded-md border border-border p-2 hover:bg-surface-raised">
                <span className="min-w-0 truncate text-sm">TT-14 UI glitch on mobile</span>
                <PriorityBadge priority="High" />
              </Link>
              <Link to="/issues/bug-9" className="flex items-center justify-between gap-2 rounded-md border border-border p-2 hover:bg-surface-raised">
                <span className="min-w-0 truncate text-sm">TT-9 Auth token refresh</span>
                <PriorityBadge priority="Medium" />
              </Link>
            </div>
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">Velocity</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-end gap-2">
              <span className="text-3xl font-bold sm:text-4xl">24 sp</span>
              <span className="mb-1 text-xs text-text-muted">Средняя за 3 спринта</span>
            </div>
            <div className="flex h-6 gap-1">
              <div className="flex-1 rounded bg-surface-raised" />
              <div className="flex-1 rounded bg-accent" />
              <div className="flex-1 rounded bg-emerald-500" />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">Assigned to me</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {assigned.map((item) => (
              <Link
                key={item.id}
                to={`/issues/${item.id}`}
                className="flex flex-col gap-1 text-sm hover:text-accent sm:flex-row sm:items-center sm:justify-between"
              >
                <span className="min-w-0 truncate">{item.key} {item.summary}</span>
                <span className="shrink-0 self-start rounded bg-surface-raised px-2 py-0.5 text-xs text-text-secondary">{item.status}</span>
              </Link>
            ))}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">Recent activity · {projects?.length ?? 0} projects</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {[
              { user: 'Ivan', hours: 2, issueKey: 'TT-42', when: '24ч назад' },
              { user: 'Anna', hours: 1.5, issueKey: 'TT-11', when: '3ч назад' },
              { user: 'Petr', hours: 3, issueKey: 'TT-15', when: '5ч назад' },
            ].map((entry, i) => (
              <div key={i} className="flex items-start gap-2 text-sm">
                <Clock className="mt-0.5 h-4 w-4 shrink-0 text-text-muted" />
                <div className="min-w-0">
                  <div className="truncate">{entry.user} logged {entry.hours}h on {entry.issueKey}</div>
                  <div className="text-xs text-text-muted">{entry.when}</div>
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
