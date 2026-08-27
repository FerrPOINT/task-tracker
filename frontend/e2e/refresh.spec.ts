import { test, expect } from '@playwright/test'
import { apiLogin } from './setup'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'

test('token refresh keeps session alive when access token expires (HTTP)', async ({ page }) => {
  const loginRes = (await apiLogin()) as {
    status: number
    data: { access_token: string; refresh_token: string }
  }
  const auth = loginRes.data

  // Seed a valid session, then corrupt the access token.
  await page.goto(baseURL + '/login')
  await page.evaluate(
    ([t, rt]) => {
      const raw = localStorage.getItem('task-tracker-auth')
      const parsed = raw ? JSON.parse(raw) : {}
      parsed.state = { ...parsed.state, token: t }
      localStorage.setItem('task-tracker-auth', JSON.stringify(parsed))
      if (rt) localStorage.setItem('tt-refresh-token', rt)
    },
    [auth.access_token, auth.refresh_token],
  )
  await page.evaluate(() => {
    const raw = localStorage.getItem('task-tracker-auth')
    const parsed = raw ? JSON.parse(raw) : {}
    parsed.state = { ...parsed.state, token: 'expired-token-value' }
    localStorage.setItem('task-tracker-auth', JSON.stringify(parsed))
  })

  await page.goto(baseURL + '/')
  await expect(page).not.toHaveURL(/\/login/, { timeout: 20_000 })
  await expect(page.getByRole('heading').first()).toBeVisible({ timeout: 20_000 })

  // The rotated refresh token must be persisted for the next cycle.
  const stored = await page.evaluate(() => localStorage.getItem('tt-refresh-token'))
  expect(stored).toBeTruthy()
})
