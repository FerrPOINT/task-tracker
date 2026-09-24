import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
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
      ) as {
        email: string
        password: string
        runId: string
      })
    : { email: '', password: '', runId: '' }

test('issue context and board remain usable on mobile and desktop', async ({
  page,
  request,
}, testInfo) => {
  test.setTimeout(180_000)
  const login = await request.post('http://localhost:7701/auth/login', {
    data: { email: account.email, password: account.password },
  })
  expect(login.ok(), await login.text()).toBeTruthy()
  const { access_token } = (await login.json()) as { access_token: string }
  const headers = { Authorization: `Bearer ${access_token}` }
  const api = 'http://localhost:7721/api/v1'
  const key = `Q${Date.now().toString(36).slice(-7).toUpperCase()}`
  const name = `QA ${account.runId} issue layout`
  let created = false

  try {
    const project = await request.post(`${api}/projects`, {
      headers,
      data: { key, name, description: 'QA mobile layout' },
    })
    expect(project.ok(), await project.text()).toBeTruthy()
    created = true
    const issue = await request.post(`${api}/issues`, {
      headers,
      data: {
        project_key: key,
        issue_type: 'Task',
        summary: name,
        priority: 'Medium',
      },
    })
    expect(issue.ok(), await issue.text()).toBeTruthy()
    const { id } = (await issue.json()) as { id: string }

    await page.setViewportSize({ width: 375, height: 812 })
    await signInAt(page, `http://localhost:7722/issues/${id}`, account)
    await expect(page.getByRole('heading', { name })).toBeVisible()
    await expect(page.locator(`a[href="/projects/${key}/backlog"]`).first()).toHaveAttribute(
      'href',
      `/projects/${key}/backlog`,
    )
    const issueOverflow = await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    )
    expect(issueOverflow).toBeLessThanOrEqual(1)
    await page.screenshot({ path: testInfo.outputPath('issue-mobile.png'), fullPage: true })

    await page.goto(`http://localhost:7722/projects/${key}/board`)
    const card = page.locator('article').filter({ hasText: name })
    await expect(card).toBeVisible()
    await expect(card).toHaveCount(1)
    const mobileBoard = await page.evaluate(() => ({
      overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      height: document.documentElement.scrollHeight,
    }))
    expect(mobileBoard.overflow).toBeLessThanOrEqual(1)
    expect(mobileBoard.height).toBeLessThan(32767)

    await page.setViewportSize({ width: 1280, height: 800 })
    await page.reload()
    await expect(card).toBeVisible()
    await expect(card).toHaveCount(1)
    const desktopOverflow = await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    )
    expect(desktopOverflow).toBeLessThanOrEqual(1)

    await page.goto('http://localhost:7722/admin')
    await expect(
      page.getByRole('tab', { name: /настройки инстанса|instance settings/i }),
    ).toBeVisible()
    await expect(page.getByRole('tab', { name: /журнал аудита|audit log/i })).toBeVisible()
    await expect(page.getByRole('tab', { name: /пользователи|users/i })).toHaveCount(0)
    await expect(
      page.getByRole('button', { name: /создать пользователя|create user/i }),
    ).toHaveCount(0)
  } finally {
    if (created) {
      const removed = await request.delete(`${api}/projects/${key}`, { headers })
      expect(removed.ok(), await removed.text()).toBeTruthy()
    }
  }
})

test('local Task user and password APIs are closed in SSO mode', async ({ request }) => {
  const api = 'http://localhost:7721/api/v1'
  for (const [path, data] of [
    ['auth/register', { email: 'qa-invalid@example.test', username: 'qa-invalid', password: 'x' }],
    ['auth/login', { email: 'nobody@example.test', password: 'wrong' }],
    ['auth/refresh', {}],
    ['auth/password/request', { email: 'invalid' }],
    ['auth/password/reset', { token: 'invalid', new_password: 'x' }],
    ['admin/users', { email: 'qa-invalid@example.test', password: 'x' }],
  ] as const) {
    const response = await request.post(`${api}/${path}`, { data })
    expect(response.status(), path).toBe(404)
  }
  const localUsers = await request.get(`${api}/admin/users`)
  expect(localUsers.status()).toBe(404)
})
