import { afterEach, describe, it, expect, vi } from 'vitest'
import { act, render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter, useLocation, useNavigate } from 'react-router'

import SearchPage from './'
import { SearchRequestError } from '@/api/search'

const useIssues = vi.hoisted(() => vi.fn())
const useProjects = vi.hoisted(() => vi.fn())
const useUsers = vi.hoisted(() => vi.fn())
vi.mock('@/shared/api/hooks', () => ({ useIssues, useProjects, useUsers }))

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
    <MemoryRouter initialEntries={[url]}>
      <SearchPage />
      <LocationProbe />
    </MemoryRouter>,
  )
}

function mockHooks(data = [issue(1)], error: Error | null = null) {
  const refetch = vi.fn()
  useIssues.mockReturnValue({ data, isLoading: false, error, refetch })
  useProjects.mockReturnValue({ data: [] })
  useUsers.mockReturnValue({ data: [] })
  return refetch
}

afterEach(() => {
  vi.useRealTimers()
  vi.clearAllMocks()
})

describe('SearchPage', () => {
  it('renders search form and results', () => {
    mockHooks()
    renderPage()
    expect(screen.getByText(/поиск задач|search issues/i)).toBeInTheDocument()
    expect(screen.getByText('TT-1')).toBeInTheDocument()
  })

  it('debounces the simple query used by the API and updates the URL', () => {
    vi.useFakeTimers()
    mockHooks()
    renderPage()

    fireEvent.change(screen.getByRole('textbox', { name: /поиск задач|search issues/i }), {
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
    fireEvent.click(screen.getByRole('button', { name: 'JQL' }))
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
    fireEvent.change(screen.getByRole('textbox', { name: /поиск задач|search issues/i }), {
      target: { value: 'new query' },
    })
    expect(screen.getByTestId('location')).not.toHaveTextContent('page=')
    fireEvent.click(screen.getByRole('button', { name: 'JQL' }))
    expect(screen.getByTestId('location')).not.toHaveTextContent('page=')
    expect(screen.getByTestId('location')).not.toHaveTextContent('sort=')
    fireEvent.click(screen.getByRole('button', { name: /простой|simple/i }))
    expect(useIssues).toHaveBeenLastCalledWith(expect.objectContaining({ offset: 0 }))
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
