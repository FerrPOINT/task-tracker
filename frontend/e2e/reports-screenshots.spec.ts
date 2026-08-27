import { test, expect, Page } from '@playwright/test'
import { seedIntegrationData } from './setup'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'

let cachedAuth: { token: string; userId: string; email: string } | null = null

async function authenticate(p: Page) {
  // One login per worker: reusing the seeded context avoids tripping the
  // auth rate limiter (5 req / 15 s) with a login per screenshot.
  if (!cachedAuth) {
    const ctx = await seedIntegrationData()
    cachedAuth = { token: ctx.token, userId: ctx.userId, email: 'demo@example.com' }
  }
  const { token: access_token, userId: user_id, email } = cachedAuth
  await p.goto(`${baseURL}/login`)
  await p.evaluate(
    (payload: { token: string; userId: string; email: string }) => {
      // zustand persist stores {state:{...},version:0}; the app rehydrates from .state.
      window.localStorage.setItem(
        'task-tracker-auth',
        JSON.stringify({ state: payload, version: 0 }),
      )
    },
    { token: access_token, userId: user_id, email },
  )
}

async function setThemeAndGoto(p: Page, theme: 'light' | 'dark', path: string, marker: string) {
  await p.addInitScript((t) => {
    document.documentElement.classList.toggle('dark', t === 'dark')
    try {
      localStorage.setItem('vite-ui-prefers-dark', JSON.stringify(t === 'dark'))
    } catch {
      // storage unavailable
    }
  }, theme)
  await p.goto(`${baseURL}${path}`)
  // SSE connection stays open forever, so networkidle never fires on authed pages.
  await p.waitForFunction((text: string) => document.body.innerText.includes(text), marker, {
    timeout: 10_000,
  })
  await p.waitForTimeout(300)
}

test.describe('reports screenshots', () => {
  test.setTimeout(120_000)

  test.beforeAll(async () => {
    await seedIntegrationData()
  })

  const tabs: Array<{ tab: string; trigger: RegExp; name: string; waitFor: string | RegExp }> = [
    {
      tab: 'velocity',
      trigger: /скорость|velocity/i,
      name: 'velocity',
      waitFor: 'Sprint 10',
    },
    {
      tab: 'burndown',
      trigger: /burndown/i,
      name: 'burndown',
      waitFor: /оставшиеся|remaining/i,
    },
    {
      tab: 'cumulative-flow',
      trigger: /кумулятивн|cumulative/i,
      name: 'cumulative-flow',
      waitFor: /todo|к выполнению/i,
    },
    {
      tab: 'control-chart',
      trigger: /контрольн|control chart/i,
      name: 'control-chart',
      waitFor: /cycle|цикл/i,
    },
  ]

  for (const viewport of [
    { name: 'fullhd', width: 1920, height: 1080 },
  ]) {
    for (const t of tabs) {
      test(`${t.name} ${viewport.name} screenshot`, async ({ page }) => {
        await authenticate(page)
        await page.setViewportSize({ width: viewport.width, height: viewport.height })
        await setThemeAndGoto(page, 'dark', '/reports', 'Отчёты')
        // select DEMO project in the picker
        await page.getByLabel(/проект/i).selectOption({ label: 'Demo Project' })
        if (t.tab === 'burndown') {
          // pick the active sprint (burndown needs a sprint id)
          const res = await page.request.get(
            `${baseURL.replace(':4173', ':3456')}/api/v1/projects/DEMO/sprints`,
            { headers: { Authorization: `Bearer ${cachedAuth!.token}` } },
          )
          const data = await res.json()
          const active =
            data.sprints?.find((sp: { state: string }) => sp.state === 'active') ??
            data.sprints?.[0]
          await page.getByLabel(/спринт/i, { exact: false }).fill(active?.id ?? '')
        }
        await page.getByRole('tab', { name: t.trigger }).click()
        // wait for chart content to render
        await expect(page.locator('.recharts-wrapper').first()).toBeVisible({ timeout: 10_000 })
        await page.waitForTimeout(600)
        await page.screenshot({
          path: `/root/.hermes/cache/images/react-reports-${t.name}-${viewport.name}.png`,
          fullPage: true,
          scale: 'css',
        })
      })
    }
  }
})
