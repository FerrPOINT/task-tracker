import { useState, type ReactNode } from 'react'
import { Link, useSearchParams } from 'react-router'
import { Download, RotateCcw } from 'lucide-react'
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
import { ErrorState } from '@sdlc/ui/ui'
import { Label } from '@sdlc/ui/ui'
import { fetchIssueExport, type IssueExportFormat } from '@/api/export'

type TabValue = 'velocity' | 'burndown' | 'cumulative-flow' | 'control-chart'

function ReportPanel({
  title,
  subtitle,
  isLoading,
  hasData,
  isEmpty,
  hasError,
  emptyMessage,
  onRetry,
  children,
}: {
  title: string
  subtitle: string
  isLoading: boolean
  hasData: boolean
  isEmpty: boolean
  hasError: boolean
  emptyMessage: string
  onRetry: () => void
  children: ReactNode
}) {
  const { t } = useTranslation()

  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        <p className="text-sm text-text-muted">{subtitle}</p>
      </CardHeader>
      <CardContent className="min-w-0 overflow-x-clip">
        {isLoading && !hasData ? (
          <p className="py-8 text-center text-text-muted">{t('reports.loading')}</p>
        ) : (
          <>
            {hasError && (
              <ErrorState
                message={t(hasData ? 'reports.refreshError' : 'reports.loadError')}
                onRetry={onRetry}
              />
            )}
            {hasData ? (
              isEmpty ? (
                <p className="py-8 text-center text-text-muted">{emptyMessage}</p>
              ) : (
                children
              )
            ) : !hasError ? (
              <p className="py-8 text-center text-text-muted">{emptyMessage}</p>
            ) : null}
          </>
        )}
      </CardContent>
    </Card>
  )
}

export function ReportsPage() {
  const { t } = useTranslation()
  const [searchParams, setSearchParams] = useSearchParams()
  const projectsQuery = useProjects()
  const projects = projectsQuery.data ?? []
  const requestedProjectKey = searchParams.get('project_key') ?? ''
  const sprintId = searchParams.get('sprint_id') ?? ''
  const [exporting, setExporting] = useState<IssueExportFormat | null>(null)
  const [exportError, setExportError] = useState<string | null>(null)
  const [failedExport, setFailedExport] = useState<IssueExportFormat | null>(null)
  const requestedTab = searchParams.get('tab')
  const tab: TabValue =
    requestedTab === 'burndown' ||
    requestedTab === 'cumulative-flow' ||
    requestedTab === 'control-chart'
      ? requestedTab
      : 'velocity'
  const selectedProject = projects.find((project) => project.key === requestedProjectKey)
  const projectId = selectedProject?.id ?? ''
  const projectKey = selectedProject?.key
  const sprintsQuery = useSprints(tab === 'burndown' ? projectKey : undefined)
  const sprints = sprintsQuery.data ?? []
  const selectedSprintId = sprints.some((sprint) => sprint.id === sprintId) ? sprintId : ''

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
    setFailedExport(null)
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
      setFailedExport(format)
    } finally {
      setExporting(null)
    }
  }

  const velocity = useVelocityReport(tab === 'velocity' ? projectId || undefined : undefined)
  const burndown = useBurndownReport(
    tab === 'burndown' && projectId ? selectedSprintId || undefined : undefined,
  )
  const cumulativeFlow = useCumulativeFlowReport(
    tab === 'cumulative-flow' ? projectId || undefined : undefined,
  )
  const controlChart = useControlChartReport(
    tab === 'control-chart' ? projectId || undefined : undefined,
  )

  return (
    <div className="mx-auto w-full max-w-7xl space-y-4">
      <h1 className="text-2xl font-bold">{t('reports.title')}</h1>

      <div className="flex flex-wrap items-end gap-3">
        <div className="flex min-w-0 flex-col gap-1.5">
          <Label htmlFor="report-project">{t('reports.project')}</Label>
          <select
            id="report-project"
            aria-label={t('reports.project')}
            className="h-10 max-w-full rounded-md border border-border-strong bg-surface px-3 text-sm text-text-primary"
            value={projectId}
            disabled={
              (projectsQuery.isLoading || Boolean(projectsQuery.error)) && projects.length === 0
            }
            onChange={(e) => {
              const nextId = e.target.value
              const nextProject = projects.find((project) => project.id === nextId)
              updateParams({ project_key: nextProject?.key, sprint_id: undefined })
              setExportError(null)
              setFailedExport(null)
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

        {tab === 'burndown' && projectKey && (
          <div className="flex min-w-0 flex-col gap-1.5">
            <Label htmlFor="report-sprint">{t('reports.sprint')}</Label>
            <select
              id="report-sprint"
              disabled={
                (sprintsQuery.isLoading || Boolean(sprintsQuery.error)) && sprints.length === 0
              }
              value={selectedSprintId}
              onChange={(e) => updateParams({ sprint_id: e.target.value || undefined })}
              className="h-10 max-w-full rounded-md border border-border-strong bg-surface px-3 text-sm text-text-primary sm:w-64"
            >
              <option value="">
                {sprintsQuery.isLoading && sprints.length === 0
                  ? t('reports.loadingSprints')
                  : sprints.length === 0 && !sprintsQuery.error
                    ? t('reports.noSprints')
                    : t('reports.selectSprint')}
              </option>
              {sprints.map((sprint) => (
                <option key={sprint.id} value={sprint.id}>
                  {sprint.name}
                </option>
              ))}
            </select>
          </div>
        )}

        <div
          className="flex gap-2 sm:ml-auto"
          aria-label={t('reports.export')}
          aria-busy={exporting !== null}
        >
          <Button
            variant="outline"
            size="sm"
            className="h-10"
            disabled={!projectKey || exporting !== null}
            onClick={() => void downloadExport('csv')}
          >
            <Download className="mr-1.5 h-4 w-4" />
            {exporting === 'csv' ? t('reports.exporting') : t('reports.exportCsv')}
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-10"
            disabled={!projectKey || exporting !== null}
            onClick={() => void downloadExport('json')}
          >
            <Download className="mr-1.5 h-4 w-4" />
            {exporting === 'json' ? t('reports.exporting') : t('reports.exportJson')}
          </Button>
        </div>
      </div>

      {projectsQuery.error && (
        <div className="flex flex-wrap items-center gap-2" role="alert">
          <span className="text-sm text-danger">{t('reports.projectsError')}</span>
          <Button
            variant="outline"
            size="sm"
            className="h-10"
            onClick={() => void projectsQuery.refetch()}
          >
            <RotateCcw className="h-4 w-4" />
            {t('common.retry')}
          </Button>
        </div>
      )}
      {tab === 'burndown' && sprintsQuery.error && (
        <div className="flex flex-wrap items-center gap-2" role="alert">
          <span className="text-sm text-danger">{t('reports.sprintsError')}</span>
          <Button
            variant="outline"
            size="sm"
            className="h-10"
            onClick={() => void sprintsQuery.refetch()}
          >
            <RotateCcw className="h-4 w-4" />
            {t('common.retry')}
          </Button>
        </div>
      )}
      {exportError && (
        <div className="flex flex-wrap items-center gap-2" role="alert">
          <span className="text-sm text-danger">{exportError}</span>
          {failedExport && (
            <Button
              variant="outline"
              size="sm"
              className="h-10"
              onClick={() => void downloadExport(failedExport)}
            >
              <RotateCcw className="h-4 w-4" />
              {t('common.retry')}
            </Button>
          )}
        </div>
      )}

      {projectsQuery.isLoading && projects.length === 0 ? (
        <p className="py-8 text-center text-text-muted">{t('reports.loadingProjects')}</p>
      ) : projectsQuery.error && projects.length === 0 ? null : !projectId ? (
        <div className="space-y-3 py-8 text-center text-text-muted">
          <p>
            {t(
              projects.length === 0
                ? 'reports.noProjects'
                : requestedProjectKey
                  ? 'reports.unknownProject'
                  : 'reports.noProject',
            )}
          </p>
          {projects.length === 0 && (
            <Button asChild variant="outline" className="h-10">
              <Link to="/projects">{t('projects.title')}</Link>
            </Button>
          )}
        </div>
      ) : (
        <Tabs value={tab} onValueChange={(value) => updateParams({ tab: value })}>
          <TabsList className="grid h-auto w-full grid-cols-2 gap-1 lg:grid-cols-4">
            <TabsTrigger
              className="min-h-10 whitespace-normal px-2 text-center leading-4"
              value="velocity"
            >
              {t('reports.tabVelocity')}
            </TabsTrigger>
            <TabsTrigger
              className="min-h-10 whitespace-normal px-2 text-center leading-4"
              value="burndown"
            >
              {t('reports.tabBurndown')}
            </TabsTrigger>
            <TabsTrigger
              className="min-h-10 whitespace-normal px-2 text-center leading-4"
              value="cumulative-flow"
            >
              {t('reports.tabCumulativeFlow')}
            </TabsTrigger>
            <TabsTrigger
              className="min-h-10 whitespace-normal px-2 text-center leading-4"
              value="control-chart"
            >
              {t('reports.tabControlChart')}
            </TabsTrigger>
          </TabsList>

          <TabsContent value="velocity">
            <ReportPanel
              title={t('reports.velocity.title')}
              subtitle={t('reports.velocity.subtitle')}
              isLoading={velocity.isLoading}
              hasData={Boolean(velocity.data)}
              isEmpty={velocity.data?.sprints.length === 0}
              hasError={Boolean(velocity.error)}
              emptyMessage={t('reports.velocity.empty')}
              onRetry={() => void velocity.refetch()}
            >
              <div aria-hidden="true">
                <ResponsiveContainer width="100%" height={300}>
                  <BarChart data={velocity.data?.sprints ?? []} margin={{ right: 20 }}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="name" />
                    <YAxis />
                    <Tooltip />
                    <Legend />
                    <Bar
                      dataKey="committed"
                      name={t('reports.velocity.committed')}
                      fill="#3b82f6"
                      isAnimationActive={false}
                    />
                    <Bar
                      dataKey="completed"
                      name={t('reports.velocity.completed')}
                      fill="#22c55e"
                      isAnimationActive={false}
                    />
                  </BarChart>
                </ResponsiveContainer>
              </div>
              <table className="sr-only" aria-label={t('reports.velocity.title')}>
                <thead>
                  <tr>
                    <th scope="col">{t('reports.velocity.sprint')}</th>
                    <th scope="col">{t('reports.velocity.committed')}</th>
                    <th scope="col">{t('reports.velocity.completed')}</th>
                  </tr>
                </thead>
                <tbody>
                  {velocity.data?.sprints.map((sprint, index) => (
                    <tr key={`${sprint.name}-${index}`}>
                      <th scope="row">{sprint.name}</th>
                      <td>{sprint.committed}</td>
                      <td>{sprint.completed}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </ReportPanel>
          </TabsContent>

          <TabsContent value="burndown">
            <ReportPanel
              title={t('reports.burndown.title')}
              subtitle={t('reports.burndown.subtitle')}
              isLoading={burndown.isLoading}
              hasData={Boolean(burndown.data)}
              isEmpty={burndown.data?.points.length === 0}
              hasError={Boolean(burndown.error)}
              emptyMessage={t(
                selectedSprintId
                  ? 'reports.burndown.empty'
                  : sprintsQuery.isLoading
                    ? 'reports.loadingSprints'
                    : sprintId && sprintsQuery.data
                      ? 'reports.unknownSprint'
                      : 'reports.selectSprintPrompt',
              )}
              onRetry={() => void burndown.refetch()}
            >
              <div aria-hidden="true">
                <ResponsiveContainer width="100%" height={300}>
                  <LineChart data={burndown.data?.points ?? []}>
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
                      isAnimationActive={false}
                    />
                  </LineChart>
                </ResponsiveContainer>
              </div>
              <table className="sr-only" aria-label={t('reports.burndown.title')}>
                <thead>
                  <tr>
                    <th scope="col">{t('reports.burndown.date')}</th>
                    <th scope="col">{t('reports.burndown.remaining')}</th>
                  </tr>
                </thead>
                <tbody>
                  {burndown.data?.points.map((point) => (
                    <tr key={point.date}>
                      <th scope="row">{point.date}</th>
                      <td>{point.remaining}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </ReportPanel>
          </TabsContent>

          <TabsContent value="cumulative-flow">
            <ReportPanel
              title={t('reports.cumulativeFlow.title')}
              subtitle={t('reports.cumulativeFlow.subtitle')}
              isLoading={cumulativeFlow.isLoading}
              hasData={Boolean(cumulativeFlow.data)}
              isEmpty={cumulativeFlow.data?.points.length === 0}
              hasError={Boolean(cumulativeFlow.error)}
              emptyMessage={t('reports.cumulativeFlow.empty')}
              onRetry={() => void cumulativeFlow.refetch()}
            >
              <div aria-hidden="true">
                <ResponsiveContainer width="100%" height={300}>
                  <AreaChart data={cumulativeFlow.data?.points ?? []}>
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
                      isAnimationActive={false}
                    />
                    <Area
                      type="monotone"
                      dataKey="in_progress"
                      stackId="1"
                      name={t('reports.cumulativeFlow.inProgress')}
                      fill="#3b82f6"
                      stroke="#3b82f6"
                      isAnimationActive={false}
                    />
                    <Area
                      type="monotone"
                      dataKey="done"
                      stackId="1"
                      name={t('reports.cumulativeFlow.done')}
                      fill="#22c55e"
                      stroke="#22c55e"
                      isAnimationActive={false}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
              <table className="sr-only" aria-label={t('reports.cumulativeFlow.title')}>
                <thead>
                  <tr>
                    <th scope="col">{t('reports.cumulativeFlow.date')}</th>
                    <th scope="col">{t('reports.cumulativeFlow.todo')}</th>
                    <th scope="col">{t('reports.cumulativeFlow.inProgress')}</th>
                    <th scope="col">{t('reports.cumulativeFlow.done')}</th>
                  </tr>
                </thead>
                <tbody>
                  {cumulativeFlow.data?.points.map((point) => (
                    <tr key={point.date}>
                      <th scope="row">{point.date}</th>
                      <td>{point.todo}</td>
                      <td>{point.in_progress}</td>
                      <td>{point.done}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </ReportPanel>
          </TabsContent>

          <TabsContent value="control-chart">
            <ReportPanel
              title={t('reports.controlChart.title')}
              subtitle={t('reports.controlChart.subtitle')}
              isLoading={controlChart.isLoading}
              hasData={Boolean(controlChart.data)}
              isEmpty={controlChart.data?.points.length === 0}
              hasError={Boolean(controlChart.error)}
              emptyMessage={t('reports.controlChart.empty')}
              onRetry={() => void controlChart.refetch()}
            >
              <div aria-hidden="true">
                <ResponsiveContainer width="100%" height={300}>
                  <ScatterChart>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="issue_key" name={t('reports.controlChart.issue')} />
                    <YAxis dataKey="cycle_time_days" name={t('reports.controlChart.cycleTime')} />
                    <Tooltip cursor={{ strokeDasharray: '3 3' }} />
                    <Legend />
                    <Scatter
                      data={controlChart.data?.points ?? []}
                      fill="#8b5cf6"
                      name={t('reports.controlChart.title')}
                      isAnimationActive={false}
                    />
                  </ScatterChart>
                </ResponsiveContainer>
              </div>
              <table className="sr-only" aria-label={t('reports.controlChart.title')}>
                <thead>
                  <tr>
                    <th scope="col">{t('reports.controlChart.issue')}</th>
                    <th scope="col">{t('reports.controlChart.cycleTime')}</th>
                  </tr>
                </thead>
                <tbody>
                  {controlChart.data?.points.map((point) => (
                    <tr key={point.issue_key}>
                      <td>{point.issue_key}</td>
                      <td>{point.cycle_time_days}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </ReportPanel>
          </TabsContent>
        </Tabs>
      )}
    </div>
  )
}
