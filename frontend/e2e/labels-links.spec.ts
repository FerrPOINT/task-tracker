import { test, expect } from '@playwright/test'

test.describe('labels and links', () => {
  test('create label → attach to issue → detach', async ({ page, request }) => {
    const login = await request.post('/api/v1/auth/login', {
      data: { email: 'demo@example.com', password: 'demo' },
    })
    const { access_token: token } = await login.json()

    const created = await request.post('/api/v1/issues', {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        project_key: 'DEMO',
        issue_type: 'task',
        priority: 'medium',
        status_id: '00000000-0000-0000-0000-000000000001',
        summary: `Labels E2E ${Date.now()}`,
        reporter_id: '00000000-0000-0000-0000-000000000001',
      },
    })
    const issue = await created.json()

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

    // Create a new label through the editor
    await page.getByRole('button', { name: /новая метка|new label/i }).first().click()
    const labelName = `e2e-label-${Date.now() % 10000}`
    await page.getByTestId('label-name-input').fill(labelName)
    await page.getByRole('button', { name: /^добавить$|^add$/i }).click()
    await expect(page.getByTestId('issue-label').filter({ hasText: labelName })).toBeVisible({
      timeout: 10_000,
    })

    // Detach it
    await page.getByRole('button', { name: new RegExp(`убрать метку ${labelName}|remove label ${labelName}`, 'i') }).click()
    await expect(page.getByTestId('issue-label')).toHaveCount(0, { timeout: 10_000 })
  })

  test('create issue link → delete', async ({ page, request }) => {
    const login = await request.post('/api/v1/auth/login', {
      data: { email: 'demo@example.com', password: 'demo' },
    })
    const { access_token: token } = await login.json()

    const mk = async (summary: string) => {
      const res = await request.post('/api/v1/issues', {
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
      return res.json()
    }
    const a = await mk(`Link E2E A ${Date.now()}`)
    const b = await mk(`Link E2E B ${Date.now()}`)

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
    await page.goto(`/issues/${a.id}`)

    // Add link a blocks b
    await page.getByRole('button', { name: /добавить связь|add link/i }).first().click()
    await page.getByTestId('link-target-input').fill(b.key)
    await page.getByRole('combobox', { name: /тип связи|link type/i }).selectOption('blocks')
    await page.getByTestId('link-submit').click()
    await expect(
      page.getByTestId('link-editor').getByText('блокирует').first(),
    ).toBeVisible({ timeout: 10_000 })

    // Delete link
    await page.getByRole('button', { name: new RegExp(`удалить связь с ${b.key}|remove link to ${b.key}`, 'i') }).click()
    await expect(page.getByTestId('link-editor').getByText('блокирует')).toHaveCount(0, {
      timeout: 10_000,
    })
  })
})
