import { test, expect } from '@playwright/test'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'

test.describe('smoke', () => {
  test('login then navigate through dashboard, projects, board and create issue', async ({ page }) => {
    await page.goto(`${baseURL}/login`)
    await page.getByRole('textbox').first().fill('demo@example.com')
    await page.getByRole('textbox').nth(1).fill('demo')
    await page.getByRole('button', { name: /войти/i }).click()
    await page.waitForURL(`${baseURL}/`)
    await expect(page.getByText('Team Dashboard')).toBeVisible()

    await page.goto(`${baseURL}/projects`)
    await expect(page.getByText('Проекты')).toBeVisible()

    await page.goto(`${baseURL}/projects/TT/board`)
    await expect(page.getByText(/TT Kanban/i)).toBeVisible()

    await page.goto(`${baseURL}/issues/create`)
    await expect(page.getByText('Создать задачу')).toBeVisible()
  })
})
