import { test, expect, Route } from '@playwright/test'

const mockUser = {
  id: '00000000-0000-0000-0000-000000000001',
  key: 'DEMO',
  name: 'Demo Project',
  issueId: 'issue-1',
}

function routeJson(route: Route, body: unknown, status = 200) {
  return route.fulfill({
    status,
    contentType: 'application/json',
    body: JSON.stringify(body),
  })
}

test.describe('smoke', () => {
  test('login then navigate through dashboard, projects, board and create issue', async ({
    page,
    baseURL,
  }, testInfo) => {
    const appBaseURL = baseURL ?? 'http://127.0.0.1:4173'
    await page.route('**/api/v1/auth/login', (route) =>
      routeJson(route, {
        access_token: 'demo-token',
        token_type: 'Bearer',
        user_id: mockUser.id,
        email: 'demo@example.com',
      }),
    )
    await page.route('**/api/v1/dashboard', (route) => routeJson(route, { assigned_issues: [] }))
    await page.route('**/api/v1/users/me', (route) =>
      routeJson(route, {
        id: mockUser.id,
        email: 'demo@example.com',
        username: 'demo',
        display_name: 'Demo User',
      }),
    )
    await page.route('**/api/v1/statuses', (route) =>
      routeJson(route, [
        {
          id: 'todo',
          name: 'To Do',
          category: 'todo',
          position: 0,
          is_default: true,
          is_closed: false,
        },
        {
          id: 'inprogress',
          name: 'In Progress',
          category: 'inprogress',
          position: 1,
          is_default: false,
          is_closed: false,
        },
        {
          id: 'done',
          name: 'Done',
          category: 'done',
          position: 2,
          is_default: false,
          is_closed: true,
        },
      ]),
    )
    await page.route('**/api/v1/transitions', (route) => routeJson(route, []))
    await page.route('**/api/v1/users', (route) => routeJson(route, []))
    await page.route('**/api/v1/issue-types', (route) => routeJson(route, []))
    await page.route('**/api/v1/projects', (route) =>
      routeJson(route, {
        projects: [
          {
            id: '00000000-0000-0000-0000-000000000010',
            key: mockUser.key,
            name: mockUser.name,
            description: 'Smoke test project',
            owner_id: mockUser.id,
            todo_count: 1,
            in_progress_count: 0,
            done_count: 0,
          },
        ],
      }),
    )
    await page.route('**/api/v1/projects/*/members', (route) => routeJson(route, { members: [] }))
    await page.route('**/api/v1/projects/*/board', (route) =>
      routeJson(route, {
        columns: [
          { id: 'todo', name: 'To Do', wip_limit: null, issue_ids: [mockUser.issueId] },
          { id: 'inprogress', name: 'In Progress', wip_limit: null, issue_ids: [] },
          { id: 'done', name: 'Done', wip_limit: null, issue_ids: [] },
        ],
        issues: [
          {
            id: mockUser.issueId,
            key: `${mockUser.key}-1`,
            summary: 'Smoke issue',
            description: '',
            issue_type: 'Task',
            status: 'To Do',
            priority: 'Medium',
            labels: [],
            assignee_id: null,
            assignee_name: null,
            reporter_id: mockUser.id,
            reporter_name: 'Demo User',
            project_name: mockUser.name,
          },
        ],
        sprint: {
          id: 'sprint-1',
          name: 'Sprint 1',
          goal: '',
          state: 'active',
          velocity: 0,
          remaining_days: 14,
          issue_ids: [mockUser.issueId],
        },
      }),
    )
    await page.route('**/api/v1/projects/*/backlog**', (route) =>
      routeJson(route, {
        project_id: '00000000-0000-0000-0000-000000000010',
        project_key: mockUser.key,
        sprint: {
          id: 'sprint-1',
          name: 'Sprint 1',
          goal: '',
          state: 'active',
          issue_ids: [],
          velocity: 0,
          remaining_days: 14,
          start_date: null,
          end_date: null,
        },
        sprint_issues: [],
        backlog_issues: [],
        backlog_total: 0,
        backlog_offset: 0,
        backlog_limit: 100,
      }),
    )
    await page.route('**/api/v1/projects/*/sprints', (route) =>
      routeJson(route, {
        sprints: [
          {
            id: 'sprint-1',
            name: 'Sprint 1',
            goal: '',
            state: 'active',
            issue_ids: [],
            velocity: 0,
            remaining_days: 14,
            start_date: null,
            end_date: null,
          },
        ],
      }),
    )
    await page.route('**/api/v1/projects/*/custom-fields', (route) =>
      routeJson(route, { fields: [] }),
    )
    await page.route('**/api/v1/projects/*/labels', (route) => routeJson(route, { labels: [] }))
    await page.route(`**/api/v1/issues/${mockUser.issueId}`, (route) =>
      routeJson(route, {
        id: mockUser.issueId,
        key: `${mockUser.key}-1`,
        summary: 'Smoke issue',
        description: 'Issue detail smoke description',
        issue_type: 'Task',
        status: 'To Do',
        status_id: 'todo',
        priority: 'Medium',
        labels: [],
        assignee_id: null,
        assignee_name: null,
        reporter_id: '00000000-0000-0000-0000-000000000002',
        reporter_name: 'Reporter User',
        project_key: mockUser.key,
        project_name: mockUser.name,
        sprint_id: null,
        original_estimate_seconds: null,
        remaining_estimate_seconds: null,
        time_spent_seconds: 0,
      }),
    )
    await page.route(`**/api/v1/issues/${mockUser.issueId}/comments**`, (route) =>
      routeJson(route, { comments: [] }),
    )
    await page.route(`**/api/v1/issues/${mockUser.issueId}/worklogs**`, (route) =>
      routeJson(route, { worklogs: [] }),
    )
    await page.route(`**/api/v1/issues/${mockUser.issueId}/attachments`, (route) =>
      routeJson(route, { attachments: [] }),
    )
    await page.route(`**/api/v1/issues/${mockUser.issueId}/labels`, (route) =>
      routeJson(route, { labels: [] }),
    )
    await page.route(`**/api/v1/issues/${mockUser.issueId}/links`, (route) =>
      routeJson(route, { links: [] }),
    )
    await page.route(`**/api/v1/issues/${mockUser.issueId}/custom-fields`, (route) =>
      routeJson(route, { values: [] }),
    )
    await page.route(`**/api/v1/issues/${mockUser.issueId}/votes`, (route) =>
      routeJson(route, {
        count: 1,
        votes: [
          {
            user_id: '00000000-0000-0000-0000-000000000003',
            username: 'voter',
            display_name: 'Voter User',
            voted_at: '2026-09-01T10:00:00Z',
          },
        ],
      }),
    )
    await page.route(`**/api/v1/issues/${mockUser.issueId}/watchers`, (route) =>
      routeJson(route, {
        watchers: [
          {
            user_id: mockUser.id,
            username: 'demo',
            display_name: 'Demo User',
          },
        ],
      }),
    )
    await page.route('**/api/v1/notifications', (route) =>
      routeJson(route, { notifications: [], unread_count: 0 }),
    )
    await page.route('**/api/v1/events**', (route) => routeJson(route, ''))
    await page.route('**/api/v1/auth/refresh', (route) =>
      routeJson(route, {
        access_token: 'demo-token',
        token_type: 'Bearer',
        user_id: mockUser.id,
        email: 'demo@example.com',
      }),
    )
    await page.route('**/api/v1/notifications**', (route) =>
      routeJson(route, { notifications: [], unread_count: 0 }),
    )

    await page.goto(`${appBaseURL}/login`)
    await page.getByRole('textbox').nth(0).fill('demo@example.com')
    await page.getByRole('textbox').nth(1).fill('demo')
    await page.getByRole('button', { name: /sign in|войти/i }).click()

    await expect(page).toHaveURL(`${appBaseURL}/`, { timeout: 10000 })
    await expect(
      page.getByRole('heading', { name: /dashboard|team dashboard|мои задачи|командный дашборд/i }),
    ).toBeVisible()

    await page.goto(`${appBaseURL}/projects`)
    await expect(page.getByText(mockUser.name)).toBeVisible()

    await page.goto(`${appBaseURL}/projects/${mockUser.key}/board`)
    await expect(page.getByText('Smoke issue').first()).toBeVisible()

    await page.goto(`${appBaseURL}/projects/${mockUser.key}/backlog`)
    await expect(page.getByRole('heading', { name: /backlog|бэклог/i })).toBeVisible()
    const createLinks = page.locator('main a[href^="/issues/create"]')
    await expect(createLinks).toHaveCount(2)
    await createLinks.last().click()
    await expect(page).toHaveURL(`${appBaseURL}/issues/create?project_key=${mockUser.key}`)
    await expect(page.locator('#issue-project')).toHaveValue(mockUser.key)

    await page.goto(`${appBaseURL}/projects/${mockUser.key}/board`)
    await expect(page.getByText('Smoke issue').first()).toBeVisible()
    await page.goto(`${appBaseURL}/issues/${mockUser.issueId}`)
    await expect(page.getByText('Issue detail smoke description')).toBeVisible()
    await expect(page.getByRole('button', { name: /vote|голос/i })).toBeVisible()
    await expect(
      page.getByRole('button', { name: /stop watching|перестать следить/i }),
    ).toBeVisible()
    await expect(page.getByText(/1 total|всего 1/i)).toBeVisible()
    await page.screenshot({ path: testInfo.outputPath('smoke-board.png') })
  })
})
