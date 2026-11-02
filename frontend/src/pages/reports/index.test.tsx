import { chooseValue } from '../../test/choice'

import { acceptSpaceSession, useSpaceSession } from '@/shared/auth/space-session'
import { useAuthStore } from '@/shared/auth/store'
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest'
import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useNamespaceNavigate as useNavigate } from '@pdlc/ui/ui'
import { MemoryRouter, useLocation } from 'react-router'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

import { ReportsPage } from './index'
import { fetchIssueExport } from '@/api/export'
import { getProject } from '@/api/project'
import { ApiError } from '@pdlc/ui/lib'

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
vi.mock('@/api/project', async (original) => ({
  ...(await original<object>()),
  getProject: vi.fn(),
}))

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

function LocationProbe() {
  const location = useLocation()
  const navigate = useNavigate()

  return (
    <>
      <output aria-label="current location">{`${location.pathname}${location.search}`}</output>
      <button type="button" onClick={() => navigate(-1)}>
        Back in history
      </button>
      <button type="button" onClick={() => navigate(1)}>
        Forward in history
      </button>
    </>
  )
}

function renderPage(initialEntry = '/reports') {
  return render(
    <QueryClientProvider client={makeQueryClient()}>
      <MemoryRouter initialEntries={[initialEntry]}>
        <ReportsPage />
        <LocationProbe />
      </MemoryRouter>
    </QueryClientProvider>,
  )
}

function setLoadedProject() {
  useProjects.mockReturnValue({ data: projects, isLoading: false })
}

async function selectProject(user: ReturnType<typeof userEvent.setup>) {
  const select = screen.getByRole('button', { name: /проект|project/i })
  await user.click(select)
  await user.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: 'Task Tracker (TT)' }),
  )
}

async function selectSprint() {
  const select = screen.getByRole('combobox', { name: /спринт|sprint/i })
  await chooseValue(select, 'sprint-1')
}

// --- Tests --------------------------------------------------------------------

describe('ReportsPage', () => {
  afterEach(() => {
    vi.unstubAllEnvs()
    vi.unstubAllGlobals()
  })
  beforeEach(() => {
    vi.clearAllMocks()
    vi.mocked(getProject).mockRejectedValue(new Error('Project unavailable'))
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

  it('resolves an accessible project beyond the page without falling back or discarding its URL', async () => {
    vi.mocked(getProject).mockResolvedValue({
      ...projects[0]!,
      id: 'proj-150',
      key: 'P150',
      name: 'Beyond the first page',
      description: '',
      owner_name: 'Fixture owner',
    })
    renderPage('/reports?project_key=P150&source=deep-link')
    await waitFor(() => expect(useVelocityReport).toHaveBeenLastCalledWith('proj-150'))
    expect(getProject).toHaveBeenCalledWith('P150', expect.any(AbortSignal))
    expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
      '/reports?project_key=P150&source=deep-link',
    )
    expect(screen.queryByText(/Проект из ссылки недоступен/)).not.toBeInTheDocument()
  })

  it('pages the catalog without changing the report and confirms a choice by its key', async () => {
    useProjects.mockImplementation((offset: number) => ({
      data: Array.from({ length: 21 }, (_, index) => ({
        ...projects[0]!,
        id: `id-${offset + index}`,
        key: `P${offset + index}`,
        name: `Project ${offset + index}`,
      })),
      isLoading: false,
    }))
    vi.mocked(getProject).mockResolvedValue({
      ...projects[0]!,
      description: '',
      owner_name: 'Fixture owner',
    })
    const user = userEvent.setup()
    renderPage('/reports?project_key=TT&source=catalog')
    await waitFor(() => expect(useVelocityReport).toHaveBeenLastCalledWith('proj-1'))
    await user.click(screen.getByRole('button', { name: /проект|project/i }))
    const dialog = within(screen.getByRole('dialog'))
    await user.click(dialog.getByRole('button', { name: 'Далее' }))
    await dialog.findByRole('button', { name: 'Project 20 (P20)' })
    expect(useProjects).toHaveBeenLastCalledWith(20, 21, '', true, { allPages: false })
    expect(useVelocityReport).toHaveBeenLastCalledWith('proj-1')
    await user.click(dialog.getByRole('button', { name: 'Project 20 (P20)' }))
    expect(useVelocityReport).toHaveBeenLastCalledWith('id-20')
    expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
      '/reports?project_key=P20&source=catalog',
    )
  })

  it('keeps an inaccessible explicit project distinct from an empty catalog', async () => {
    renderPage('/reports?project_key=OLD&source=deep-link')
    await screen.findByText(/Проект из ссылки недоступен/)
    expect(useVelocityReport).toHaveBeenLastCalledWith(undefined)
    expect(screen.getByRole('button', { name: /CSV/i })).toBeDisabled()
    expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
      '/reports?project_key=OLD&source=deep-link',
    )
    expect(screen.queryByText('Нет проектов')).not.toBeInTheDocument()
  })

  it('passes debounced v2 search to the counted catalog and resets its page without changing the URL', async () => {
    vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'true')
    useProjects.mockReturnValue({ data: projects, catalog: { total: 45 }, isLoading: false })
    const user = userEvent.setup()
    renderPage('/reports?source=search')
    await user.click(screen.getByRole('button', { name: /проект|project/i }))
    const dialog = within(screen.getByRole('dialog'))
    await user.click(dialog.getByRole('button', { name: 'Далее' }))
    expect(useProjects).toHaveBeenLastCalledWith(20, 21, '', true, { allPages: false })
    await user.type(dialog.getByRole('searchbox'), 'team')
    await waitFor(() =>
      expect(useProjects).toHaveBeenLastCalledWith(0, 21, 'team', true, { allPages: false }),
    )
    await user.click(dialog.getByRole('button', { name: 'Закрыть' }))
    expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
      '/reports?source=search',
    )
    expect(useVelocityReport).toHaveBeenLastCalledWith(undefined)
  })

  it('does not call an archive-filtered empty page an empty catalog while a next page exists', async () => {
    vi.stubEnv('VITE_NAMESPACE_ENABLED', 'true')
    const rows = Array.from({ length: 21 }, (_, index) => ({
      ...projects[0]!,
      id: `id-${index}`,
      key: `P${index}`,
    }))
    useProjects.mockReturnValue({ data: rows, isLoading: false })
    vi.stubGlobal(
      'fetch',
      vi.fn(
        async () =>
          new Response(
            JSON.stringify(
              rows.slice(0, 20).map((project) => ({
                label: project.key,
                resource_key: project.key,
                binding: {
                  state: 'archived',
                  resource: { resource_id: project.id },
                  namespace: { registry_instance_id: 'registry', namespace_id: project.id },
                },
              })),
            ),
            { status: 200 },
          ),
      ),
    )
    const user = userEvent.setup()
    renderPage('/reports?source=archive')
    await screen.findByText('Выберите проект для просмотра отчётов.')
    expect(screen.queryByText('Нет доступных проектов.')).not.toBeInTheDocument()
    expect(screen.queryByRole('link', { name: 'Проекты' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /проект|project/i }))
    expect(within(screen.getByRole('dialog')).getByRole('button', { name: 'Далее' })).toBeEnabled()
  })

  it('renders the page title and project selector', () => {
    setLoadedProject()
    renderPage()
    expect(screen.getByRole('heading', { name: /отчёты|reports/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /проект|project/i })).toBeInTheDocument()
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
    await selectSprint()

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
    expect(table.closest('.sr-only')).toBeInTheDocument()
    expect(table).not.toHaveClass('sr-only')
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
    await selectSprint()
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

    const select = screen.getByRole('button', { name: /проект|project/i })
    await user.click(select)
    await user.click(
      within(screen.getByRole('dialog')).getByRole('button', { name: 'Task Tracker (TT)' }),
    )

    expect(useVelocityReport).toHaveBeenCalled()
  })

  it('uses the URL project and requests only the active report', async () => {
    const user = userEvent.setup()
    renderPage('/reports?project_key=TT')

    expect(screen.getByRole('button', { name: /проект|project/i })).toHaveTextContent(
      'Task Tracker (TT)',
    )
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

  it('stores report choices in history and restores them with Back and Forward', async () => {
    const user = userEvent.setup()
    renderPage('/reports?source=qa&project_key=TT')

    await user.click(screen.getByRole('tab', { name: /burndown/i }))
    expect(screen.getByRole('status', { name: /current location/i })).toHaveTextContent(
      '/reports?source=qa&project_key=TT&tab=burndown',
    )

    await user.click(screen.getByRole('tab', { name: /контрольная диаграмма|control chart/i }))
    expect(screen.getByRole('status', { name: /current location/i })).toHaveTextContent(
      '/reports?source=qa&project_key=TT&tab=control-chart',
    )

    await user.click(screen.getByRole('button', { name: 'Back in history' }))
    await waitFor(() =>
      expect(screen.getByRole('tab', { name: /burndown/i })).toHaveAttribute(
        'aria-selected',
        'true',
      ),
    )

    await user.click(screen.getByRole('button', { name: 'Forward in history' }))
    await waitFor(() =>
      expect(
        screen.getByRole('tab', { name: /контрольная диаграмма|control chart/i }),
      ).toHaveAttribute('aria-selected', 'true'),
    )
  })

  it.each([
    '/reports?source=qa&project_key=TT&tab=velocity',
    '/reports?source=qa&project_key=TT&tab=unknown',
  ])('canonicalizes the default or invalid tab in %s', async (initialEntry) => {
    renderPage(initialEntry)

    await waitFor(() =>
      expect(screen.getByRole('status', { name: /current location/i })).toHaveTextContent(
        '/reports?source=qa&project_key=TT',
      ),
    )
    expect(screen.getByRole('tab', { name: /скорость|velocity/i })).toHaveAttribute(
      'aria-selected',
      'true',
    )
  })

  it('preserves unrelated parameters and restores project selection from history', async () => {
    const user = userEvent.setup()
    renderPage('/reports?source=qa')

    await user.click(screen.getByRole('button', { name: /проект|project/i }))
    await user.click(
      within(screen.getByRole('dialog')).getByRole('button', { name: 'Task Tracker (TT)' }),
    )
    expect(screen.getByRole('status', { name: /current location/i })).toHaveTextContent(
      '/reports?source=qa&project_key=TT',
    )

    await user.click(screen.getByRole('button', { name: 'Back in history' }))
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /проект|project/i })).toHaveTextContent(
        'Выберите проект',
      ),
    )
    expect(screen.getByRole('status', { name: /current location/i })).toHaveTextContent(
      '/reports?source=qa',
    )
  })

  it('uses shared Base sizing contracts for the primary report tabs', () => {
    renderPage('/reports?project_key=TT')

    expect(screen.getByRole('button', { name: /проект|project/i })).toHaveClass('base-control')
    for (const tab of screen.getAllByRole('tab')) {
      expect(tab).toHaveClass('base-control', 'base-tabs-trigger')
    }
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

    expect(
      screen
        .getByText(/показаны ранее загруженные данные|previously loaded data/i)
        .closest('[role="status"]'),
    ).toBeInTheDocument()
    expect(within(screen.getByTestId('chart')).getByText('Sprint 1')).toBeInTheDocument()
  })

  it.each([
    [
      'velocity',
      useVelocityReport,
      { sprints: [{ name: 'Private sprint', committed: 30, completed: 25 }] },
    ],
    [
      'burndown',
      useBurndownReport,
      { sprint_name: 'Private sprint', points: [{ date: '2026-08-01', remaining: 5, ideal: 4 }] },
    ],
    [
      'cumulative-flow',
      useCumulativeFlowReport,
      { points: [{ date: '2026-08-01', todo: 5, in_progress: 2, done: 1 }] },
    ],
    [
      'control-chart',
      useControlChartReport,
      { points: [{ issue_key: 'TT-PRIVATE', cycle_time_days: 2.5 }] },
    ],
  ] as const)('uses the Base resource boundary for cached %s reports', async (tab, hook, data) => {
    const user = userEvent.setup()
    const retry = vi.fn()
    for (const status of [401, 403, 404, 422, 503]) {
      hook.mockReturnValue({
        data,
        isLoading: false,
        error: new ApiError(status, 'report unavailable'),
        refetch: retry,
      })
      const view = renderPage(`/reports?project_key=TT&tab=${tab}&sprint_id=sprint-1`)
      if (status === 503) {
        expect(screen.getByTestId('chart')).toBeInTheDocument()
        expect(screen.getByRole('table')).toBeInTheDocument()
        expect(
          screen
            .getByText(/показаны ранее загруженные данные|previously loaded data/i)
            .closest('[role="status"]'),
        ).toBeInTheDocument()
        expect(retry).not.toHaveBeenCalled()
        await user.click(screen.getByRole('button', { name: /повторить|retry/i }))
        expect(retry).toHaveBeenCalledOnce()
      } else {
        expect(screen.queryByTestId('chart')).not.toBeInTheDocument()
        expect(screen.queryByRole('table')).not.toBeInTheDocument()
        expect(screen.queryByRole('button', { name: /повторить|retry/i })).not.toBeInTheDocument()
        expect(
          screen.queryByText(/показаны ранее загруженные данные|previously loaded data/i),
        ).not.toBeInTheDocument()
        expect(retry).not.toHaveBeenCalled()
      }
      view.unmount()
    }
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
  it.each([false, true])(
    'delivers the completed export only in its initiating identity (same actor: %s)',
    async (same) => {
      vi.stubEnv('VITE_SPACE_ACCESS_ENABLED', 'true')
      function login(subject: string, token: string) {
        useAuthStore.getState().setAuth({ token, userId: subject, email: 'fixture@example.test' })
        acceptSpaceSession(
          {
            accessToken: token,
            subject,
            email: 'fixture@example.test',
            name: subject,
            returnTo: '/',
            expiresAt: Date.now() + 60_000,
          },
          { id: subject },
        )
      }
      const create = Object.getOwnPropertyDescriptor(URL, 'createObjectURL')
      const revoke = Object.getOwnPropertyDescriptor(URL, 'revokeObjectURL')
      const objectURL = vi.fn(() => 'blob:scoped-export'),
        revokeURL = vi.fn()
      Object.defineProperty(URL, 'createObjectURL', { configurable: true, value: objectURL })
      Object.defineProperty(URL, 'revokeObjectURL', { configurable: true, value: revokeURL })
      const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {})
      try {
        login('A', 'a')
        let complete: (file: { blob: Blob; filename: string }) => void = () => {}
        vi.mocked(fetchIssueExport).mockImplementation(
          () =>
            new Promise((resolve) => {
              complete = resolve
            }),
        )
        renderPage('/reports?project_key=TT')
        await userEvent.setup().click(screen.getByRole('button', { name: 'CSV' }))
        await waitFor(() => expect(fetchIssueExport).toHaveBeenCalledTimes(1))
        await act(async () => {
          login(same ? 'A' : 'B', same ? 'fresh-a' : 'b')
          complete({ blob: new Blob(['A export']), filename: 'A.csv' })
        })
        await waitFor(() => expect(screen.getByRole('button', { name: 'CSV' })).toBeEnabled())
        expect(objectURL).toHaveBeenCalledTimes(same ? 1 : 0)
        expect(click).toHaveBeenCalledTimes(same ? 1 : 0)
        expect(revokeURL).toHaveBeenCalledTimes(same ? 1 : 0)
      } finally {
        click.mockRestore()
        if (create) Object.defineProperty(URL, 'createObjectURL', create)
        else Reflect.deleteProperty(URL, 'createObjectURL')
        if (revoke) Object.defineProperty(URL, 'revokeObjectURL', revoke)
        else Reflect.deleteProperty(URL, 'revokeObjectURL')
        vi.unstubAllEnvs()
        useAuthStore.getState().logout()
        useSpaceSession.setState({ session: null })
      }
    },
  )
})
