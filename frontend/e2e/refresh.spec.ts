import { test, expect } from '@playwright/test'
import { API_BASE_URL, seedIntegrationData } from './setup'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'

test.setTimeout(120_000)

test('token refresh keeps session alive through the httpOnly cookie', async ({ page }) => {
  await seedIntegrationData()
  const loginRes = await page.request.post(`${API_BASE_URL}/auth/login`, {
    data: { email: 'demo@example.com', password: 'demo' },
  })
  expect(loginRes.status()).toBe(200)
  const auth = (await loginRes.json()) as { access_token: string; refresh_token?: string }
  expect(auth.refresh_token).toBeUndefined()

  // Refresh state must live in the browser cookie jar, not in localStorage.
  await page.goto(baseURL + '/login')
  await page.evaluate(() => {
    const raw = localStorage.getItem('task-tracker-auth')
    const parsed = raw ? JSON.parse(raw) : {}
    parsed.state = { ...parsed.state, token: null }
    localStorage.setItem('task-tracker-auth', JSON.stringify(parsed))
  })

  await page.goto(baseURL + '/')
  await expect(page).not.toHaveURL(/\/login/, { timeout: 20_000 })
  await expect(page.getByRole('heading').first()).toBeVisible({ timeout: 20_000 })

  const stored = await page.evaluate(() => localStorage.getItem('tt-refresh-token'))
  expect(stored).toBeNull()
})
