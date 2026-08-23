import { test, expect } from '@playwright/test'

test.describe('real-time board updates (SSE)', () => {
  test('issue created via API appears on open board without reload', async ({ page, context, request }) => {
    const login = await request.post('/api/v1/auth/login', {
      data: { email: 'demo@example.com', password: 'demo' },
    })
    const { access_token: token } = await login.json()

    // first tab: open the board
    await page.goto('/login')
    await page.evaluate(
      ([t]) => {
        localStorage.setItem(
          'task-tracker-auth',
          JSON.stringify({ state: { token: t }, version: 0 }),
        )
      },
      [token],
    )
    await page.goto('/projects/DEMO/board')
    await expect(page.getByText('To Do').first()).toBeVisible({ timeout: 15_000 })

    // second "tab": create an issue through the API (same as another user action)
    const summary = `SSE Live ${Date.now() % 100000}`
    const created = await request.post('/api/v1/issues', {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        project_key: 'DEMO',
        issue_type: 'task',
        priority: 'medium',
        status_id: '00000000-0000-0000-0000-000000000001',
        summary,
        reporter_id: '00000000-0000-0000-0000-000000000001',
      },
    })
    expect(created.ok()).toBeTruthy()

    // board should refresh via SSE invalidation — no page.reload()
    await expect(page.getByText(summary).first()).toBeVisible({ timeout: 15_000 })
  })
})
