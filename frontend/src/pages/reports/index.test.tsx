import { describe, expect, it, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

import { ReportsPage } from './index'
import { fetchIssueExport } from '@/api/export'

// --- Mock hooks (NOT backend) -------------------------------------------------
const useProjects = vi.hoisted(() => vi.fn())
const useVelocityReport = vi.hoisted(() => vi.fn())
const useBurndownReport = vi.hoisted(() => vi.fn())
const useCumulativeFlowReport = vi.hoisted(() => vi.fn())
const useControlChartReport = vi.hoisted(() => vi.fn())
const useSprints = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', async () => {
  const actual = await vi.importActual<typeof import('@/shared/api/hooks')>('@/shared/api/hooks')
  return {
    ...actual,
    useProjects,
    useVelocityReport,
    useBurndownReport,
    useCumulativeFlowReport,
    useControlChartReport,
    useSprints,
  }
})

vi.mock('@/api/export', () => ({ fetchIssueExport: vi.fn() }))

// recharts renders SVG in jsdom; stub chart + container to render data as text
vi.mock('recharts', () => ({
  ResponsiveContainer: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="responsive-container">{children}</div>
  ),
  BarChart: ({
    data,
    children,
  }: {
    data?: Array<Record<string, unknown>>
    children?: React.ReactNode
  }) => (
    <div data-testid="chart">
      {data?.map((d, i) => (
        <div key={i}>
          {Object.values(d).map((v, j) => (
            <span key={j}>{String(v)}</span>
          ))}
        </div>
      ))}
      {children}
    </div>
  ),
  LineChart: ({
    data,
    children,
  }: {
    data?: Array<Record<string, unknown>>
    children?: React.ReactNode
  }) => (
    <div data-testid="chart">
      {data?.map((d, i) => (
        <div key={i}>
          {Object.values(d).map((v, j) => (
            <span key={j}>{String(v)}</span>
          ))}
        </div>
      ))}
      {children}
    </div>
  ),
  AreaChart: ({
    data,
    children,
  }: {
    data?: Array<Record<string, unknown>>
    children?: React.ReactNode
  }) => (
    <div data-testid="chart">
      {data?.map((d, i) => (
        <div key={i}>
          {Object.values(d).map((v, j) => (
            <span key={j}>{String(v)}</span>
          ))}
        </div>
      ))}
      {children}
    </div>
  ),
  ScatterChart: ({
    children,
  }: {
    data?: Array<Record<string, unknown>>
    children?: React.ReactNode
  }) => <div data-testid="chart">{children}</div>,
  Bar: () => null,
  Line: () => null,
  Area: () => null,
  Scatter: ({ data }: { data?: Array<Record<string, unknown>> }) => (
    <>
      {data?.map((d, i) => (
        <div key={i}>
          {Object.entries(d).map(([k, v], j) => (
            <span key={j}>{`${k}:${String(v)}`}</span>
          ))}
        </div>
      ))}
    </>
  ),
  XAxis: ({ dataKey }: { dataKey?: string }) => <span data-testid="xaxis">{dataKey ?? ''}</span>,
  YAxis: () => null,
  CartesianGrid: () => null,
  Tooltip: () => null,
  Legend: () => <div data-testid="legend" />,
}))

// --- Helpers ------------------------------------------------------------------

const projects = [
  {
    id: 'proj-1',
    key: 'TT',
    name: 'Task Tracker',
    owner_id: 'u1',
    todo_count: 0,
    in_progress_count: 0,
    done_count: 0,
  },
  {
    id: 'proj-2',
    key: 'QA',
    name: 'QA Tool',
    owner_id: 'u1',
    todo_count: 0,
    in_progress_count: 0,
    done_count: 0,
  },
]

function makeQueryClient() {
  return new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
}

function renderPage(initialEntry = '/reports') {
  return render(
    <QueryClientProvider client={makeQueryClient()}>
      <MemoryRouter initialEntries={[initialEntry]}>
        <ReportsPage />
      </MemoryRouter>
    </QueryClientProvider>,
  )
}

function setLoadedProject() {
  useProjects.mockReturnValue({ data: projects, isLoading: false })
}

async function selectProject(user: ReturnType<typeof userEvent.setup>) {
  const select = screen.getByRole('combobox', { name: /проект|project/i })
  await user.selectOptions(select, 'proj-1')
}

async function selectSprint(user: ReturnType<typeof userEvent.setup>) {
  const select = screen.getByRole('combobox', { name: /спринт|sprint/i })
  await user.selectOptions(select, 'sprint-1')
}

// --- Tests --------------------------------------------------------------------

describe('ReportsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useProjects.mockReturnValue({ data: projects, isLoading: false })
    useVelocityReport.mockReturnValue({ data: undefined, isLoading: true })
    useBurndownReport.mockReturnValue({ data: undefined, isLoading: true })
    useCumulativeFlowReport.mockReturnValue({ data: undefined, isLoading: true })
    useControlChartReport.mockReturnValue({ data: undefined, isLoading: true })
    useSprints.mockReturnValue({
      data: [{ id: 'sprint-1', name: 'Sprint 1' }],
      isLoading: false,
    })
  })

  it('renders the page title and project selector', () => {
    setLoadedProject()
    renderPage()
    expect(screen.getByRole('heading', { name: /отчёты|reports/i })).toBeInTheDocument()
    expect(screen.getByRole('combobox', { name: /проект|project/i })).toBeInTheDocument()
  })

  it('shows velocity loading state on the default tab', async () => {
    const user = userEvent.setup()
    setLoadedProject()
    useVelocityReport.mockReturnValue({ data: undefined, isLoading: true })
    renderPage()
    await selectProject(user)
    // default tab = velocity, isLoading true
    expect(screen.getByText(/загрузка отчёта|loading report/i)).toBeInTheDocument()
  })

  it('renders velocity bar chart with mock data', async () => {
    const user = userEvent.setup()
    setLoadedProject()
    useVelocityReport.mockReturnValue({
      data: {
        sprints: [
          { name: 'Sprint 1', committed: 30, completed: 25 },
          { name: 'Sprint 2', committed: 28, completed: 28 },
        ],
      },
      isLoading: false,
    })
    renderPage()
    await selectProject(user)

    // Bar chart renders sprint names and values as text via mock
    await waitFor(() => {
      expect(within(screen.getByTestId('chart')).getByText('Sprint 1')).toBeInTheDocument()
      expect(within(screen.getByTestId('chart')).getByText('Sprint 2')).toBeInTheDocument()
    })
    expect(screen.getByTestId('chart')).toBeInTheDocument()
    expect(screen.getByTestId('legend')).toBeInTheDocument()
    const table = screen.getByRole('table', { name: /скорость|velocity/i })
    expect(within(table).getAllByRole('row')).toHaveLength(3)
    expect(within(table).getByRole('row', { name: /Sprint 1 30 25/i })).toBeInTheDocument()
    expect(screen.getByTestId('chart').closest('[aria-hidden="true"]')).toBeInTheDocument()
  })

  it('switches to burndown tab and renders line chart', async () => {
    const user = userEvent.setup()
    setLoadedProject()
    useVelocityReport.mockReturnValue({ data: undefined, isLoading: false })
    useBurndownReport.mockReturnValue({
      data: {
        sprint_name: 'Sprint 1',
        points: [
          { date: '2026-08-01', remaining: 10 },
          { date: '2026-08-05', remaining: 5 },
        ],
      },
      isLoading: false,
    })
    renderPage()
    await selectProject(user)
    await user.click(screen.getByRole('tab', { name: /burndown/i }))
    await selectSprint(user)

    await waitFor(() => {
      expect(within(screen.getByTestId('chart')).getByText('2026-08-01')).toBeInTheDocument()
      expect(within(screen.getByTestId('chart')).getByText('2026-08-05')).toBeInTheDocument()
    })
    const table = screen.getByRole('table', { name: /burndown/i })
    expect(within(table).getByRole('row', { name: /2026-08-01 10/i })).toBeInTheDocument()
    expect(within(table).getByRole('row', { name: /2026-08-05 5/i })).toBeInTheDocument()
    expect(screen.getByTestId('chart').closest('[aria-hidden="true"]')).toBeInTheDocument()
  })

  it('switches to cumulative flow tab and renders stacked area chart', async () => {
    const user = userEvent.setup()
    setLoadedProject()
    useCumulativeFlowReport.mockReturnValue({
      data: {
        points: [
          { date: '2026-08-01', todo: 5, in_progress: 2, done: 1 },
          { date: '2026-08-02', todo: 3, in_progress: 4, done: 2 },
        ],
      },
      isLoading: false,
    })
    renderPage()
    await selectProject(user)

    await user.click(screen.getByRole('tab', { name: /кумулятивный поток|cumulative flow/i }))

    await waitFor(() => {
      expect(within(screen.getByTestId('chart')).getByText('2026-08-01')).toBeInTheDocument()
    })
    expect(screen.getByTestId('chart')).toBeInTheDocument()
    const table = screen.getByRole('table', { name: /кумулятивный поток|cumulative flow/i })
    expect(within(table).getByRole('row', { name: /2026-08-01 5 2 1/i })).toBeInTheDocument()
    expect(within(table).getByRole('row', { name: /2026-08-02 3 4 2/i })).toBeInTheDocument()
    expect(screen.getByTestId('chart').closest('[aria-hidden="true"]')).toBeInTheDocument()
  })

  it('switches to control chart tab and renders scatter chart', async () => {
    const user = userEvent.setup()
    setLoadedProject()
    useControlChartReport.mockReturnValue({
      data: {
        points: [
          { issue_key: 'TT-1', cycle_time_days: 1.5 },
          { issue_key: 'TT-2', cycle_time_days: 3.2 },
        ],
      },
      isLoading: false,
    })
    renderPage()
    await selectProject(user)

    await user.click(screen.getByRole('tab', { name: /контрольная диаграмма|control chart/i }))

    await waitFor(() => {
      expect(screen.getByText(/issue_key:TT-1/)).toBeInTheDocument()
      expect(screen.getByText(/issue_key:TT-2/)).toBeInTheDocument()
    })
  })

  it('shows empty state when velocity data has no sprints', async () => {
    const user = userEvent.setup()
    setLoadedProject()
    useVelocityReport.mockReturnValue({ data: { sprints: [] }, isLoading: false })
    renderPage()
    await selectProject(user)
    expect(screen.getByText(/нет данных о скорости|no velocity data/i)).toBeInTheDocument()
  })

  it('shows empty state for burndown when no points', async () => {
    const user = userEvent.setup()
    setLoadedProject()
    useBurndownReport.mockReturnValue({
      data: { sprint_name: 'Sprint 1', points: [] },
      isLoading: false,
    })
    renderPage()
    await selectProject(user)
    await user.click(screen.getByRole('tab', { name: /burndown/i }))
    await selectSprint(user)
    expect(screen.getByText(/нет данных burndown|no burndown data/i)).toBeInTheDocument()
  })

  it('shows no-project prompt when no project is selected', () => {
    useProjects.mockReturnValue({ data: projects, isLoading: false })
    renderPage()
    // Initially no project selected -> prompt visible
    expect(
      screen.getByText(/выберите проект для просмотра отчётов|select a project to view reports/i),
    ).toBeInTheDocument()
  })

  it('selecting a project enables report hooks', async () => {
    const user = userEvent.setup()
    setLoadedProject()
    useVelocityReport.mockReturnValue({ data: { sprints: [] }, isLoading: false })

    renderPage()

    const select = screen.getByRole('combobox', { name: /проект|project/i })
    await user.selectOptions(select, 'proj-1')

    expect(useVelocityReport).toHaveBeenCalled()
  })

  it('uses the URL project and requests only the active report', async () => {
    const user = userEvent.setup()
    renderPage('/reports?project_key=TT')

    expect(screen.getByRole('combobox', { name: /проект|project/i })).toHaveValue('proj-1')
    expect(useVelocityReport).toHaveBeenLastCalledWith('proj-1')
    expect(useCumulativeFlowReport).toHaveBeenLastCalledWith(undefined)
    expect(useControlChartReport).toHaveBeenLastCalledWith(undefined)
    expect(useSprints).toHaveBeenLastCalledWith(undefined)
    expect(screen.queryByRole('combobox', { name: /спринт|sprint/i })).not.toBeInTheDocument()

    await user.click(screen.getByRole('tab', { name: /burndown/i }))
    expect(useSprints).toHaveBeenLastCalledWith('TT')
    expect(useVelocityReport).toHaveBeenLastCalledWith(undefined)
    expect(screen.getByRole('combobox', { name: /спринт|sprint/i })).toBeInTheDocument()
  })

  it('does not request Burndown for a sprint link without an accessible project', () => {
    renderPage('/reports?tab=burndown&sprint_id=sprint-1')
    expect(useBurndownReport).toHaveBeenLastCalledWith(undefined)
  })

  it('rejects a sprint deep link that does not belong to the selected project', () => {
    useBurndownReport.mockReturnValue({ data: undefined, isLoading: false })
    renderPage('/reports?project_key=TT&tab=burndown&sprint_id=sprint-from-other-project')

    expect(useBurndownReport).toHaveBeenLastCalledWith(undefined)
    expect(screen.getByRole('combobox', { name: /спринт|sprint/i })).toHaveValue('')
    expect(
      screen.getByText(/спринт из ссылки недоступен|sprint in this link is unavailable/i),
    ).toBeInTheDocument()
  })

  it('asks for a sprint before requesting Burndown data', () => {
    useBurndownReport.mockReturnValue({ data: undefined, isLoading: false })
    renderPage('/reports?project_key=TT&tab=burndown')
    expect(
      screen.getByText(
        /выберите спринт для построения графика|select a sprint to build this chart/i,
      ),
    ).toBeInTheDocument()
    expect(useBurndownReport).toHaveBeenLastCalledWith(undefined)
  })

  it('distinguishes project loading, failure and an empty project list', async () => {
    const retry = vi.fn()
    useProjects.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('503'),
      refetch: retry,
    })
    const page = renderPage()

    expect(screen.getByRole('alert')).toHaveTextContent(
      /не удалось загрузить проекты|could not load projects/i,
    )
    expect(
      screen.queryByText(/выберите проект для просмотра|select a project to view/i),
    ).not.toBeInTheDocument()
    await userEvent.setup().click(screen.getByRole('button', { name: /повторить|retry/i }))
    expect(retry).toHaveBeenCalledOnce()

    useProjects.mockReturnValue({ data: [], isLoading: false, error: null, refetch: retry })
    page.rerender(
      <QueryClientProvider client={makeQueryClient()}>
        <MemoryRouter>
          <ReportsPage />
        </MemoryRouter>
      </QueryClientProvider>,
    )
    expect(screen.getByText(/нет доступных проектов|no projects available/i)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /проекты|projects/i })).toHaveAttribute(
      'href',
      '/projects',
    )
  })

  it('shows report failure with retry instead of claiming that data is empty', async () => {
    const user = userEvent.setup()
    const retry = vi.fn()
    useVelocityReport.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('503'),
      refetch: retry,
    })
    renderPage()
    await selectProject(user)

    expect(screen.getByRole('alert')).toHaveTextContent(
      /не удалось загрузить отчёт|could not load report/i,
    )
    expect(screen.queryByText(/нет данных о скорости|no velocity data/i)).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /повторить|retry/i }))
    expect(retry).toHaveBeenCalledOnce()
  })

  it.each([
    ['burndown', useBurndownReport, 'sprint_id=sprint-1'],
    ['cumulative-flow', useCumulativeFlowReport, ''],
    ['control-chart', useControlChartReport, ''],
  ])('shows a retryable error on the %s tab', async (tab, reportHook, extraQuery) => {
    const user = userEvent.setup()
    const retry = vi.fn()
    reportHook.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('503'),
      refetch: retry,
    })
    renderPage(`/reports?project_key=TT&tab=${tab}${extraQuery ? `&${extraQuery}` : ''}`)

    expect(screen.getByRole('alert')).toHaveTextContent(
      /не удалось загрузить отчёт|could not load report/i,
    )
    await user.click(screen.getByRole('button', { name: /повторить|retry/i }))
    expect(retry).toHaveBeenCalledOnce()
  })

  it('keeps previously loaded report data visible when refresh fails', async () => {
    const user = userEvent.setup()
    useVelocityReport.mockReturnValue({
      data: { sprints: [{ name: 'Sprint 1', committed: 30, completed: 25 }] },
      isLoading: false,
      error: new Error('503'),
      refetch: vi.fn(),
    })
    renderPage()
    await selectProject(user)

    expect(screen.getByRole('alert')).toHaveTextContent(
      /показаны ранее загруженные данные|previously loaded data/i,
    )
    expect(within(screen.getByTestId('chart')).getByText('Sprint 1')).toBeInTheDocument()
  })

  it('provides readable control-chart data for assistive technology', () => {
    useControlChartReport.mockReturnValue({
      data: { points: [{ issue_key: 'TT-42', cycle_time_days: 2.5 }] },
      isLoading: false,
    })
    renderPage('/reports?project_key=TT&tab=control-chart')

    const table = screen.getByRole('table', { name: /контрольная диаграмма|control chart/i })
    expect(within(table).getByText('TT-42')).toBeInTheDocument()
    expect(within(table).getByText('2.5')).toBeInTheDocument()
    expect(screen.getByTestId('chart').closest('[aria-hidden]')).toHaveAttribute(
      'aria-hidden',
      'true',
    )
  })

  it('shows sprint loading failure with retry only on Burndown', async () => {
    const user = userEvent.setup()
    const retry = vi.fn()
    useSprints.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('503'),
      refetch: retry,
    })
    renderPage('/reports?project_key=TT')

    expect(
      screen.queryByText(/не удалось загрузить спринты|could not load sprints/i),
    ).not.toBeInTheDocument()
    await user.click(screen.getByRole('tab', { name: /burndown/i }))
    expect(screen.getByRole('alert')).toHaveTextContent(
      /не удалось загрузить спринты|could not load sprints/i,
    )
    expect(screen.getByRole('combobox', { name: /спринт|sprint/i })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: /повторить|retry/i }))
    expect(retry).toHaveBeenCalledOnce()
  })

  it('retries a failed export with the same project and format', async () => {
    const user = userEvent.setup()
    vi.mocked(fetchIssueExport).mockRejectedValue(new Error('503'))
    renderPage('/reports?project_key=TT')

    await user.click(screen.getByRole('button', { name: 'CSV' }))
    expect(await screen.findByRole('alert')).toHaveTextContent(
      /не удалось экспортировать|could not export/i,
    )
    await user.click(screen.getByRole('button', { name: /повторить|retry/i }))
    await waitFor(() => expect(fetchIssueExport).toHaveBeenCalledTimes(2))
    expect(fetchIssueExport).toHaveBeenNthCalledWith(1, 'TT', 'csv')
    expect(fetchIssueExport).toHaveBeenNthCalledWith(2, 'TT', 'csv')
  })
})
