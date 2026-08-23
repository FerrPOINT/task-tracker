import { test, expect } from '@playwright/test'

test.describe('attachments', () => {
  test('upload → list → delete attachment on issue detail', async ({ page, request }) => {
    // Seed: login + create issue via API
    const login = await request.post('/api/v1/auth/login', {
      data: { email: 'demo@example.com', password: 'demo' },
    })
    expect(login.ok()).toBeTruthy()
    const { access_token: token } = await login.json()

    const created = await request.post('/api/v1/issues', {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        project_key: 'DEMO',
        issue_type: 'task',
        priority: 'medium',
        status_id: '00000000-0000-0000-0000-000000000001',
        summary: `Attachment E2E ${Date.now()}`,
        reporter_id: '00000000-0000-0000-0000-000000000001',
      },
    })
    expect(created.ok()).toBeTruthy()
    const issue = await created.json()

    // Inject auth into localStorage then reload (zustand persist rehydrates async)
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
    await page.goto(`/issues/${issue.id}`)

    // Open attachments tab
    await page.getByRole('tab', { name: /вложения|attachments/i }).click()

    // Upload a file
    await page.setInputFiles('input[data-testid="attachment-input"]', {
      name: 'e2e-note.txt',
      mimeType: 'text/plain',
      buffer: Buffer.from('e2e attachment content'),
    })
    await expect(page.getByText('e2e-note.txt')).toBeVisible({ timeout: 10_000 })

    // Delete it
    await page.getByRole('button', { name: /удалить e2e-note\.txt|delete e2e-note\.txt/i }).click()
    await expect(page.getByText('e2e-note.txt')).not.toBeVisible({ timeout: 10_000 })
  })
})
