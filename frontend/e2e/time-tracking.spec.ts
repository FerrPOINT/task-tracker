import { test, expect } from '@playwright/test'
import { seedIntegrationData } from './setup'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'

test.describe('time tracking against live backend', () => {
  test.beforeAll(async () => {
    await seedIntegrationData()
  })

  test('time tracking panel and worklog flow', async ({ page }) => {
    await page.goto(`${baseURL}/login`)
    await page.getByRole('textbox').nth(0).fill('demo@example.com')
    await page.getByRole('textbox').nth(1).fill('demo')
    await page.getByRole('button', { name: /войти|login/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 10000 })

    await page.goto(`${baseURL}/issues/DEMO-1`)
    await expect(page.getByText('Учёт времени')).toBeVisible()
    await expect(page.getByTestId('time-tracking-summary')).toHaveText(
      /3h потрачено \/ 8h оценка \/ 4h осталось|3h spent \/ 8h estimate \/ 4h remaining/,
    )

    await page.getByRole('button', { name: /залогировать|log time/i }).click()
    await page.getByLabel(/описание|description/i).fill('E2E logged work')
    await page.getByLabel(/минуты|minutes/i).fill('60')
    await page.getByRole('button', { name: /сохранить|save/i }).click()
    await expect(page.getByText('E2E logged work')).toBeVisible()
  })

  test('timer adds time to input', async ({ page }) => {
    await page.goto(`${baseURL}/login`)
    await page.getByRole('textbox').nth(0).fill('demo@example.com')
    await page.getByRole('textbox').nth(1).fill('demo')
    await page.getByRole('button', { name: /войти|login/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 10000 })

    await page.goto(`${baseURL}/issues/DEMO-1`)
    await page.getByRole('button', { name: /залогировать|log time/i }).click()
    await page.getByLabel(/запустить таймер|start timer/i).click()
    await page.waitForTimeout(1100)
    await page.getByLabel(/остановить таймер|stop timer/i).click()
    const input = page.getByLabel(/минуты|minutes/i)
    const value = await input.inputValue()
    expect(Number(value)).toBeGreaterThanOrEqual(1)
  })
})
