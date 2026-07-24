import { test, expect } from '@playwright/test'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'

test('end-to-end smoke: login, browse project, create issue', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 })
  await page.goto(`${baseURL}/login`)

  await expect(page.getByText('TaskTracker')).toBeVisible()
  await page.locator('input[type="email"]').fill('demo@example.com')
  await page.locator('input[type="password"]').fill('demo')
  await page.getByRole('button', { name: 'Войти' }).click()

  await page.waitForURL(`${baseURL}/`)
  await expect(page.getByText('Team Dashboard')).toBeVisible()

  await page.getByText('Проекты').first().click()
  await page.waitForURL(`${baseURL}/projects`)
  await expect(page.getByText('Task Tracker')).toBeVisible()

  await page.getByText('Task Tracker').first().click()
  await page.getByText('Доска').click()
  await page.waitForURL(`${baseURL}/projects/TT/board`)
  await expect(page.getByText('TT Kanban')).toBeVisible()

  await page.getByText('Создать').first().click()
  await page.waitForURL(`${baseURL}/issues/create`)
  await page.getByPlaceholder('Краткое описание задачи').fill('E2E smoke 1784867024981')
  await page.getByRole('button', { name: 'Создать', exact: true }).click()

  await page.waitForURL(`${baseURL}/projects/TT/backlog`)
  await expect(page.getByText('E2E smoke 1784867024981', { exact: true })).toBeVisible()
})
