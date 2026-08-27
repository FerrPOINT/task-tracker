import { test, expect } from '@playwright/test'
import { apiLogin, apiGet, apiPost } from './setup'

test.describe('project members', () => {
test.setTimeout(120_000)
  test('add member → list → remove via board panel', async ({ page }) => {
    // register a fresh user to invite (single register per run; retried on 429)
    const username = `e2emember${Date.now() % 100000}`
    const displayName = `E2E Member ${Date.now() % 100000}`
    const email = `${username}@example.com`
    const reg = await apiPost('/auth/register', {
      email,
      username,
      name: displayName,
      password: 'Secret12345',
    })
    expect([201, 409]).toContain(reg.status)
    const newUserId: string = reg.data.user_id

    const login = await apiLogin()
    expect(login.status).toBe(200)
    const token: string = login.data.access_token

    // members panel works by project key; DEMO must exist (seed guarantees it)
    const projects = await apiGet('/projects', token)
    const demo = projects.data.projects.find((p: { key: string }) => p.key === 'DEMO')
    expect(demo).toBeTruthy()

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

    // open members panel
    await page
      .getByRole('button', { name: /участники|members/i })
      .first()
      .click()
    const combo = page.getByRole('combobox')
    await expect(combo).toBeVisible({ timeout: 10_000 })

    // add the new user
    await combo.selectOption(newUserId)
    await page.locator('form button[type="submit"]').click()
    // The member list shows display_name; the <option> is hidden but the row is visible
    await expect(
      page.getByRole('dialog').getByText(displayName, { exact: true }).first(),
    ).toBeVisible({ timeout: 10_000 })

    // remove — the delete button is aria-labelled with the member name
    await page
      .getByRole('button', {
        name: new RegExp(`удалить.*${displayName}|remove.*${displayName}`, 'i'),
      })
      .click()
    await expect(page.getByRole('dialog').getByText(displayName, { exact: true })).toHaveCount(0, {
      timeout: 10_000,
    })
  })
})
