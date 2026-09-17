import { test, expect, Page } from '@playwright/test'
import { seedIntegrationData, uiLogin } from './setup'

const login = (page: Page) => uiLogin(page)

test.setTimeout(120_000)

test.describe('integration against live backend', () => {
  test.beforeAll(async () => {
    test.setTimeout(180_000)
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

    await page.getByRole('button', { name: /проекты|projects/i }).click()
    await page.getByRole('menuitem', { name: /все проекты|all projects/i }).click()
    await expect(page.getByRole('heading', { name: /проекты|projects/i })).toBeVisible()
    await expect(page.getByText('Demo Project').first()).toBeVisible()

    await page
      .getByRole('link', { name: /Demo Project/i })
      .first()
      .click()
    await expect(page.getByText(/К выполнению|To Do/).first()).toBeVisible()
    await expect(page.getByText(/В работе|In Progress/).first()).toBeVisible()
    await expect(page.getByText(/Готово|Done/).first()).toBeVisible()

    await page
      .getByRole('link', { name: /бэклог|backlog/i })
      .first()
      .click()
    await expect(page.getByRole('heading', { name: /бэклог|backlog/i })).toBeVisible()
    await expect(page.getByRole('link').first()).toBeVisible()

    await page.getByRole('link', { name: /доска|board/i }).click()
    const todoCard = page.getByText(ctx.issueKey).first()
    await expect(todoCard).toBeVisible()
    const issueCard = page.locator('article').filter({ hasText: ctx.issueKey })
    await issueCard.getByRole('button', { name: /изменить статус|change status/i }).click()
    await page.getByRole('menuitem', { name: /в работе|in progress/i }).click()
    await expect(page.getByText(ctx.issueKey).first()).toBeVisible()

    await page.getByRole('link', { name: /поиск|search/i }).click()
    await page.getByPlaceholder(/поиск задач|search issues/i).fill('Smoke')
    // Debounced URL-param search: wait for results without a submit button
    await expect(page.getByText('Smoke issue').first()).toBeVisible({ timeout: 10_000 })

    const issueSummary = `Integration test issue ${Date.now()}`
    await page
      .getByRole('link', { name: /создать|create/i })
      .first()
      .click()
    await expect(page.getByRole('heading', { name: /создать задачу|new issue/i })).toBeVisible()
    await page.getByRole('combobox').first().selectOption('DEMO')
    await page.getByRole('textbox').nth(0).fill(issueSummary)
    await page.getByRole('textbox').nth(1).fill('Created by Playwright integration spec')
    await page.getByRole('button', { name: /создать$/i }).click()
    await expect(page).toHaveURL(/\/projects\/DEMO\/backlog/, { timeout: 10000 })
    await expect(page.getByText(issueSummary)).toBeVisible()
  })
})
