import { choiceOptions, chooseValue } from '@/test/choice'
import { describe, it, expect, vi, beforeAll } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@pdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { IssueMetaEditor } from './IssueMetaEditor'
import type { Issue } from '@/api/issue'
import type { Board } from '@/api/board'

beforeAll(() => {
  i18n.changeLanguage('en')
})

vi.mock('@/shared/api/hooks', () => ({
  useUsers: () => ({
    data: [
      { id: 'u1', username: 'alice', display_name: 'Alice' },
      { id: 'u2', username: 'bob', display_name: 'Bob' },
      { id: 'u3', username: 'charlie', display_name: 'Charlie' },
    ],
    isLoading: false,
    error: null,
  }),
  useStatuses: () => ({
    data: [
      { id: 's1', name: 'Open' },
      { id: 's2', name: 'In Progress' },
      { id: 's3', name: 'Done' },
    ],
    isLoading: false,
    error: null,
  }),
  useTransitions: () => ({
    data: [
      { id: 't1', from_status_id: 's1', to_status_id: 's2' },
      { id: 't2', from_status_id: 's1', to_status_id: 's3' },
    ],
    isLoading: false,
    error: null,
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

const columns: Board['columns'] = [
  { id: 's1', name: 'Open', issue_ids: [] },
  { id: 's2', name: 'In Progress', issue_ids: [] },
  { id: 's3', name: 'Done', issue_ids: [] },
]

describe('IssueMetaEditor', () => {
  it('renders metadata select fields', async () => {
    render(wrapper(<IssueMetaEditor issue={issue} columns={columns} onChange={vi.fn()} />))
    expect(screen.getByText(/status/i)).toBeInTheDocument()
    expect(screen.getByText(/priority/i)).toBeInTheDocument()
    expect(screen.getByText(/assignee/i)).toBeInTheDocument()
    const options = (await choiceOptions(screen.getByLabelText(/priority/i))).map(
      (option) => option.value,
    )
    expect(options).toEqual(['Lowest', 'Low', 'Medium', 'High', 'Highest'])
    expect(screen.queryByRole('option', { name: /critical/i })).not.toBeInTheDocument()
  })

  it('calls onChange when priority is changed', async () => {
    const onChange = vi.fn()
    render(wrapper(<IssueMetaEditor issue={issue} columns={columns} onChange={onChange} />))
    await chooseValue(screen.getByLabelText(/priority/i), 'Low')
    expect(onChange).toHaveBeenCalledWith({ priority: 'Low' })
  })

  it('offers every active directory user for assignment', async () => {
    render(wrapper(<IssueMetaEditor issue={issue} columns={columns} onChange={vi.fn()} />))
    expect(
      (await choiceOptions(screen.getByLabelText(/assignee/i))).some(
        (option) => option.textContent === 'Charlie',
      ),
    ).toBe(true)
  })

  it('keeps stale assignee visible as a disabled current option', async () => {
    const staleIssue = {
      ...issue,
      assignee_id: 'u4',
      assignee_name: 'Former User',
    }
    render(wrapper(<IssueMetaEditor issue={staleIssue} columns={columns} onChange={vi.fn()} />))
    fireEvent.keyDown(screen.getByLabelText(/assignee/i), { key: 'Enter' })
    const option = await screen.findByRole('option', { name: 'Former User' })
    expect(option).toHaveAttribute('data-disabled')
  })
})
