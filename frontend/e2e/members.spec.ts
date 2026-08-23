import { test, expect } from '@playwright/test'

test.describe('project members', () => {
  test('add member → list → remove via board panel', async ({ page, request }) => {
    // register a fresh user to invite
    const username = `e2emember${Date.now() % 100000}`
    const email = `${username}@example.com`
    const reg = await request.post('/api/v1/auth/register', {
      data: {
        email,
        username,
        name: 'E2E Member',
        password: 'secret123',
      },
    })
    expect(reg.ok()).toBeTruthy()
    const newUser = await reg.json()
    const newUserId: string = newUser.user_id

    const login = await request.post('/api/v1/auth/login', {
      data: { email: 'demo@example.com', password: 'demo' },
    })
    const { access_token: token } = await login.json()

    // find DEMO project id
    const projects = await request.get('/api/v1/projects', {
      headers: { Authorization: `Bearer ${token}` },
    })
    const projectList = (await projects.json()).projects
    const demo = projectList.find((p: { key: string }) => p.key === 'DEMO')
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
    await page.getByRole('button', { name: /участники|members/i }).first().click()
    const combo = page.getByRole('combobox')
    // option presence is verified implicitly by label-based selection below
    await expect(combo).toBeVisible({ timeout: 10_000 })

    // add the new user
    await combo.selectOption(newUserId)
    await page.locator('form button[type="submit"]').click()
    await expect(
      page.getByRole('dialog').getByText(username, { exact: false }).first(),
    ).toBeVisible({ timeout: 10_000 })

    // remove
    await page.getByRole('button', { name: new RegExp(username) }).click()
    // member row disappears; username only remains inside the hidden <option> of the select
    await expect(
      page.getByRole('dialog').getByText(username, { exact: true }),
    ).toHaveCount(0, { timeout: 10_000 })
  })
})
