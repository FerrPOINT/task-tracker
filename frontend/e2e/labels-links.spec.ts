import { test, expect } from '@playwright/test'
import { apiLogin, apiPost } from './setup'

test.describe('labels and links', () => {
  test('create label → attach to issue → detach', async ({ page }) => {
    const login = await apiLogin()
    expect(login.status).toBe(200)
    const token: string = login.data.access_token

    const created = await apiPost(
      '/issues',
      {
        project_key: 'DEMO',
        issue_type: 'task',
        priority: 'medium',
        summary: `Labels E2E ${Date.now()}`,
        reporter_id: login.data.user_id,
      },
      token,
    )
    expect([200, 201]).toContain(created.status)
    const issue = created.data

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
    await page
      .getByRole('button', { name: /новая метка|new label/i })
      .first()
      .click()
    const labelName = `e2e-label-${Date.now() % 10000}`
    await page.getByTestId('label-name-input').fill(labelName)
    await page.getByRole('button', { name: /^добавить$|^add$/i }).click()
    await expect(page.getByTestId('issue-label').filter({ hasText: labelName })).toBeVisible({
      timeout: 10_000,
    })

    // Detach it
    await page
      .getByRole('button', {
        name: new RegExp(`убрать метку ${labelName}|remove label ${labelName}`, 'i'),
      })
      .click()
    await expect(page.getByTestId('issue-label')).toHaveCount(0, { timeout: 10_000 })
  })

  test('create issue link → delete', async ({ page }) => {
    const login = await apiLogin()
    expect(login.status).toBe(200)
    const token: string = login.data.access_token

    const mk = async (summary: string) => {
      const res = await apiPost(
        '/issues',
        {
          project_key: 'DEMO',
          issue_type: 'task',
          priority: 'medium',
          summary,
          reporter_id: login.data.user_id,
        },
        token,
      )
      expect([200, 201]).toContain(res.status)
      return res.data
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
    await page
      .getByRole('button', { name: /добавить связь|add link/i })
      .first()
      .click()
    await page.getByTestId('link-target-input').fill(b.key)
    await page.getByRole('combobox', { name: /тип связи|link type/i }).selectOption('blocks')
    await page.getByTestId('link-submit').click()
    await expect(page.getByTestId('link-editor').getByText('блокирует').first()).toBeVisible({
      timeout: 10_000,
    })

    // Delete link
    await page
      .getByRole('button', {
        name: new RegExp(`удалить связь с ${b.key}|remove link to ${b.key}`, 'i'),
      })
      .click()
    await expect(page.getByTestId('link-editor').getByText('блокирует')).toHaveCount(0, {
      timeout: 10_000,
    })
  })
})
