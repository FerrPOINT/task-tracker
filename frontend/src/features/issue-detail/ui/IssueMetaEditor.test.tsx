import { describe, it, expect, vi, beforeAll } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@/shared/lib/theme'
import i18n from '@/shared/i18n/config'
import { IssueMetaEditor } from './IssueMetaEditor'
import type { Issue } from '@/api/issue'
import type { Board } from '@/api/board'

beforeAll(() => {
  i18n.changeLanguage('en')
})

vi.mock('@/shared/api/hooks', () => ({
  useCurrentUser: () => ({
    data: { id: 'u1', username: 'alice', display_name: 'Alice', email: 'alice@test.com' },
    isLoading: false,
    error: null,
  }),
  useUsers: () => ({
    data: [
      { id: 'u1', username: 'alice', display_name: 'Alice', email: 'alice@test.com' },
      { id: 'u2', username: 'bob', display_name: 'Bob', email: 'bob@test.com' },
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
  it('renders metadata select fields', () => {
    render(wrapper(<IssueMetaEditor issue={issue} columns={columns} onChange={vi.fn()} />))
    expect(screen.getByText(/status/i)).toBeInTheDocument()
    expect(screen.getByText(/priority/i)).toBeInTheDocument()
    expect(screen.getByText(/assignee/i)).toBeInTheDocument()
  })

  it('calls onChange when priority is changed', () => {
    const onChange = vi.fn()
    render(wrapper(<IssueMetaEditor issue={issue} columns={columns} onChange={onChange} />))
    fireEvent.change(screen.getByDisplayValue('High'), { target: { value: 'Low' } })
    expect(onChange).toHaveBeenCalledWith({ priority: 'Low' })
  })
})