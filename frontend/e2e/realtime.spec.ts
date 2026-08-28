import { test, expect } from '@playwright/test'
import { apiPost, authenticatePage } from './setup'

test.describe('real-time board updates (SSE)', () => {
  test.setTimeout(120_000)
  test('issue created via API appears on open board without reload', async ({ page }) => {
    const auth = await authenticatePage(page)
    const token = auth.access_token

    // first tab: open the board
    await page.goto('/projects/DEMO/board')
    await expect(page.getByText('To Do').first()).toBeVisible({ timeout: 15_000 })

    // second "tab": create an issue through the API (same as another user action)
    const summary = `SSE Live ${Date.now() % 100000}`
    const created = await apiPost(
      '/issues',
      {
        project_key: 'DEMO',
        issue_type: 'task',
        priority: 'medium',
        summary,
        reporter_id: auth.user_id,
      },
      token,
    )
    expect([200, 201]).toContain(created.status)

    // board should refresh via SSE invalidation — no page.reload()
    await expect(page.getByText(summary).first()).toBeVisible({ timeout: 15_000 })
  })
})
