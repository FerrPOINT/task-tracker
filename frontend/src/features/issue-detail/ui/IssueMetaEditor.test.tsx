import { describe, it, expect, vi, beforeAll, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { Sprint } from '@/api/sprint'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@pdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { IssueMetaEditor } from './IssueMetaEditor'
import type { Issue } from '@/api/issue'

beforeAll(() => {
  i18n.changeLanguage('en')
})

const directory = vi.hoisted(() => ({ loading: false, failed: false, retry: vi.fn() }))
beforeEach(() => {
  directory.loading = false
  directory.failed = false
  directory.retry.mockClear()
})

vi.mock('@/shared/api/hooks', () => ({
  useUsers: () => ({
    data:
      directory.loading || directory.failed
        ? undefined
        : [
            { id: 'u1', username: 'alice', display_name: 'Alice' },
            { id: 'u2', username: 'bob', display_name: 'Bob' },
            { id: 'u3', username: 'charlie', display_name: 'Charlie' },
          ],
    isLoading: directory.loading,
    isError: directory.failed,
    isFetching: false,
    refetch: directory.retry,
    error: directory.failed ? new Error('Directory unavailable') : null,
  }),
}))

function wrapper(children: React.ReactNode) {
  return (
    <ThemeProvider>
      <MemoryRouter>{children}</MemoryRouter>
    </ThemeProvider>
  )
}

const issue: Issue = {
  id: 'i1',
  key: 'TT-1',
  summary: 'Fix login bug',
  description: 'Login crashes',
  status: 'Open',
  status_id: 's1',
  priority: 'High',
  project_key: 'TT',
  project_name: 'Task Tracker',
  reporter_id: 'u1',
  issue_type: 'Bug',
  labels: [],
  time_spent_seconds: 0,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
  assignee_id: null,
  assignee_name: null,
  reporter_name: null,
  sprint_id: null,
}

describe('IssueMetaEditor', () => {
  it('presents compact properties and opens only the requested menu without writing', async () => {
    const onChange = vi.fn()
    render(wrapper(<IssueMetaEditor issue={issue}  onChange={onChange} />))
    expect(screen.getAllByRole('combobox')).toHaveLength(2)
    expect(screen.getByRole('combobox', { name: /priority.*high/i })).toBeVisible()
    expect(screen.getByRole('combobox', { name: /assignee.*unassigned/i })).toBeVisible()
    const user = userEvent.setup()
    const priority = screen.getByRole('combobox', { name: /priority/i })
    await user.click(priority)
    expect(screen.getAllByRole('option').map((option) => option.textContent)).toEqual([
      'Lowest',
      'Low',
      'Medium',
      'High',
      'Highest',
    ])
    await user.keyboard('{Escape}')
    expect(priority).toHaveFocus()
    expect(onChange).not.toHaveBeenCalled()
  })

  it('saves one priority change and restores keyboard focus', async () => {
    const onChange = vi.fn()
    render(wrapper(<IssueMetaEditor issue={issue}  onChange={onChange} />))
    const user = userEvent.setup()
    const priority = screen.getByRole('combobox', { name: /priority/i })
    priority.focus()
    await user.keyboard('{Enter}')
    await user.click(screen.getByRole('option', { name: 'Low' }))
    expect(onChange).toHaveBeenCalledExactlyOnceWith({ priority: 'Low' })
    expect(priority).toHaveFocus()
  })

  it('offers every directory user and can clear an assignment', async () => {
    const onChange = vi.fn()
    render(
      wrapper(
        <IssueMetaEditor
          issue={{ ...issue, assignee_id: 'u1', assignee_name: 'Alice' }}

          onChange={onChange}
        />,
      ),
    )
    const user = userEvent.setup()
    await user.click(screen.getByRole('combobox', { name: /assignee/i }))
    expect(screen.getByRole('option', { name: 'Charlie' })).toBeVisible()
    await user.click(screen.getByRole('option', { name: 'Unassigned' }))
    expect(onChange).toHaveBeenCalledExactlyOnceWith({ assignee_id: null })
  })

  it('keeps a former assignee readable and unavailable for selection', async () => {
    render(
      wrapper(
        <IssueMetaEditor
          issue={{ ...issue, assignee_id: 'u4', assignee_name: 'Former User' }}

          onChange={vi.fn()}
        />,
      ),
    )
    await userEvent.click(screen.getByRole('combobox', { name: /assignee.*Former User/i }))
    expect(screen.getByRole('option', { name: 'Former User' })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
  })

  it('saves a sprint and clears it using the same existing patch contract', async () => {
    const onChange = vi.fn()
    const sprints = [{ id: 'sp1', name: 'Sprint 1' } as Sprint]
    const view = render(
      wrapper(
        <IssueMetaEditor issue={issue}  sprints={sprints} onChange={onChange} />,
      ),
    )
    const user = userEvent.setup()
    await user.click(screen.getByRole('combobox', { name: /sprint/i }))
    await user.click(screen.getByRole('option', { name: 'Sprint 1' }))
    expect(onChange).toHaveBeenCalledWith({ sprint_id: 'sp1' })
    view.rerender(
      wrapper(
        <IssueMetaEditor
          issue={{ ...issue, sprint_id: 'sp1' }}

          sprints={sprints}
          onChange={onChange}
        />,
      ),
    )
    await user.click(screen.getByRole('combobox', { name: /sprint/i }))
    await user.click(screen.getByRole('option', { name: /no sprint/i }))
    expect(onChange).toHaveBeenLastCalledWith({ sprint_id: null })
  })

  it.each(['loading', 'failed'] as const)(
    'blocks assignee editing while the directory is %s',
    async (state) => {
      directory.loading = state === 'loading'
      directory.failed = state === 'failed'
      const onChange = vi.fn()
      render(wrapper(<IssueMetaEditor issue={issue}  onChange={onChange} />))
      const user = userEvent.setup()
      expect(screen.getByRole('combobox', { name: /assignee/i })).toBeDisabled()
      await user.click(screen.getByRole('combobox', { name: /assignee/i }))
      expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
      if (state === 'failed') {
        await user.click(screen.getByRole('button', { name: /retry/i }))
        expect(directory.retry).toHaveBeenCalledOnce()
      }
      expect(onChange).not.toHaveBeenCalled()
    },
  )

  it('closes an open choice while saving and allows editing again after pending ends', async () => {
    const onChange = vi.fn()
    const view = render(
      wrapper(<IssueMetaEditor issue={issue}  onChange={onChange} />),
    )
    const user = userEvent.setup()
    await user.click(screen.getByRole('combobox', { name: /priority/i }))
    view.rerender(
      wrapper(<IssueMetaEditor issue={issue}  disabled onChange={onChange} />),
    )
    await waitFor(() => expect(screen.queryByRole('listbox')).not.toBeInTheDocument())
    expect(screen.getByRole('combobox', { name: /priority/i })).toBeDisabled()
    view.rerender(wrapper(<IssueMetaEditor issue={issue}  onChange={onChange} />))
    await user.click(screen.getByRole('combobox', { name: /priority/i }))
    expect(screen.getByRole('listbox')).toBeInTheDocument()
    expect(onChange).not.toHaveBeenCalled()
  })
})
