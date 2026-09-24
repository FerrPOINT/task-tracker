import { mkdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'
import { signInAt } from './qa-login'

test.skip(process.env.SDLC_LIVE_QA !== '1', 'Requires the running local SDLC fleet')
test.skip(({ browserName }) => browserName !== 'chromium', 'Single browser live smoke')
test.use({ trace: 'off' })

const account =
  process.env.SDLC_LIVE_QA === '1'
    ? (JSON.parse(
        readFileSync(
          process.env.SDLC_QA_SESSION_FILE ??
            fileURLToPath(
              new URL('../../../services-base/deploy/.local/qa-session.json', import.meta.url),
            ),
          'utf8',
        ),
      ) as { email: string; password: string })
    : { email: '', password: '' }
const screenshotDir = fileURLToPath(
  new URL('../../../.local/screenshots/fleet-pages/', import.meta.url),
)
const base = 'http://localhost:7742'
const routes = [
  'dashboard',
  'leaders',
  'executors',
  'agents',
  'sessions',
  'workflows',
  'deployments',
  'alerts',
  'logs',
  'settings',
] as const
const viewports = [
  [375, 812],
  [768, 1024],
  [1280, 800],
  [1920, 1080],
] as const

test('Fleet Control routes remain usable across themes and viewports', async ({ page }) => {
  test.setTimeout(900_000)
  mkdirSync(screenshotDir, { recursive: true })
  await signInAt(page, `${base}/`, account)
  const runtimeErrors: string[] = []
  page.on('pageerror', (error) => runtimeErrors.push(`page: ${error.message}`))
  page.on('console', (message) => {
    if (message.type() === 'error') runtimeErrors.push(`console: ${message.text()}`)
  })
  page.on('requestfailed', (request) => {
    const reason = request.failure()?.errorText ?? 'unknown'
    if (!reason.includes('ERR_ABORTED')) {
      runtimeErrors.push(
        `request: ${request.method()} ${new URL(request.url()).pathname} (${reason})`,
      )
    }
  })
  page.on('response', (response) => {
    if (response.status() >= 500) {
      runtimeErrors.push(`response: ${response.status()} ${new URL(response.url()).pathname}`)
    }
  })

  for (const theme of ['dark', 'gray', 'light'] as const) {
    for (let step = 0; step < 3; step++) {
      if ((await page.locator('html').getAttribute('data-theme')) === theme) break
      await page.getByRole('button', { name: /Тема:|Theme:|Переключить тему|Switch theme/ }).click()
    }
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
    for (const route of routes) {
      const path = route === 'dashboard' ? '/' : `/${route}`
      await page.goto(`${base}${path}`)
      await expect(page).toHaveURL(`${base}${path}`)
      await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible()
      await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible()
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
      for (const [width, height] of viewports) {
        await page.setViewportSize({ width, height })
        await expect
          .poll(
            () =>
              page.evaluate(
                () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
              ),
            { message: `Fleet ${route} ${theme} ${width}px overflow` },
          )
          .toBeLessThanOrEqual(1)
        const nestedScrollers = await page.evaluate(() =>
          [...document.querySelectorAll<HTMLElement>('*')]
            .filter((element) => {
              if (element === document.body || element === document.documentElement) return false
              if (!element.getClientRects().length) return false
              const style = getComputedStyle(element)
              return (
                (/^(auto|scroll)$/.test(style.overflowX) &&
                  element.scrollWidth > element.clientWidth + 1) ||
                (/^(auto|scroll)$/.test(style.overflowY) &&
                  element.scrollHeight > element.clientHeight + 1)
              )
            })
            .map(
              (element) => `${element.tagName.toLowerCase()}${element.id ? `#${element.id}` : ''}`,
            ),
        )
        expect(nestedScrollers, `Fleet ${route} ${theme} ${width}px nested scrollers`).toEqual([])
        await page.screenshot({
          path: `${screenshotDir}/${route}-${theme}-${width}.png`,
          fullPage: true,
          animations: 'disabled',
        })
        const audit = await new AxeBuilder({ page }).analyze()
        expect(
          audit.violations
            .filter((issue) => issue.impact === 'serious' || issue.impact === 'critical')
            .map((issue) => ({
              id: issue.id,
              count: issue.nodes.length,
              nodes: issue.nodes.slice(0, 5).map((node) => ({
                target: node.target,
                reason: node.failureSummary,
              })),
            })),
          `Fleet ${route} ${theme} ${width}px accessibility`,
        ).toEqual([])
      }
    }
  }
  expect(runtimeErrors, 'Fleet console, page, network and server errors').toEqual([])
})
