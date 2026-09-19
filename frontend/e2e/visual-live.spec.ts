import { mkdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'
import { signInAt } from './qa-login'

test.skip(process.env.SDLC_LIVE_QA !== '1', 'Requires the running local SDLC fleet')
test.skip(({ browserName }) => browserName !== 'chromium', 'Single browser live smoke')
test.use({ trace: 'off' })

const account = process.env.SDLC_LIVE_QA === '1'
  ? JSON.parse(readFileSync(fileURLToPath(new URL('../../../.local/qa-session.json', import.meta.url)), 'utf8')) as {
      email: string; password: string
    }
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

test('real pages fit four viewports in three themes without serious accessibility errors', async ({ page }) => {
  test.setTimeout(240_000)
  mkdirSync(screenshotDir, { recursive: true })
  await signInAt(page, 'http://localhost:7772/users', account)

  for (const app of apps) {
    await page.goto(app.url)
    if (app.key === 'workflow') {
      await expect(page.locator('details.service-menu summary')).toBeVisible({ timeout: 30_000 })
    } else {
      await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible({ timeout: 30_000 })
    }
    const pageContent = app.key === 'workflow'
      ? page.locator('#workflowForm')
      : page.getByRole('heading', { level: 1 }).first()
    await expect(pageContent).toBeVisible({ timeout: 30_000 })
    for (const theme of ['dark', 'gray', 'light'] as const) {
      if (app.key === 'workflow') {
        await page.getByRole('combobox', { name: 'Тема' }).selectOption(theme)
      } else {
        for (let step = 0; step < 3; step++) {
          if (await page.locator('html').getAttribute('data-theme') === theme) break
          await page.getByRole('button', { name: /Тема:|Theme:|Переключить тему|Switch theme/ }).click({ timeout: 10_000 })
        }
      }
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
      for (const [width, height] of [[375, 812], [768, 1024], [1280, 800], [1920, 1080]]) {
        await page.setViewportSize({ width, height })
        await expect(pageContent).toBeVisible({ timeout: 10_000 })
        await expect.poll(() => page.evaluate(() =>
          document.documentElement.scrollWidth - document.documentElement.clientWidth,
        ), { message: `${app.key} ${theme} ${width}px overflow` }).toBeLessThanOrEqual(1)
        await page.screenshot({
          path: `${screenshotDir}/sso-${app.key}-${theme}-${width}.png`,
          fullPage: true,
          animations: 'disabled',
        })
        if ((theme === 'dark' && width === 375) || (theme === 'light' && width === 1280)) {
          const result = await new AxeBuilder({ page }).analyze()
          expect(result.violations.filter((issue) =>
            issue.impact === 'serious' || issue.impact === 'critical',
          ), `${app.key} ${theme} ${width}px`).toEqual([])
        }
      }
    }
  }
})
