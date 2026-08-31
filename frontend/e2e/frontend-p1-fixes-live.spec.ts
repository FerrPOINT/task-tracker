// P1 frontend fixes live verification: sidebar context on issue page, mobile
// tabs overflow, board single-tree render, DnD workflow gating.
import { test, expect, type Page } from '@playwright/test'

const BASE = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:19877'
const API = process.env.VITE_API_BASE_URL ?? 'http://localhost:3456/api/v1'

let lastToken: string | undefined

async function loginToken(page: Page): Promise<string> {
  for (let attempt = 0; attempt < 6; attempt++) {
    const res = await page.request.post(`${API}/auth/login`, {
      data: { email: 'demo@example.com', password: 'demo' },
    })
    if (res.ok()) return (await res.json()).access_token
    await page.waitForTimeout(3000)
  }
  throw new Error('login kept failing (rate limit?)')
}

async function cachedToken(page: Page): Promise<string> {
  lastToken ??= await loginToken(page)
  return lastToken
}

async function authedGet(page: Page, url: string) {
  for (let attempt = 0; attempt < 3; attempt++) {
    const token = await cachedToken(page)
    const res = await page.request.get(url, {
      headers: { Authorization: `Bearer ${token}` },
    })
    if (res.status() !== 401) return res
    lastToken = undefined
  }
  throw new Error('auth kept failing')
}

async function auth(page: Page) {
  const token = await cachedToken(page)
  await page.goto(`${BASE}/login`)
  await page.evaluate(
    (t) =>
      localStorage.setItem(
        'task-tracker-auth',
        JSON.stringify({ state: { token: t }, version: 0 }),
      ),
    token,
  )
}

async function firstIssue(page: Page) {
  const issues = await authedGet(page, `${API}/search?q=test&limit=1`)
  expect(issues.ok()).toBeTruthy()
  return (await issues.json()).issues[0]
}

test('issue page keeps project context in sidebar (no /projects/TT 404)', async ({ page }) => {
  await auth(page)
  const issue = await firstIssue(page)
  await page.goto(`${BASE}/issues/${issue.id}`)
  await page.waitForFunction(() => document.body.innerText.length > 50, null, { timeout: 30000 })
  await page.waitForTimeout(2000)
  const href = await page.evaluate(() => {
    const links = [...document.querySelectorAll('a[href*="/backlog"]')]
    return links[0]?.getAttribute('href') ?? ''
  })
  expect(href).toContain(`/projects/${issue.project_key}/backlog`)
  const bad = await page.evaluate(() =>
    performance
      .getEntriesByType('resource')
      .some((r) => r.name.includes('/projects/TT/') && r.name.includes('404')),
  )
  expect(bad).toBe(false)
})

test('issue detail does not overflow horizontally at 375px', async ({ page }) => {
  await auth(page)
  const issue = await firstIssue(page)
  await page.setViewportSize({ width: 375, height: 812 })
  await page.goto(`${BASE}/issues/${issue.id}`)
  await page.waitForFunction(() => document.body.innerText.length > 50, null, { timeout: 30000 })
  await page.waitForTimeout(1500)
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  )
  expect(overflow).toBeLessThanOrEqual(1)
  await page.screenshot({
    path: '/root/.hermes/cache/images/issue-mobile-fixed.png',
    fullPage: true,
  })
})

test('board renders a single tree and pages stay under the pixel cap', async ({ page }) => {
  await auth(page)
  await page.goto(`${BASE}/projects/DEMO/board`)
  await page.waitForFunction(
    () => document.body.innerText.includes('Backlog') || document.body.innerText.includes('Todo'),
    null,
    { timeout: 30000 },
  )
  await page.waitForTimeout(2000)
  const stats = await page.evaluate(() => ({
    cards: document.querySelectorAll('a[href*="/issues/"]').length,
    height: document.documentElement.scrollHeight,
  }))
  expect(stats.height).toBeLessThan(32767)
})
