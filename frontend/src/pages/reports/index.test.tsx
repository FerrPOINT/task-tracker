import { describe, expect, it, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

import { ReportsPage } from './index'

// --- Mock hooks (NOT backend) -------------------------------------------------
const useProjects = vi.hoisted(() => vi.fn())
const useVelocityReport = vi.hoisted(() => vi.fn())
const useBurndownReport = vi.hoisted(() => vi.fn())
const useCumulativeFlowReport = vi.hoisted(() => vi.fn())
const useControlChartReport = vi.hoisted(() => vi.fn())

vi.mock('@/shared/api/hooks', async () => {
  const actual = await vi.importActual<typeof import('@/shared/api/hooks')>('@/shared/api/hooks')
  return {
    ...actual,
    useProjects,
    useVelocityReport,
    useBurndownReport,
    useCumulativeFlowReport,
    useControlChartReport,
  }
})

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

function renderPage() {
  return render(
    <QueryClientProvider client={makeQueryClient()}>
      <MemoryRouter>
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

// --- Tests --------------------------------------------------------------------

describe('ReportsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useProjects.mockReturnValue({ data: projects, isLoading: false })
    useVelocityReport.mockReturnValue({ data: undefined, isLoading: true })
    useBurndownReport.mockReturnValue({ data: undefined, isLoading: true })
    useCumulativeFlowReport.mockReturnValue({ data: undefined, isLoading: true })
    useControlChartReport.mockReturnValue({ data: undefined, isLoading: true })
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
      expect(screen.getByText('Sprint 1')).toBeInTheDocument()
      expect(screen.getByText('Sprint 2')).toBeInTheDocument()
    })
    expect(screen.getByTestId('chart')).toBeInTheDocument()
    expect(screen.getByTestId('legend')).toBeInTheDocument()
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

    await waitFor(() => {
      expect(screen.getByText('2026-08-01')).toBeInTheDocument()
      expect(screen.getByText('2026-08-05')).toBeInTheDocument()
    })
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
      expect(screen.getByText('2026-08-01')).toBeInTheDocument()
    })
    expect(screen.getByTestId('chart')).toBeInTheDocument()
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
})
