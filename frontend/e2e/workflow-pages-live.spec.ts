import { mkdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'
import { signInAt } from './qa-login'

test.skip(process.env.SDLC_LIVE_QA !== '1', 'Requires the running local SDLC stack')
test.skip(({ browserName }) => browserName !== 'chromium', 'Single browser live smoke')
test.use({ trace: 'off' })

const account =
  process.env.SDLC_LIVE_QA === '1'
    ? (JSON.parse(
        readFileSync(
          fileURLToPath(new URL('../../../.local/qa-session.json', import.meta.url)),
          'utf8',
        ),
      ) as { email: string; password: string })
    : { email: '', password: '' }
const screenshotDir = fileURLToPath(
  new URL('../../../.local/screenshots/workflow-pages/', import.meta.url),
)
const base = 'http://localhost:8812'
const routes = [
  'dashboard',
  'phases',
  'instructions',
  'tasks',
  'namespaces',
  'workflows',
  'settings',
  'agents',
] as const
const viewports = [
  [375, 812],
  [768, 1024],
  [1280, 800],
  [1920, 1080],
] as const

test('Project Workflow routes remain usable across themes and viewports', async ({ page }) => {
  test.setTimeout(900_000)
  mkdirSync(screenshotDir, { recursive: true })
  const services = page.locator('summary[aria-label="Открыть список сервисов платформы"]')
  await signInAt(page, `${base}/`, account, services)
  await page.waitForLoadState('load')
  await expect
    .poll(() =>
      page.evaluate(
        () => typeof (window as unknown as { setPlatformTheme?: unknown }).setPlatformTheme,
      ),
    )
    .toBe('function')
  const runtimeErrors: string[] = []
  const layoutIssues: string[] = []
  const accessibilityIssues: string[] = []
  page.on('pageerror', (error) => runtimeErrors.push(`page: ${error.message} at ${page.url()}`))
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
    await page.locator('#themeSelector').selectOption(theme)
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
    for (const route of routes) {
      const path = route === 'dashboard' ? '/' : `/${route}`
      await page.goto(`${base}${path}`)
      await expect(page).toHaveURL(`${base}${path}`)
      await expect(services).toBeVisible()
      await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible()
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
      await page.waitForLoadState('networkidle')
      await expect
        .poll(() =>
          page.evaluate(
            () => typeof (window as unknown as { setPlatformTheme?: unknown }).setPlatformTheme,
          ),
        )
        .toBe('function')
      for (const [width, height] of viewports) {
        await page.setViewportSize({ width, height })
        const overflow = await page.evaluate(
          () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
        )
        if (overflow > 1) layoutIssues.push(`${route} ${theme} ${width}px: ${overflow}px overflow`)
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
        if (nestedScrollers.length) {
          layoutIssues.push(
            `${route} ${theme} ${width}px: nested scrollers ${nestedScrollers.join(', ')}`,
          )
        }
        await page.screenshot({
          path: `${screenshotDir}/${route}-${theme}-${width}.png`,
          fullPage: true,
          animations: 'disabled',
        })
        const audit = await new AxeBuilder({ page }).analyze()
        for (const issue of audit.violations) {
          if (issue.impact !== 'serious' && issue.impact !== 'critical') continue
          accessibilityIssues.push(
            `${route} ${theme} ${width}px: ${issue.id} (${issue.nodes.length}) ${issue.nodes
              .slice(0, 3)
              .map((node) => node.target.join(' '))
              .join(', ')}`,
          )
        }
      }
    }
  }
  expect.soft(layoutIssues, 'Workflow document overflow and nested scrollers').toEqual([])
  expect.soft(accessibilityIssues, 'Workflow serious/critical accessibility violations').toEqual([])
  expect.soft(runtimeErrors, 'Workflow console, page, network and server errors').toEqual([])
})
