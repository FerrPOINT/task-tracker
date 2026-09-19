import { readFileSync } from 'node:fs'
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

test('dashboard uses Russian labels on live project counters', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await signInAt(page, 'http://localhost:7722/', account)
  await expect(page.getByRole('heading', { name: 'Командный дашборд' })).toBeVisible()
  await expect(page.getByText(/К выполнению: \d+/).first()).toBeVisible()
  await expect(page.getByText(/В работе: \d+/).first()).toBeVisible()
  await expect(page.getByText(/Готово: \d+/).first()).toBeVisible()
  await expect(page.getByText(/To Do:|In Progress:|Done:/)).toHaveCount(0)
  await page.screenshot({
    path: fileURLToPath(
      new URL('../../../.local/screenshots/task-dashboard-localized.png', import.meta.url),
    ),
    fullPage: true,
  })
})
