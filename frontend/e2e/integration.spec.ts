import { test, expect } from '@playwright/test'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'
const apiURL = process.env.PLAYWRIGHT_API_URL ?? 'http://127.0.0.1:3456'

const credentials = {
  email: 'demo@example.com',
  password: 'demo',
}

async function login(page: any) {
  await page.goto(`${baseURL}/login`)
  await page.getByRole('textbox').nth(0).fill(credentials.email)
  await page.getByRole('textbox').nth(1).fill(credentials.password)
  await page.getByRole('button', { name: /войти|login/i }).click()
  await expect(page).toHaveURL(`${baseURL}/`, { timeout: 10000 })
}

test.describe('integration against live backend', () => {
  test('login then navigate through dashboard, projects, board, backlog, search, create issue', async ({ page }) => {
    await login(page)

    await expect(page.getByRole('heading', { name: /dashboard|мои задачи|team dashboard/i })).toBeVisible()

    await page.goto(`${baseURL}/projects`)
    await expect(page.getByRole('heading', { name: /проекты|projects/i })).toBeVisible()
    await expect(page.getByText('Task Tracker').first()).toBeVisible()
    await expect(page.getByText('Demo Project').first()).toBeVisible()

    await page.goto(`${baseURL}/projects/DEMO/board`)
    await expect(page.getByText('Todo').first()).toBeVisible()
    await expect(page.getByText('In Progress').first()).toBeVisible()
    await expect(page.getByText('Done').first()).toBeVisible()

    await page.goto(`${baseURL}/projects/DEMO/backlog`)
    await expect(page.getByRole('heading', { name: /бэклог|backlog/i })).toBeVisible()
    await expect(page.getByRole('link').first()).toBeVisible()

    await page.goto(`${baseURL}/search`)
    await page.getByRole('textbox').fill('DEMO')
    await page.getByRole('button', { name: /искать|search/i }).click()
    await expect(page.getByText('DEMO').first()).toBeVisible()

    await page.goto(`${baseURL}/issues/create`)
    await expect(page.getByRole('heading', { name: /создать задачу|new issue/i })).toBeVisible()
    await page.getByRole('combobox').first().selectOption('DEMO')
    await page.getByRole('textbox').nth(0).fill('Integration test issue')
    await page.getByRole('textbox').nth(1).fill('Created by Playwright integration spec')
    await page.getByRole('button', { name: /создать$/i }).click()
    await expect(page).toHaveURL(/\/projects\/DEMO\/backlog/, { timeout: 10000 })
    await expect(page.getByText('Integration test issue')).toBeVisible()
  })
})
