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
          process.env.SDLC_QA_SESSION_FILE ??
            fileURLToPath(
              new URL('../../../services-base/deploy/.local/qa-session.json', import.meta.url),
            ),
          'utf8',
        ),
      ) as { email: string; password: string })
    : { email: '', password: '' }

test('dashboard uses Russian labels on live project counters', async ({ page, request }) => {
  test.setTimeout(120_000)
  const login = await request.post('http://localhost:7701/auth/login', {
    data: { email: account.email, password: account.password },
  })
  expect(login.ok(), await login.text()).toBeTruthy()
  const { access_token: token } = (await login.json()) as { access_token: string }
  const headers = { Authorization: `Bearer ${token}` }
  const key = `QD${Date.now().toString(36).slice(-7).toUpperCase()}`
  const created = await request.post('http://localhost:7721/api/v1/projects', {
    headers,
    data: { key, name: `QA dashboard ${key}`, description: 'QA localization smoke' },
  })
  expect(created.ok(), await created.text()).toBeTruthy()

  try {
    await page.setViewportSize({ width: 375, height: 812 })
    await signInAt(page, 'http://localhost:7722/', account)
    await expect(page.getByRole('heading', { name: 'Командный дашборд' })).toBeVisible()
    await expect(page.getByText('К выполнению: 0').first()).toBeVisible()
    await expect(page.getByText('В работе: 0').first()).toBeVisible()
    await expect(page.getByText('Готово: 0').first()).toBeVisible()
    await expect(page.getByText(/To Do:|In Progress:|Done:/)).toHaveCount(0)
    await page.screenshot({
      path: fileURLToPath(
        new URL('../../../.local/screenshots/task-dashboard-localized.png', import.meta.url),
      ),
      fullPage: true,
    })
  } finally {
    const removed = await request.delete(`http://localhost:7721/api/v1/projects/${key}`, {
      headers,
    })
    expect(removed.ok(), await removed.text()).toBeTruthy()
  }
})
