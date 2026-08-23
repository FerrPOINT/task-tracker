import { test, expect } from '@playwright/test'
import { seedIntegrationData } from './setup'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'

let seededIssueId = ''

test.describe.configure({ mode: 'serial' })

test.describe('time tracking against live backend', () => {
  test.beforeAll(async () => {
    const ctx = await seedIntegrationData()
    seededIssueId = ctx.issueId
  })

  test('time tracking panel and worklog flow', async ({ page }) => {
    await page.goto(`${baseURL}/login`)
    await page.getByRole('textbox').nth(0).fill('demo@example.com')
    await page.getByRole('textbox').nth(1).fill('demo')
    await page.getByRole('button', { name: /войти|login/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 10000 })

    await page.goto(`${baseURL}/issues/${seededIssueId}`)
    await expect(page.getByText(/учёт времени|time tracking/i)).toBeVisible()
    await expect(page.getByTestId('time-tracking-summary')).toHaveText(/3m/)

    await page.getByRole('button', { name: /записать время|log work/i }).click()
    await page.getByLabel(/комментарий|comment/i).fill('E2E logged work')
    await page.getByLabel(/затрачено|time spent/i).fill('1h 30m')
    await page.getByRole('button', { name: /сохранить|save/i }).click()
    // Worklog rows live on the «Журнал работ» tab, not the default «Активность» tab
    await page.getByRole('tab', { name: /журнал работ|worklog/i }).click()
    await expect(page.getByRole('cell', { name: 'E2E logged work' })).toBeVisible()
  })

  test('timer adds time to input', async ({ page }) => {
    await page.goto(`${baseURL}/login`)
    await page.getByRole('textbox').nth(0).fill('demo@example.com')
    await page.getByRole('textbox').nth(1).fill('demo')
    await page.getByRole('button', { name: /войти|login/i }).click()
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 10000 })

    await page.goto(`${baseURL}/issues/${seededIssueId}`)
    await page.getByRole('button', { name: /записать время|log work/i }).click()
    await page.getByLabel(/запустить таймер|start timer/i).click()
    await page.waitForTimeout(1100)
    await page.getByLabel(/остановить таймер|stop timer/i).click()
    const input = page.getByLabel(/затрачено|time spent/i)
    const value = await input.inputValue()
    // Value is a duration string like "1s" or "1m 5s"
    const seconds = (value.match(/(\d+)s/) || [0, 0]).slice(1).map(Number).reduce((a, x) => a + x, 0)
    const minutes = (value.match(/(\d+)m/) || [0, 0]).slice(1).map(Number).reduce((a, x) => a + x, 0)
    expect(seconds + minutes * 60).toBeGreaterThanOrEqual(1)
  })
})
