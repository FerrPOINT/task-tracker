import { test, expect } from '@playwright/test'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'

test.setTimeout(120_000)
test('token refresh keeps session alive when access token expires (HTTP)', async ({ page }) => {
  // Login over the same origin the app uses (vite preview proxy), so the
  // HttpOnly refresh cookie lands in the browser jar.
  const loginApi = await page.request.post(`${baseURL}/api/v1/auth/login`, {
    data: { email: 'demo@example.com', password: 'demo' },
  })
  expect(loginApi.ok()).toBeTruthy()
  const data = (await loginApi.json()) as { access_token: string; refresh_token?: string }
  expect(data.refresh_token).toBeUndefined()
  const access_token = data.access_token

  // Seed the auth store with the access token, then corrupt it: the app
  // must recover via the HttpOnly refresh cookie (no localStorage copy).
  await page.goto(baseURL + '/login')
  await page.evaluate((t) => {
    localStorage.setItem(
      'task-tracker-auth',
      JSON.stringify({ state: { token: t, userId: '', email: 'demo@example.com' }, version: 0 }),
    )
  }, access_token)
  await page.evaluate(() => {
    const raw = localStorage.getItem('task-tracker-auth')
    const parsed = raw ? JSON.parse(raw) : {}
    parsed.state = { ...parsed.state, token: null }
    localStorage.setItem('task-tracker-auth', JSON.stringify(parsed))
  })

  await page.goto(baseURL + '/')
  await expect(page).not.toHaveURL(/\/login/, { timeout: 20_000 })
  await expect(page.getByRole('heading').first()).toBeVisible({ timeout: 20_000 })

  // The refresh credential must never be exposed to JS storage.
  const leaked = await page.evaluate(() => localStorage.getItem('tt-refresh-token'))
  expect(leaked).toBeNull()
})
