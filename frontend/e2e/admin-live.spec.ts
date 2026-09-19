import { mkdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'
import { signInAt } from './qa-login'

test.skip(process.env.SDLC_LIVE_QA !== '1', 'Requires the running local SDLC fleet')
test.skip(({ browserName }) => browserName !== 'chromium', 'Single browser live smoke')
test.use({ trace: 'off' })

const screenshotDir = fileURLToPath(new URL('../../../.local/screenshots/', import.meta.url))
const qaAccount =
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
const paths = [
  '/',
  '/services',
  '/services/admin-panel',
  '/revisions',
  '/branding',
  '/users',
  '/tokens',
  '/audit',
  '/runtime',
  '/settings',
]
const viewports = [
  [375, 812],
  [768, 1024],
  [1280, 800],
  [1920, 1080],
] as const

test('Admin pages stay usable across themes and viewports', async ({ page, request }) => {
  test.setTimeout(240_000)
  mkdirSync(screenshotDir, { recursive: true })
  await signInAt(page, 'http://localhost:7772/', qaAccount)

  for (const path of paths) {
    await page.goto(`http://localhost:7772${path}`)
    await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible({ timeout: 30_000 })
    for (const theme of ['dark', 'gray', 'light'] as const) {
      for (let attempt = 0; attempt < 3; attempt++) {
        if ((await page.locator('html').getAttribute('data-theme')) === theme) break
        await page
          .getByRole('button', { name: /Тема:|Theme:|Переключить тему|Switch theme/ })
          .click()
      }
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
      for (const [width, height] of viewports) {
        await page.setViewportSize({ width, height })
        await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible()
        await expect
          .poll(
            () =>
              page.evaluate(
                () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
              ),
            { message: `Admin ${path} ${theme} ${width}px overflow` },
          )
          .toBeLessThanOrEqual(1)
        await page.screenshot({
          path: `${screenshotDir}/admin-${path.replaceAll('/', '-') || 'home'}-${theme}-${width}.png`,
          fullPage: true,
          animations: 'disabled',
        })
        if ((theme === 'dark' && width === 375) || (theme === 'light' && width === 1280)) {
          const audit = await new AxeBuilder({ page }).analyze()
          expect(
            audit.violations.filter(
              (issue) => issue.impact === 'serious' || issue.impact === 'critical',
            ),
            `Admin ${path} ${theme} ${width}px`,
          ).toEqual([])
        }
      }
    }
  }

  const response = await request.get('http://localhost:7771/api/v1/runtime/services')
  expect(response.ok()).toBeTruthy()
  const { services } = (await response.json()) as {
    services: { key: string; ui_url: string | null; health: string }[]
  }
  expect(services).toHaveLength(8)
  expect(services.filter((service) => service.ui_url)).toHaveLength(6)
  expect(services.every((service) => service.health === 'healthy')).toBeTruthy()
})
