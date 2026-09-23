import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter, Routes, Route, useLocation, useNavigate } from 'react-router'

import { IssueCreatePage } from './'
import { ThemeProvider } from '@sdlc/ui/lib'
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
      {
        id: 'p2',
        key: 'OPS',
        name: 'Operations',
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

function LocationProbe() {
  const location = useLocation()
  const navigate = useNavigate()

  return (
    <>
      <output aria-label="current location">{`${location.pathname}${location.search}`}</output>
      <button type="button" onClick={() => navigate(-1)}>
        Back in history
      </button>
    </>
  )
}

function wrapper(children: React.ReactNode, initialEntry = '/issues/create') {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return (
    <ThemeProvider>
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={[initialEntry]}>
          <Routes>
            <Route
              path="/issues/create"
              element={
                <>
                  {children}
                  <LocationProbe />
                </>
              }
            />
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
    expect(payload).not.toHaveProperty('status_id')
    expect(payload).toMatchObject({
      project_key: 'TT',
      summary: 'Test issue',
      custom_fields: { f1: 'custom value' },
    })
  })

  it('requires an explicit project choice when the URL points to an unavailable project', async () => {
    render(wrapper(<IssueCreatePage />, '/issues/create?project_key=OLD'))
    const user = userEvent.setup()

    await screen.findByText('Выбранный проект недоступен. Выберите другой проект.')
    await user.type(screen.getByLabelText(/Заголовок/), 'Issue')
    expect(screen.getByRole('button', { name: /^создать$/i })).toBeDisabled()
    expect(listCustomFields).not.toHaveBeenCalled()

    await user.selectOptions(screen.getByLabelText(/Проект/), 'TT')
    await user.type(await screen.findByLabelText(/Required text/), 'ready')
    await user.click(screen.getByRole('button', { name: /^создать$/i }))
    await waitFor(() => expect(createIssue).toHaveBeenCalled())
    expect(createIssue.mock.calls[0]?.[0]).toMatchObject({ project_key: 'TT' })
  })

  it('blocks submission while required project fields failed to load, then retries', async () => {
    listCustomFields.mockRejectedValueOnce(new Error('Failed to load custom fields'))
    const user = userEvent.setup()
    render(wrapper(<IssueCreatePage />))

    await user.type(screen.getByLabelText(/Заголовок/), 'Issue')
    await screen.findByText('Не удалось загрузить поля проекта. Повторите попытку.')
    expect(screen.getByRole('button', { name: /^создать$/i })).toBeDisabled()
    expect(createIssue).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: /повторить/i }))
    await user.type(await screen.findByLabelText(/Required text/), 'ready')
    await user.click(screen.getByRole('button', { name: /^создать$/i }))
    await waitFor(() => expect(createIssue).toHaveBeenCalled())
  })

  it('does not substitute hardcoded issue types after a loading error', async () => {
    listIssueTypes.mockRejectedValueOnce(new Error('Failed to load issue types'))
    render(wrapper(<IssueCreatePage />))

    await screen.findByText('Не удалось загрузить типы задач. Повторите попытку.')
    expect(screen.getByRole('combobox', { name: /Тип задачи/ })).toBeDisabled()
    expect(screen.getByRole('button', { name: /^создать$/i })).toBeDisabled()
    expect(screen.queryByRole('option', { name: 'Task' })).not.toBeInTheDocument()
  })

  it('allows an unassigned issue when the assignee directory is unavailable', async () => {
    listUsers.mockRejectedValueOnce(new Error('Failed to load users'))
    const user = userEvent.setup()
    render(wrapper(<IssueCreatePage />))

    await screen.findByText(
      'Не удалось загрузить исполнителей. Задачу можно создать без назначения.',
    )
    expect(screen.getByRole('combobox', { name: /Исполнитель/ })).toBeDisabled()
    await user.type(screen.getByLabelText(/Заголовок/), 'Issue')
    await user.type(await screen.findByLabelText(/Required text/), 'ready')
    await user.click(screen.getByRole('button', { name: /^создать$/i }))
    await waitFor(() => expect(createIssue).toHaveBeenCalled())
    expect(createIssue.mock.calls[0]?.[0]).toMatchObject({ assignee_id: null })
  })

  it('keeps the draft and explains a failed save without exposing API text', async () => {
    createIssue.mockRejectedValueOnce(new Error('Failed to create issue'))
    const user = userEvent.setup()
    render(wrapper(<IssueCreatePage />))

    await user.type(screen.getByLabelText(/Заголовок/), 'Draft issue')
    await user.type(await screen.findByLabelText(/Required text/), 'ready')
    await user.click(screen.getByRole('button', { name: /^создать$/i }))

    await screen.findByText('Не удалось создать задачу. Проверьте данные и повторите попытку.')
    expect(screen.getByLabelText(/Заголовок/)).toHaveValue('Draft issue')
    expect(screen.getByLabelText(/Required text/)).toHaveValue('ready')
    expect(screen.getByRole('button', { name: /^создать$/i })).toBeEnabled()
  })

  it('stores project changes in URL history and preserves unrelated parameters', async () => {
    const user = userEvent.setup()
    render(wrapper(<IssueCreatePage />, '/issues/create?project_key=TT&source=board'))

    await screen.findByRole('option', { name: /Operations/ })
    await user.selectOptions(screen.getByLabelText(/Проект/), 'OPS')
    expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
      '/issues/create?project_key=OPS&source=board',
    )

    await user.click(screen.getByRole('button', { name: 'Back in history' }))
    await waitFor(() => expect(screen.getByLabelText(/Проект/)).toHaveValue('TT'))
    expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
      '/issues/create?project_key=TT&source=board',
    )
  })

  it('writes the fallback project to the URL without losing other parameters', async () => {
    render(wrapper(<IssueCreatePage />, '/issues/create?source=navigation'))

    await waitFor(() => expect(screen.getByLabelText(/Проект/)).toHaveValue('TT'))
    await waitFor(() =>
      expect(screen.getByRole('status', { name: 'current location' })).toHaveTextContent(
        '/issues/create?source=navigation&project_key=TT',
      ),
    )
  })
})
