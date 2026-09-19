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
          fileURLToPath(new URL('../../../.local/qa-session.json', import.meta.url)),
          'utf8',
        ),
      ) as {
        email: string
        password: string
      })
    : { email: '', password: '' }
const screenshotDir = fileURLToPath(new URL('../../../.local/screenshots/', import.meta.url))
const apps = [
  { key: 'admin', url: 'http://localhost:7772/users' },
  { key: 'ci', url: 'http://localhost:7712/projects' },
  { key: 'task-board', url: 'http://localhost:7722/projects/POLKA/board' },
  { key: 'wiki', url: 'http://localhost:7732/spaces' },
  { key: 'fleet', url: 'http://localhost:7742/agents' },
  { key: 'workflow', url: 'http://localhost:8812/workflows' },
] as const

test('real pages fit four viewports in three themes without serious accessibility errors', async ({
  page,
}) => {
  test.setTimeout(360_000)
  mkdirSync(screenshotDir, { recursive: true })
  await signInAt(page, 'http://localhost:7772/users', account)
  const runtimeErrors: string[] = []
  page.on('pageerror', (error) => runtimeErrors.push(`page: ${error.message}`))
  page.on('console', (message) => {
    if (message.type() === 'error') runtimeErrors.push(`console: ${message.text()}`)
  })
  page.on('requestfailed', (request) => {
    const reason = request.failure()?.errorText ?? 'unknown'
    if (!reason.includes('ERR_ABORTED')) {
      runtimeErrors.push(`request: ${request.method()} ${request.url()} (${reason})`)
    }
  })

  for (const app of apps) {
    await page.goto(app.url)
    if (app.key === 'workflow') {
      await expect(page.locator('details.service-menu summary')).toBeVisible({ timeout: 30_000 })
    } else {
      await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible({
        timeout: 30_000,
      })
    }
    const pageContent =
      app.key === 'workflow'
        ? page.locator('#workflowForm')
        : page.getByRole('heading', { level: 1 }).first()
    await expect(pageContent).toBeVisible({ timeout: 30_000 })
    for (const theme of ['dark', 'gray', 'light'] as const) {
      if (app.key === 'workflow') {
        await page.getByRole('combobox', { name: 'Тема' }).selectOption(theme)
      } else {
        for (let step = 0; step < 3; step++) {
          if ((await page.locator('html').getAttribute('data-theme')) === theme) break
          await page
            .getByRole('button', { name: /Тема:|Theme:|Переключить тему|Switch theme/ })
            .click({ timeout: 10_000 })
        }
      }
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
      for (const [width, height] of [
        [375, 812],
        [768, 1024],
        [1280, 800],
        [1920, 1080],
      ]) {
        await page.setViewportSize({ width, height })
        await expect(pageContent).toBeVisible({ timeout: 10_000 })
        await expect
          .poll(
            () =>
              page.evaluate(
                () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
              ),
            { message: `${app.key} ${theme} ${width}px overflow` },
          )
          .toBeLessThanOrEqual(1)
        const nestedScrollers = await page.evaluate(() =>
          [...document.querySelectorAll<HTMLElement>('*')]
            .filter((element) => {
              if (element === document.body || element === document.documentElement) return false
              if (!element.getClientRects().length) return false
              const style = getComputedStyle(element)
              const scrollsX =
                /^(auto|scroll)$/.test(style.overflowX) &&
                element.scrollWidth > element.clientWidth + 1
              const scrollsY =
                /^(auto|scroll)$/.test(style.overflowY) &&
                element.scrollHeight > element.clientHeight + 1
              return scrollsX || scrollsY
            })
            .map((element) =>
              `${element.tagName.toLowerCase()}${element.id ? `#${element.id}` : ''}.${
                [...element.classList].slice(0, 3).join('.')
              }`,
            ),
        )
        expect(nestedScrollers, `${app.key} ${theme} ${width}px nested scrollers`).toEqual([])
        if (app.key === 'fleet' && theme === 'dark' && width === 375) {
          const section = page.getByRole('combobox', { name: 'Fleet section' })
          await section.selectOption('/settings')
          await expect(page).toHaveURL('http://localhost:7742/settings')
          await section.selectOption('/agents')
          await expect(page).toHaveURL(app.url)
        }
        await page.screenshot({
          path: `${screenshotDir}/sso-${app.key}-${theme}-${width}.png`,
          fullPage: true,
          animations: 'disabled',
        })
        const result = await new AxeBuilder({ page }).analyze()
        expect(
          result.violations
            .filter((issue) => issue.impact === 'serious' || issue.impact === 'critical')
            .map((issue) => ({
              id: issue.id,
              impact: issue.impact,
              count: issue.nodes.length,
              targets: issue.nodes.slice(0, 5).map((node) => node.target),
            })),
          `${app.key} ${theme} ${width}px`,
        ).toEqual([])
      }
    }
  }
  expect(runtimeErrors, 'Console, page and network errors').toEqual([])
})
