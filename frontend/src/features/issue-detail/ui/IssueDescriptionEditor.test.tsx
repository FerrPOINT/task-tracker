import { describe, it, expect, vi, beforeAll } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@sdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { IssueDescriptionEditor } from './IssueDescriptionEditor'
import type { Issue } from '@/api/issue'

beforeAll(() => {
  i18n.changeLanguage('en')
})

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
  description: 'Login page crashes on submit',
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

describe('IssueDescriptionEditor', () => {
  it('renders summary and description in view mode', () => {
    render(wrapper(<IssueDescriptionEditor issue={issue} onSubmit={vi.fn()} />))
    expect(screen.getByText('Fix login bug')).toBeInTheDocument()
    expect(screen.getByText('Login page crashes on submit')).toBeInTheDocument()
  })

  it('enters edit mode on click', () => {
    render(wrapper(<IssueDescriptionEditor issue={issue} onSubmit={vi.fn()} />))
    fireEvent.click(screen.getByText('Fix login bug'))
    expect(screen.getByDisplayValue('Fix login bug')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /save/i })).toBeInTheDocument()
  })
})
