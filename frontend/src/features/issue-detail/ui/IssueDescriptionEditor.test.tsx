import { describe, it, expect, vi, beforeAll } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { ThemeProvider } from '@pdlc/ui/lib'
import i18n from '@/shared/i18n/config'
import { IssueDescriptionEditor } from './IssueDescriptionEditor'
import type { Issue } from '@/api/issue'
import type { RequirementsRevision } from '@/api/sdlc'

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
  it('shows one task description and edits only the title without rewriting the stored input', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined)
    const requirements: RequirementsRevision = {
      revision: 4,
      content_hash: 'a'.repeat(64),
      author_subject: 'pm',
      created_at: '2026-01-01T00:00:00Z',
      goal: 'Customers choose a quantity before buying',
      scope: ['Allow whole quantities from 1 to 20'],
      acceptance_criteria: ['Three items create one order for three items'],
      exclusions: [],
      scenarios: [],
      constraints: [],
      dependencies: [],
      assumptions: [],
      checklist: [],
      prerequisites: [],
    }
    render(
      wrapper(
        <IssueDescriptionEditor issue={issue} requirements={requirements} onSubmit={onSubmit} />,
      ),
    )
    expect(screen.getByText(requirements.goal)).toBeVisible()
    expect(screen.getByText('Allow whole quantities from 1 to 20')).toBeVisible()
    expect(screen.getByText('Three items create one order for three items')).toBeVisible()
    expect(screen.queryByText(issue.description!)).not.toBeInTheDocument()
    expect(document.querySelector('details')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Edit title' }))
    expect(screen.getAllByRole('textbox')).toHaveLength(1)
    expect(screen.getByText(requirements.goal)).toBeVisible()
    fireEvent.change(screen.getByRole('textbox', { name: /summary/i }), {
      target: { value: 'Updated title' },
    })
    expect(screen.queryByDisplayValue(requirements.goal)).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /save/i }))
    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith({ summary: 'Updated title' }))
  })
  it('shows the original description even when its text happens to be JSON', () => {
    const description = '{"reference":"original-ref","items":[1,2]}'
    render(wrapper(<IssueDescriptionEditor issue={{ ...issue, description }} onSubmit={vi.fn()} />))
    expect(screen.getByRole('heading', { name: 'Description' })).toBeVisible()
    expect(screen.getByText(description)).toBeVisible()
    expect(document.querySelector('details')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: /edit/i }))
    expect(screen.getByRole('textbox', { name: /^description$/i })).toHaveValue(description)
  })

  it('keeps invalid JSON visible as ordinary description text', () => {
    const description = '{not JSON: important user text}'
    render(wrapper(<IssueDescriptionEditor issue={{ ...issue, description }} onSubmit={vi.fn()} />))
    expect(screen.getByText(description)).toBeVisible()
  })
  it('renders summary and description in view mode', () => {
    render(wrapper(<IssueDescriptionEditor issue={issue} onSubmit={vi.fn()} />))
    expect(screen.getByText('Fix login bug')).toBeInTheDocument()
    expect(screen.getByText('Login page crashes on submit')).toBeInTheDocument()
  })

  it('shows the complete long description once without an expand action', () => {
    const longIssue = { ...issue, description: `${'Long description '.repeat(35)}END_MARKER` }
    render(wrapper(<IssueDescriptionEditor issue={longIssue} onSubmit={vi.fn()} />))

    expect(screen.getAllByText(longIssue.description)).toHaveLength(1)
    expect(screen.getByText(longIssue.description)).toBeVisible()
    expect(screen.queryByRole('button', { name: /description/i })).not.toBeInTheDocument()
    expect(screen.queryByDisplayValue(longIssue.summary)).not.toBeInTheDocument()
  })

  it('enters edit mode only through the edit action', () => {
    render(wrapper(<IssueDescriptionEditor issue={issue} onSubmit={vi.fn()} />))
    fireEvent.click(screen.getByText('Fix login bug'))
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /edit/i }))
    expect(screen.getByDisplayValue('Fix login bug')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /save/i })).toBeInTheDocument()
  })

  it('retains the edited summary and description when saving fails', async () => {
    const onSubmit = vi.fn().mockRejectedValue(new Error('Save failed'))
    render(wrapper(<IssueDescriptionEditor issue={issue} onSubmit={onSubmit} />))
    fireEvent.click(screen.getByRole('button', { name: /edit/i }))
    const summary = screen.getByRole('textbox', { name: /summary/i })
    fireEvent.change(summary, { target: { value: 'Revised summary' } })
    fireEvent.click(screen.getByRole('button', { name: /save/i }))
    await waitFor(() => expect(onSubmit).toHaveBeenCalledOnce())
    expect(await screen.findByRole('alert')).toHaveTextContent('Save failed')
    expect(summary).toHaveValue('Revised summary')
    expect(screen.getByRole('button', { name: /save/i })).toBeEnabled()
  })
})
