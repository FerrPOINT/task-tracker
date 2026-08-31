import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter, Routes, Route } from 'react-router'

import { IssueCreatePage } from './'
import { ThemeProvider } from '@/shared/lib/theme'
import { useAuthStore } from '@/shared/auth/store'

const createIssue = vi.hoisted(() =>
  vi.fn((input: Record<string, unknown>) => {
    void input
    return Promise.resolve({ id: 'new', project_key: 'TT' })
  }),
)
const listProjects = vi.hoisted(() =>
  vi.fn(() =>
    Promise.resolve([
      {
        id: 'p1',
        key: 'TT',
        name: 'Task Tracker',
        description: '',
        owner_id: 'u1',
        owner_name: 'Alice',
        created_at: '2026-08-01T00:00:00Z',
        todo_count: 0,
        in_progress_count: 0,
        done_count: 0,
      },
    ]),
  ),
)
const listUsers = vi.hoisted(() =>
  vi.fn(() => Promise.resolve([{ id: 'u2', username: 'bob', display_name: 'Bob' }])),
)
const listProjectMembers = vi.hoisted(() =>
  vi.fn(() => Promise.resolve({ members: [{ project_id: 'p1', user_id: 'u2', role: 'member' }] })),
)
const listIssueTypes = vi.hoisted(() =>
  vi.fn(() =>
    Promise.resolve([{ id: 'it1', name: 'Task', description: '', icon: '', is_subtask: false }]),
  ),
)
const listCustomFields = vi.hoisted(() =>
  vi.fn(() =>
    Promise.resolve([
      {
        id: 'f1',
        project_id: 'p1',
        name: 'Required text',
        field_type: 'text',
        options: [],
        is_required: true,
        created_at: '2026-08-01T00:00:00Z',
      },
    ]),
  ),
)
vi.mock('@/api/issue-create', () => ({
  createIssue,
}))
vi.mock('@/api/project', () => ({
  listProjects,
  createProject: vi.fn(),
  updateProject: vi.fn(),
  deleteProject: vi.fn(),
}))
vi.mock('@/api/auth', () => ({
  login: vi.fn(),
  register: vi.fn(),
  getCurrentUser: vi.fn(),
  listUsers,
  logout: vi.fn(),
}))
vi.mock('@/api/members', () => ({
  listProjectMembers,
  addProjectMember: vi.fn(),
  removeProjectMember: vi.fn(),
}))
vi.mock('@/api/workflow', () => ({
  listStatuses: vi.fn(),
  listTransitions: vi.fn(),
  listIssueTypes,
}))
vi.mock('@/api/custom-fields', () => ({
  createCustomField: vi.fn(),
  deleteCustomField: vi.fn(),
  listCustomFields,
  listIssueCustomFieldValues: vi.fn(),
  setIssueCustomFieldValue: vi.fn(),
}))

function wrapper(children: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return (
    <ThemeProvider>
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={['/issues/create']}>
          <Routes>
            <Route path="/issues/create" element={children} />
            <Route path="/projects/:key/backlog" element={<div>Backlog</div>} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>
    </ThemeProvider>
  )
}

describe('IssueCreatePage', () => {
  beforeEach(() => {
    createIssue.mockClear()
    listProjects.mockClear()
    listUsers.mockClear()
    listProjectMembers.mockClear()
    listIssueTypes.mockClear()
    listCustomFields.mockClear()
    useAuthStore.setState({ token: 'tok', userId: 'u1', email: 'a@b' })
  })

  it('creates issue and navigates', async () => {
    render(wrapper(<IssueCreatePage />))
    await waitFor(() => expect(screen.getByText('Создать задачу')).toBeInTheDocument())

    const summary = screen.getByPlaceholderText(/Краткое описание задачи/i) as HTMLInputElement
    await userEvent.clear(summary)
    await userEvent.type(summary, 'Test issue')
    await userEvent.type(await screen.findByLabelText(/Required text/), 'custom value')

    const submit = screen.getByRole('button', { name: /^создать$/i })
    await userEvent.click(submit)

    await waitFor(() => expect(createIssue).toHaveBeenCalled())
    const firstCall = createIssue.mock.calls[0]
    if (!firstCall) throw new Error('createIssue was not called')
    const payload = firstCall[0]
    expect(payload).not.toHaveProperty('reporter_id')
    expect(payload).toMatchObject({
      project_key: 'TT',
      summary: 'Test issue',
      custom_fields: { f1: 'custom value' },
    })
  })
})
