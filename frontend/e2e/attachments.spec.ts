import { test, expect } from '@playwright/test'
import { apiPost, authenticatePage } from './setup'

test.describe('attachments', () => {
  test.setTimeout(120_000)
  test('upload → list → delete attachment on issue detail', async ({ page }) => {
    // Seed: login + create issue via API (shared seed keeps auth rate-limit happy)
    const auth = await authenticatePage(page)
    const token = auth.access_token

    const created = await apiPost(
      '/issues',
      {
        project_key: 'DEMO',
        issue_type: 'task',
        priority: 'medium',
        summary: `Attachment E2E ${Date.now()}`,
        reporter_id: auth.user_id,
      },
      token,
    )
    expect([200, 201]).toContain(created.status)
    const issue = created.data

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
