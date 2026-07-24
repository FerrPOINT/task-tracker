import { test, expect } from '@playwright/test'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'

const mockUser = {
  id: '00000000-0000-0000-0000-000000000001',
  key: 'DEMO',
  name: 'Demo Project',
  issueId: 'issue-1',
}

function routeJson(route: any, body: unknown, status = 200) {
  return route.fulfill({
    status,
    contentType: 'application/json',
    body: JSON.stringify(body),
  })
}

test.describe('smoke', () => {
  test('login then navigate through dashboard, projects, board and create issue', async ({ page }) => {
    await page.route('**/api/v1/auth/login', (route) =>
      routeJson(route, {
        access_token: 'demo-token',
        token_type: 'Bearer',
        user_id: mockUser.id,
        email: 'demo@example.com',
      }),
    )
    await page.route('**/api/v1/dashboard', (route) =>
      routeJson(route, { assigned_issues: [] }),
    )
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

    page.on('request', (req) => console.log('request', req.method(), req.url()))
    page.on('response', (res) => console.log('response', res.status(), res.url()))

    await page.goto(`${baseURL}/login`)
    await page.getByRole('textbox').nth(0).fill('demo@example.com')
    await page.getByRole('textbox').nth(1).fill('demo')
    await page.getByRole('button', { name: /войти/i }).click()

    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 10000 })
    await expect(page.getByRole('heading', { name: /dashboard|team dashboard|мои задачи/i })).toBeVisible()

    await page.goto(`${baseURL}/projects`)
    await expect(page.getByText(mockUser.name)).toBeVisible()

    await page.goto(`${baseURL}/projects/${mockUser.key}/board`)
    await expect(page.getByText('Smoke issue').first()).toBeVisible()
    await page.screenshot({ path: '/root/.hermes/cache/images/smoke-board.png' })
  })
})
