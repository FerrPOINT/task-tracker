import { test, expect, Page } from '@playwright/test'
import { seedIntegrationData } from './setup'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'

const credentials = {
  email: 'demo@example.com',
  password: 'demo',
}

async function login(page: Page) {
  await page.goto(`${baseURL}/login`)
  await page.getByRole('textbox').nth(0).fill(credentials.email)
  await page.getByRole('textbox').nth(1).fill(credentials.password)
  await page.getByRole('button', { name: /войти|login/i }).click()
  await expect(page).toHaveURL(`${baseURL}/`, { timeout: 10000 })
}

test.describe('integration against live backend', () => {
  test.beforeAll(async () => {
    await seedIntegrationData()
  })

  test('login then navigate through dashboard, projects, board, backlog, search, create issue', async ({
    page,
  }) => {
    const ctx = await seedIntegrationData()
    await login(page)

    await expect(
      page.getByRole('heading', { name: /dashboard|мои задачи|team dashboard|командный дашборд/i }),
    ).toBeVisible()

    await page.goto(`${baseURL}/projects`)
    await expect(page.getByRole('heading', { name: /проекты|projects/i })).toBeVisible()
    await expect(page.getByText('Demo Project').first()).toBeVisible()

    await page.goto(`${baseURL}/projects/DEMO/board`)
    await expect(page.getByText('To Do').first()).toBeVisible()
    await expect(page.getByText('In Progress').first()).toBeVisible()
    await expect(page.getByText('Done').first()).toBeVisible()

    await page.goto(`${baseURL}/projects/DEMO/backlog`)
    await expect(page.getByRole('heading', { name: /бэклог|backlog/i })).toBeVisible()
    await expect(page.getByRole('link').first()).toBeVisible()

    await page.goto(`${baseURL}/projects/DEMO/board`)
    const todoCard = page.getByText(ctx.issueKey).first()
    await expect(todoCard).toBeVisible()
    // Board move is triggered by ctrl+click on the issue card
    await todoCard.click({ modifiers: ['Control'] })
    await expect(page.getByText(ctx.issueKey).first()).toBeVisible()

    await page.goto(`${baseURL}/search`)
    await page.getByPlaceholder(/поиск задач|search issues/i).fill('Smoke')
    // Debounced URL-param search: wait for results without a submit button
    await expect(page.getByText('Smoke issue').first()).toBeVisible({ timeout: 10_000 })

    const issueSummary = `Integration test issue ${Date.now()}`
    await page.goto(`${baseURL}/issues/create`)
    await expect(page.getByRole('heading', { name: /создать задачу|new issue/i })).toBeVisible()
    await page.getByRole('combobox').first().selectOption('DEMO')
    await page.getByRole('textbox').nth(0).fill(issueSummary)
    await page.getByRole('textbox').nth(1).fill('Created by Playwright integration spec')
    await page.getByRole('button', { name: /создать$/i }).click()
    await expect(page).toHaveURL(/\/projects\/DEMO\/backlog/, { timeout: 10000 })
    await expect(page.getByText(issueSummary)).toBeVisible()
  })
})
