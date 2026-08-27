// Live smoke: backlog pagination pager works against the real Docker stack.
import { test, expect } from '@playwright/test'

test('backlog pager pages through the full backlog', async ({ page }) => {
  const login = await page.request.post('http://localhost:3456/api/v1/auth/login', {
    data: { email: 'demo@example.com', password: 'demo' },
  })
  const { access_token: token } = await login.json()
  await page.goto('http://localhost:19877/login')
  await page.evaluate(
    (t) => localStorage.setItem('task-tracker-auth', JSON.stringify({ state: { token: t }, version: 0 })),
    token,
  )
  await page.goto('http://localhost:19877/projects/DEMO/backlog')
  await page.waitForFunction(() => document.body.innerText.includes('Backlog'), null, { timeout: 20000 })
  await page.waitForTimeout(1500)

  const body1 = await page.evaluate(() => document.body.innerText)
  const totalMatch = body1.match(/из (\d+)/)
  expect(totalMatch, 'total counter visible').not.toBeNull()

  const next = page.locator('button', { hasText: 'Вперёд' })
  await expect(next).toBeVisible()
  await next.click()
  await page.waitForTimeout(1500)

  const body2 = await page.evaluate(() => document.body.innerText)
  const range = body2.match(/(\d+)[–-](\d+) из (\d+)/)
  expect(range, 'page-2 range visible').not.toBeNull()
  expect(Number(range?.[1] ?? 0)).toBeGreaterThan(1)
})
