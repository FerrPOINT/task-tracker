import { mkdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { expect, test } from '@playwright/test'
import { signInAt } from './qa-login'

test.skip(process.env.SDLC_LIVE_QA !== '1', 'Requires the running local SDLC fleet')
test.skip(({ browserName }) => browserName !== 'chromium', 'Single browser live smoke')

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
  new URL('../../../.local/screenshots/shared-touch/', import.meta.url),
)
const apps = [
  ['admin', 'http://localhost:7772/', 'Обзор платформы'],
  ['cicd', 'http://localhost:7712/', 'Дашборд'],
  ['task', 'http://localhost:7722/', 'Командный дашборд'],
  ['wiki', 'http://localhost:7732/', 'Wiki'],
  ['fleet', 'http://localhost:7742/', 'Fleet dashboard'],
] as const

test('shared controls stay touch-sized without mobile overflow', async ({ page }) => {
  test.setTimeout(180_000)
  mkdirSync(screenshotDir, { recursive: true })
  await page.setViewportSize({ width: 375, height: 812 })
  await signInAt(page, apps[0][1], account)

  for (const [key, url, heading] of apps) {
    await page.goto(url)
    await expect(page.getByRole('heading', { name: heading, exact: true })).toBeVisible()
    const switcher = page.getByRole('button', { name: 'Открыть список сервисов' })
    await expect(switcher).toBeVisible()
    const header = await page.evaluate(() => ({
      overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
      smallButtons: [...document.querySelectorAll('header button')]
        .filter((button) => {
          const box = button.getBoundingClientRect()
          return box.width > 0 && box.height > 0
        })
        .map((button) => {
          const box = button.getBoundingClientRect()
          return {
            name: button.getAttribute('aria-label') ?? button.textContent?.trim(),
            width: Math.round(box.width),
            height: Math.round(box.height),
          }
        })
        .filter((button) => button.width < 40 || button.height < 40),
    }))
    expect(header.overflow, `${key} document overflow`).toBe(false)
    expect(header.smallButtons, `${key} header buttons`).toEqual([])

    await switcher.click()
    const menuItems = page.getByRole('menuitem')
    await expect(menuItems).toHaveCount(6)
    const smallItems = await menuItems.evaluateAll((items) =>
      items
        .map((item) => {
          const box = item.getBoundingClientRect()
          return { label: item.textContent?.trim(), height: Math.round(box.height) }
        })
        .filter((item) => item.height < 40),
    )
    expect(smallItems, `${key} service menu items`).toEqual([])
    await page.keyboard.press('Escape')
    await page.screenshot({ path: `${screenshotDir}/${key}-375.png`, fullPage: true })
  }
})
