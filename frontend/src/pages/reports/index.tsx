import { useEffect, useState } from 'react'
import { useSearchParams } from 'react-router'
import { Download } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import {
  BarChart,
  Bar,
  LineChart,
  Line,
  AreaChart,
  Area,
  ScatterChart,
  Scatter,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
} from 'recharts'
import {
  useProjects,
  useVelocityReport,
  useBurndownReport,
  useCumulativeFlowReport,
  useControlChartReport,
  useSprints,
} from '@/shared/api/hooks'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@sdlc/ui/ui'
import { Card, CardHeader, CardTitle, CardContent } from '@sdlc/ui/ui'
import { Button } from '@sdlc/ui/ui'
import { Label } from '@sdlc/ui/ui'
import { fetchIssueExport, type IssueExportFormat } from '@/api/export'

type TabValue = 'velocity' | 'burndown' | 'cumulative-flow' | 'control-chart'

export function ReportsPage() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const { data: projects = [] } = useProjects()
  const requestedProjectKey = searchParams.get('project_key') ?? ''
  const [projectId, setProjectId] = useState('')
  const sprintId = searchParams.get('sprint_id') ?? ''
  const [exporting, setExporting] = useState<IssueExportFormat | null>(null)
  const [exportError, setExportError] = useState<string | null>(null)
  const requestedTab = searchParams.get('tab')
  const tab: TabValue =
    requestedTab === 'burndown' ||
    requestedTab === 'cumulative-flow' ||
    requestedTab === 'control-chart'
      ? requestedTab
      : 'velocity'
  const projectKey = projects.find((project) => project.id === projectId)?.key
  const { data: sprints = [] } = useSprints(projectKey)

  useEffect(() => {
    if (!projectId && requestedProjectKey) {
      const project = projects.find((candidate) => candidate.key === requestedProjectKey)
      if (project) setProjectId(project.id)
    }
  }, [projectId, projects, requestedProjectKey])

  function updateParams(values: Record<string, string | undefined>) {
    setSearchParams(
      (previous) => {
        const next = new URLSearchParams(previous)
        for (const [key, value] of Object.entries(values)) {
          if (value) next.set(key, value)
          else next.delete(key)
        }
        return next
      },
      { replace: true },
    )
  }

  async function downloadExport(format: IssueExportFormat) {
    if (!projectKey) return
    setExporting(format)
    setExportError(null)
    try {
      const file = await fetchIssueExport(projectKey, format)
      const url = URL.createObjectURL(file.blob)
      const anchor = document.createElement('a')
      anchor.href = url
      anchor.download = file.filename
      anchor.click()
      URL.revokeObjectURL(url)
    } catch {
      setExportError(t('reports.exportError'))
    } finally {
      setExporting(null)
    }
  }

  const velocity = useVelocityReport(projectId || undefined)
  const burndown = useBurndownReport(sprintId || undefined)
  const cumulativeFlow = useCumulativeFlowReport(projectId || undefined)
  const controlChart = useControlChartReport(projectId || undefined)

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-bold">{t('reports.title')}</h1>

      <div className="flex flex-wrap items-end gap-4">
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="report-project">{t('reports.project')}</Label>
          <select
            id="report-project"
            aria-label={t('reports.project')}
            className="h-9 rounded-md border border-border-strong bg-surface px-3 text-sm text-text-primary"
            value={projectId}
            onChange={(e) => {
              const nextId = e.target.value
              setProjectId(nextId)
              const nextProject = projects.find((project) => project.id === nextId)
              updateParams({ project_key: nextProject?.key, sprint_id: undefined })
            }}
          >
            <option value="">{t('reports.selectProject')}</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </div>

        <div className="flex flex-col gap-1.5">
          <Label htmlFor="report-sprint">Спринт</Label>
          <select
            id="report-sprint"
            aria-label="Спринт"
            disabled={!projectKey}
            value={sprintId}
            onChange={(e) => updateParams({ sprint_id: e.target.value || undefined })}
            className="h-9 w-full rounded-md border border-border-strong bg-surface px-3 text-sm text-text-primary sm:w-64"
          >
            <option value="">Выберите спринт</option>
            {sprints.map((sprint) => (
              <option key={sprint.id} value={sprint.id}>
                {sprint.name}
              </option>
            ))}
          </select>
        </div>

        <div className="flex gap-2" aria-label={t('reports.export')}>
          <Button
            variant="outline"
            size="sm"
            disabled={!projectKey || exporting !== null}
            onClick={() => void downloadExport('csv')}
          >
            <Download className="mr-1.5 h-4 w-4" />
            {exporting === 'csv' ? t('reports.exporting') : t('reports.exportCsv')}
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={!projectKey || exporting !== null}
            onClick={() => void downloadExport('json')}
          >
            <Download className="mr-1.5 h-4 w-4" />
            {exporting === 'json' ? t('reports.exporting') : t('reports.exportJson')}
          </Button>
        </div>
        {exportError && (
          <p role="alert" className="text-sm text-danger">
            {exportError}
          </p>
        )}
      </div>

      {!projectId ? (
        <p className="py-8 text-center text-text-muted">{t('reports.noProject')}</p>
      ) : (
        <Tabs value={tab} onValueChange={(value) => updateParams({ tab: value })}>
          <TabsList className="grid h-auto w-full grid-cols-2 gap-1 lg:grid-cols-4">
            <TabsTrigger value="velocity">{t('reports.tabVelocity')}</TabsTrigger>
            <TabsTrigger value="burndown">{t('reports.tabBurndown')}</TabsTrigger>
            <TabsTrigger value="cumulative-flow">{t('reports.tabCumulativeFlow')}</TabsTrigger>
            <TabsTrigger value="control-chart">{t('reports.tabControlChart')}</TabsTrigger>
          </TabsList>

          {/* Velocity */}
          <TabsContent value="velocity">
            <Card>
              <CardHeader>
                <CardTitle>{t('reports.velocity.title')}</CardTitle>
                <p className="text-sm text-text-muted">{t('reports.velocity.subtitle')}</p>
              </CardHeader>
              <CardContent>
                {velocity.isLoading ? (
                  <p className="py-8 text-center text-text-muted">{t('reports.loading')}</p>
                ) : !velocity.data || velocity.data.sprints.length === 0 ? (
                  <p className="py-8 text-center text-text-muted">{t('reports.velocity.empty')}</p>
                ) : (
                  <ResponsiveContainer width="100%" height={300}>
                    <BarChart data={velocity.data.sprints}>
                      <CartesianGrid strokeDasharray="3 3" />
                      <XAxis dataKey="name" />
                      <YAxis />
                      <Tooltip />
                      <Legend />
                      <Bar
                        dataKey="committed"
                        name={t('reports.velocity.committed')}
                        fill="#3b82f6"
                      />
                      <Bar
                        dataKey="completed"
                        name={t('reports.velocity.completed')}
                        fill="#22c55e"
                      />
                    </BarChart>
                  </ResponsiveContainer>
                )}
              </CardContent>
            </Card>
          </TabsContent>

          {/* Burndown */}
          <TabsContent value="burndown">
            <Card>
              <CardHeader>
                <CardTitle>{t('reports.burndown.title')}</CardTitle>
                <p className="text-sm text-text-muted">{t('reports.burndown.subtitle')}</p>
              </CardHeader>
              <CardContent>
                {!sprintId ? (
                  <p className="py-8 text-center text-text-muted">
                    Выберите спринт для построения графика.
                  </p>
                ) : burndown.isLoading ? (
                  <p className="py-8 text-center text-text-muted">{t('reports.loading')}</p>
                ) : !burndown.data || burndown.data.points.length === 0 ? (
                  <p className="py-8 text-center text-text-muted">{t('reports.burndown.empty')}</p>
                ) : (
                  <ResponsiveContainer width="100%" height={300}>
                    <LineChart data={burndown.data.points}>
                      <CartesianGrid strokeDasharray="3 3" />
                      <XAxis dataKey="date" />
                      <YAxis />
                      <Tooltip />
                      <Legend />
                      <Line
                        type="monotone"
                        dataKey="remaining"
                        name={t('reports.burndown.remaining')}
                        stroke="#ef4444"
                        strokeWidth={2}
                      />
                    </LineChart>
                  </ResponsiveContainer>
                )}
              </CardContent>
            </Card>
          </TabsContent>

          {/* Cumulative Flow */}
          <TabsContent value="cumulative-flow">
            <Card>
              <CardHeader>
                <CardTitle>{t('reports.cumulativeFlow.title')}</CardTitle>
                <p className="text-sm text-text-muted">{t('reports.cumulativeFlow.subtitle')}</p>
              </CardHeader>
              <CardContent>
                {cumulativeFlow.isLoading ? (
                  <p className="py-8 text-center text-text-muted">{t('reports.loading')}</p>
                ) : !cumulativeFlow.data || cumulativeFlow.data.points.length === 0 ? (
                  <p className="py-8 text-center text-text-muted">
                    {t('reports.cumulativeFlow.empty')}
                  </p>
                ) : (
                  <ResponsiveContainer width="100%" height={300}>
                    <AreaChart data={cumulativeFlow.data.points}>
                      <CartesianGrid strokeDasharray="3 3" />
                      <XAxis dataKey="date" />
                      <YAxis />
                      <Tooltip />
                      <Legend />
                      <Area
                        type="monotone"
                        dataKey="todo"
                        stackId="1"
                        name={t('reports.cumulativeFlow.todo')}
                        fill="#94a3b8"
                        stroke="#94a3b8"
                      />
                      <Area
                        type="monotone"
                        dataKey="in_progress"
                        stackId="1"
                        name={t('reports.cumulativeFlow.inProgress')}
                        fill="#3b82f6"
                        stroke="#3b82f6"
                      />
                      <Area
                        type="monotone"
                        dataKey="done"
                        stackId="1"
                        name={t('reports.cumulativeFlow.done')}
                        fill="#22c55e"
                        stroke="#22c55e"
                      />
                    </AreaChart>
                  </ResponsiveContainer>
                )}
              </CardContent>
            </Card>
          </TabsContent>

          {/* Control Chart */}
          <TabsContent value="control-chart">
            <Card>
              <CardHeader>
                <CardTitle>{t('reports.controlChart.title')}</CardTitle>
                <p className="text-sm text-text-muted">{t('reports.controlChart.subtitle')}</p>
              </CardHeader>
              <CardContent>
                {controlChart.isLoading ? (
                  <p className="py-8 text-center text-text-muted">{t('reports.loading')}</p>
                ) : !controlChart.data || controlChart.data.points.length === 0 ? (
                  <p className="py-8 text-center text-text-muted">
                    {t('reports.controlChart.empty')}
                  </p>
                ) : (
                  <ResponsiveContainer width="100%" height={300}>
                    <ScatterChart>
                      <CartesianGrid strokeDasharray="3 3" />
                      <XAxis dataKey="issue_key" name={t('reports.controlChart.issue')} />
                      <YAxis dataKey="cycle_time_days" name={t('reports.controlChart.cycleTime')} />
                      <Tooltip cursor={{ strokeDasharray: '3 3' }} />
                      <Legend />
                      <Scatter
                        data={controlChart.data.points}
                        fill="#8b5cf6"
                        name={t('reports.controlChart.title')}
                      />
                    </ScatterChart>
                  </ResponsiveContainer>
                )}
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>
      )}
    </div>
  )
}
