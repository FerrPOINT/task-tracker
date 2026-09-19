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
  ['admin', 'http://localhost:7772/'],
  ['cicd', 'http://localhost:7712/'],
  ['task', 'http://localhost:7722/'],
  ['wiki', 'http://localhost:7732/'],
  ['fleet', 'http://localhost:7742/'],
] as const

test('shared controls stay touch-sized without mobile overflow', async ({ page }) => {
  test.setTimeout(180_000)
  mkdirSync(screenshotDir, { recursive: true })
  await page.setViewportSize({ width: 375, height: 812 })
  await signInAt(page, apps[0][1], account)

  for (const [key, url] of apps) {
    await page.goto(url)
    const switcher = page.getByRole('button', { name: 'Открыть список сервисов' })
    await expect(switcher).toBeVisible({ timeout: 30_000 })
    await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible()
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

test('Workflow mobile shell keeps touch targets and keyboard focus inside its drawer', async ({
  page,
}) => {
  test.setTimeout(90_000)
  mkdirSync(screenshotDir, { recursive: true })
  await page.setViewportSize({ width: 375, height: 812 })
  const base = 'http://localhost:8812/'
  const serviceMenu = page.locator('summary[aria-label="Открыть список сервисов платформы"]')
  await signInAt(page, base, account, serviceMenu)
  await page.waitForLoadState('load')

  const smallControls = await page
    .locator('.header button, .header select, .header summary, .header a')
    .evaluateAll((elements) =>
      elements
        .map((element) => {
          const box = element.getBoundingClientRect()
          return {
            name: element.getAttribute('aria-label') ?? element.textContent?.trim(),
            width: box.width,
            height: box.height,
          }
        })
        .filter(
          (control) =>
            control.width > 0 && control.height > 0 && (control.width < 40 || control.height < 40),
        ),
    )
  expect(smallControls, 'Workflow visible header controls').toEqual([])
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    ),
  ).toBeLessThanOrEqual(1)

  const sidebar = page.locator('#sidebar')
  const burger = page.getByRole('button', { name: 'Открыть навигацию' })
  await expect(sidebar).toHaveAttribute('inert')
  await burger.click()
  await expect(burger).toHaveAttribute('aria-expanded', 'true')
  await expect(sidebar).not.toHaveAttribute('inert')
  await expect(page.getByRole('button', { name: 'Закрыть навигацию' })).toBeFocused()
  await page.keyboard.press('Shift+Tab')
  await page.keyboard.press('Shift+Tab')
  expect(
    await page.evaluate(() => document.getElementById('sidebar')?.contains(document.activeElement)),
  ).toBe(true)
  await page.keyboard.press('Escape')
  await expect(sidebar).toHaveAttribute('inert')
  await expect(burger).toHaveAttribute('aria-expanded', 'false')
  await expect(burger).toBeFocused()

  await serviceMenu.click()
  await expect(page.getByRole('menu').getByRole('menuitem')).toHaveCount(6)
  await page.keyboard.press('Escape')
  await expect(serviceMenu).toBeFocused()
  await page.screenshot({ path: `${screenshotDir}/workflow-375.png`, fullPage: true })
})
