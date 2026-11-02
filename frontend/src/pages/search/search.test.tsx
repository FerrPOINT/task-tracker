import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, describe, it, expect, vi } from 'vitest'
import { act, render, screen, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { within } from '@testing-library/react'
import { useNamespaceNavigate as useNavigate } from '@pdlc/ui/ui'
import { MemoryRouter, useLocation } from 'react-router'

import SearchPage from './'
import { SearchRequestError } from '@/api/search'

const useIssues = vi.hoisted(() => vi.fn())
const useProjects = vi.hoisted(() => vi.fn())
const useUsers = vi.hoisted(() => vi.fn())
const projectCatalog = vi.hoisted(() => vi.fn())
vi.mock('@/shared/api/hooks', () => ({ useIssues, useProjects, useUsers }))
vi.mock('@/widgets/project-choice-catalog', () => ({
  useProjectChoiceCatalog: projectCatalog,
  PROJECT_CHOICE_PAGE_SIZE: 20,
}))

const issue = (number: number) => ({
  id: `i${number}`,
  key: `TT-${number}`,
  summary: `Issue ${number}`,
  status: 'In Progress',
  priority: 'High',
  assignee_name: 'Ivan',
})

function LocationProbe() {
  const location = useLocation()
  const navigate = useNavigate()
  return (
    <>
      <output data-testid="location">{location.search}</output>
      <button onClick={() => navigate(-1)}>Browser back</button>
      <button onClick={() => navigate(1)}>Browser forward</button>
      <button onClick={() => navigate('/search?mode=jql&jql=project%20%3D%20DEMO&page=3')}>
        Other URL
      </button>
    </>
  )
}

function renderPage(url = '/search') {
  return render(
    <QueryClientProvider
      client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
    >
      <MemoryRouter initialEntries={[url]}>
        <SearchPage />
        <LocationProbe />
      </MemoryRouter>
    </QueryClientProvider>,
  )
}

function mockHooks(data = [issue(1)], error: Error | null = null) {
  const refetch = vi.fn()
  useIssues.mockReturnValue({ data, isLoading: false, error, refetch })
  useProjects.mockReturnValue({ data: [] })
  useUsers.mockReturnValue({ data: [], isPending: false, isError: false, refetch: vi.fn() })
  projectCatalog.mockReturnValue({
    data: [],
    isPending: false,
    isError: false,
    selected: undefined,
    selectedPending: false,
    selectedUnavailable: false,
    searchable: false,
    catalog: undefined,
    hasMore: false,
    refetch: vi.fn(),
    retrySelected: vi.fn(),
  })
  return refetch
}

afterEach(() => {
  vi.useRealTimers()
  vi.clearAllMocks()
})

describe('SearchPage', () => {
  it('does not replace unknown deep-link filter values on mount or catalog refresh', () => {
    mockHooks()
    renderPage('/search?status=custom&priority=Urgent&sort=custom_desc&assignee_id=missing&page=3')
    expect(screen.getByTestId('location')).toHaveTextContent(
      'status=custom&priority=Urgent&sort=custom_desc&assignee_id=missing&page=3',
    )
    expect(useIssues).toHaveBeenLastCalledWith(
      expect.objectContaining({
        status: 'custom',
        priority: 'Urgent',
        assignee_id: 'missing',
        sort_by: 'custom',
        sort_order: 'desc',
        offset: 50,
      }),
    )
  })

  it('commits the Select value once, keeps priority casing and restores filters with history', async () => {
    const user = userEvent.setup()
    mockHooks()
    renderPage('/search?q=retained&page=3&registry_instance_id=registry&namespace_id=namespace')
    const priority = screen.getByRole('combobox', { name: /приоритет/i })
    await user.click(priority)
    await user.click(await screen.findByRole('option', { name: /^высокий$/i }))
    expect(screen.getByTestId('location')).toHaveTextContent('priority=High')
    expect(screen.getByTestId('location')).not.toHaveTextContent('page=')
    expect(screen.getByTestId('location')).toHaveTextContent(
      'registry_instance_id=registry&namespace_id=namespace',
    )
    expect(useIssues).toHaveBeenLastCalledWith(
      expect.objectContaining({ priority: 'High', q: 'retained', offset: 0 }),
    )
    await user.click(screen.getByRole('button', { name: 'Browser back' }))
    expect(screen.getByTestId('location')).toHaveTextContent('page=3')
    expect(screen.getByTestId('location')).not.toHaveTextContent('priority=')
  })

  it('searches the complete user catalog and selects IDs for identical display names', async () => {
    const user = userEvent.setup()
    mockHooks()
    useUsers.mockReturnValue({
      data: [
        { id: 'first', display_name: 'Оператор', username: 'alpha' },
        { id: 'second', display_name: 'Оператор', username: 'beta' },
      ],
      isPending: false,
      isError: false,
    })
    renderPage('/search?page=3')
    await user.click(screen.getByRole('button', { name: /исполнитель:.*все исполнители/i }))
    const dialog = screen.getByRole('dialog')
    await user.type(within(dialog).getByRole('searchbox'), 'beta')
    expect(
      within(dialog).queryByRole('button', { name: 'Оператор · alpha' }),
    ).not.toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: 'Оператор · beta' }))
    expect(screen.getByTestId('location')).toHaveTextContent('assignee_id=second')
    expect(screen.getByTestId('location')).not.toHaveTextContent('page=')
    expect(useIssues).toHaveBeenLastCalledWith(expect.objectContaining({ assignee_id: 'second' }))
  })

  it('pages the project catalog without changing a selected value outside the page', async () => {
    const user = userEvent.setup()
    mockHooks()
    projectCatalog.mockImplementation((selected: string, offset: number) => ({
      data: [{ key: `P${offset}`, name: `Project ${offset}` }],
      selected:
        selected === 'P350'
          ? { key: selected, name: 'Selected elsewhere' }
          : { key: selected, name: 'Project 20' },
      hasMore: offset === 0,
      searchable: false,
      catalog: undefined,
      isPending: false,
      isError: false,
      selectedPending: false,
      selectedUnavailable: false,
    }))
    renderPage('/search?project_key=P350&page=3')
    await user.click(screen.getByRole('button', { name: 'Проект: Selected elsewhere · P350' }))
    const dialog = screen.getByRole('dialog')
    expect(within(dialog).queryByRole('searchbox')).not.toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: 'Далее' }))
    expect(projectCatalog).toHaveBeenLastCalledWith('P350', 20, '')
    expect(within(dialog).getByRole('button', { name: 'Project 20 · P20' })).toBeInTheDocument()
    expect(screen.getByTestId('location')).toHaveTextContent('project_key=P350&page=3')
    await user.click(within(dialog).getByRole('button', { name: 'Project 20 · P20' }))
    expect(screen.getByTestId('location')).toHaveTextContent('project_key=P20')
    expect(screen.getByTestId('location')).not.toHaveTextContent('page=')
  })

  it('keeps a project failure separate from empty options and other issue results', async () => {
    const user = userEvent.setup()
    mockHooks()
    const retry = vi.fn()
    projectCatalog.mockReturnValue({
      data: undefined,
      isPending: false,
      isError: true,
      refetch: retry,
    })
    renderPage()
    await user.click(screen.getByRole('button', { name: 'Проект: Все проекты' }))
    const dialog = screen.getByRole('dialog')
    expect(within(dialog).getByRole('alert')).toHaveTextContent('Не удалось загрузить проекты.')
    expect(within(dialog).queryByText('Нет доступных вариантов.')).not.toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: 'Повторить' }))
    expect(retry).toHaveBeenCalledTimes(1)
    await user.keyboard('{Escape}')
    expect(screen.getByRole('link', { name: 'Issue 1' })).toBeInTheDocument()
  })
  it('renders search form and results', () => {
    mockHooks()
    renderPage()
    expect(screen.getByText(/поиск задач|search issues/i)).toBeInTheDocument()
    expect(screen.getByText('TT-1')).toBeInTheDocument()
    expect(screen.getByRole('list').querySelectorAll('li')).toHaveLength(1)
    expect(screen.getByRole('link', { name: 'Issue 1' })).toHaveAttribute('href', '/issues/i1')
  })

  it('keeps the full issue title in the link and tooltip', () => {
    const longTitle = 'Plan cross-team integration and verify every dependent workflow'
    mockHooks([{ ...issue(1), summary: longTitle }])
    renderPage()
    expect(screen.getByRole('link', { name: longTitle })).toHaveAttribute('title', longTitle)
  })

  it('debounces the simple query used by the API and updates the URL', () => {
    vi.useFakeTimers()
    mockHooks()
    renderPage()

    fireEvent.change(screen.getByRole('searchbox', { name: /поиск задач|search issues/i }), {
      target: { value: 'test' },
    })
    expect(screen.getByTestId('location')).toHaveTextContent('q=test')
    expect(useIssues).toHaveBeenLastCalledWith(expect.objectContaining({ q: undefined }))

    act(() => vi.advanceTimersByTime(350))
    expect(useIssues).toHaveBeenLastCalledWith(expect.objectContaining({ q: 'test' }))
  })

  it('runs JQL only on submit and keeps its draft until then', () => {
    mockHooks()
    renderPage()
    fireEvent.click(screen.getByRole('radio', { name: 'JQL' }))
    const input = screen.getByRole('textbox', { name: /например: project|for example: project/i })
    fireEvent.change(input, { target: { value: 'project = TT' } })

    expect(useIssues).toHaveBeenLastCalledWith(expect.objectContaining({ jql: undefined }))
    expect(screen.getByTestId('location')).not.toHaveTextContent('jql=')

    fireEvent.click(screen.getByRole('button', { name: /выполнить|execute/i }))
    expect(useIssues).toHaveBeenLastCalledWith(expect.objectContaining({ jql: 'project = TT' }))
    expect(input).toHaveValue('project = TT')
  })

  it('does not report a server failure as no results and allows retry', () => {
    const refetch = mockHooks(undefined, new SearchRequestError(503))
    renderPage()
    expect(screen.queryByText(/ничего не найдено|no results/i)).not.toBeInTheDocument()
    expect(screen.getByText(/не удалось загрузить|could not load/i)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /повторить|retry/i }))
    expect(refetch).toHaveBeenCalledOnce()
  })

  it('shows JQL validation errors without an empty-state or retry loop', () => {
    mockHooks(undefined, new SearchRequestError(400))
    renderPage('/search?mode=jql&jql=invalid')
    expect(screen.getByRole('alert')).toHaveTextContent(/некорректный|invalid/i)
    expect(screen.queryByText(/ничего не найдено|no results/i)).not.toBeInTheDocument()
    expect(
      screen.getByRole('textbox', { name: /например: project|for example: project/i }),
    ).toHaveValue('invalid')
  })

  it('shows 25 rows from a 26-row sentinel and pages by URL offset', () => {
    mockHooks(Array.from({ length: 26 }, (_, index) => issue(index + 1)))
    renderPage('/search?page=2')
    expect(useIssues).toHaveBeenLastCalledWith(expect.objectContaining({ limit: 26, offset: 25 }))
    expect(screen.getAllByRole('link', { name: /^Issue \d+$/ })).toHaveLength(25)
    expect(screen.getByText(/страница 2|page 2/i)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /далее|next/i }))
    expect(screen.getByTestId('location')).toHaveTextContent('page=3')
    expect(useIssues).toHaveBeenLastCalledWith(expect.objectContaining({ offset: 50 }))
  })

  it('resets page on query or mode changes and clears hidden simple sorting', () => {
    mockHooks()
    renderPage('/search?page=3&sort=priority_desc')
    fireEvent.change(screen.getByRole('searchbox', { name: /поиск задач|search issues/i }), {
      target: { value: 'new query' },
    })
    expect(screen.getByTestId('location')).not.toHaveTextContent('page=')
    fireEvent.click(screen.getByRole('radio', { name: 'JQL' }))
    expect(screen.getByTestId('location')).not.toHaveTextContent('page=')
    expect(screen.getByTestId('location')).not.toHaveTextContent('sort=')
    fireEvent.click(screen.getByRole('radio', { name: /простой|simple/i }))
    expect(useIssues).toHaveBeenLastCalledWith(expect.objectContaining({ offset: 0 }))
  })

  it('clears only search filters while preserving Namespace and the all-project catalog', () => {
    mockHooks()
    renderPage(
      '/search?q=test&status=open&registry_instance_id=registry&namespace_id=namespace&project_scope=all',
    )
    fireEvent.click(screen.getByRole('button', { name: /сбросить|clear/i }))
    expect(screen.getByTestId('location')).toHaveTextContent(
      'registry_instance_id=registry&namespace_id=namespace&project_scope=all',
    )
    expect(screen.getByTestId('location')).not.toHaveTextContent('status=')
    expect(screen.getByTestId('location')).not.toHaveTextContent('q=')
  })

  it('restores URL-backed JQL and page with browser history', () => {
    mockHooks()
    renderPage('/search?mode=jql&jql=project%20%3D%20TT&page=2')
    const input = screen.getByRole('textbox', { name: /например: project|for example: project/i })
    expect(input).toHaveValue('project = TT')
    fireEvent.click(screen.getByRole('button', { name: 'Other URL' }))
    expect(input).toHaveValue('project = DEMO')
    expect(useIssues).toHaveBeenLastCalledWith(expect.objectContaining({ offset: 50 }))
    fireEvent.click(screen.getByRole('button', { name: 'Browser back' }))
    expect(input).toHaveValue('project = TT')
    expect(useIssues).toHaveBeenLastCalledWith(expect.objectContaining({ offset: 25 }))
    fireEvent.click(screen.getByRole('button', { name: 'Browser forward' }))
    expect(input).toHaveValue('project = DEMO')
  })

  it('ignores simple-only sorting in a JQL deep link', () => {
    mockHooks()
    renderPage('/search?mode=jql&jql=project%20%3D%20TT&sort=priority_desc')
    expect(useIssues).toHaveBeenLastCalledWith(
      expect.objectContaining({ jql: 'project = TT', sort_by: undefined, sort_order: undefined }),
    )
  })
})
