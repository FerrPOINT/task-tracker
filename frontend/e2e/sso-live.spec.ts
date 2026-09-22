import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { randomUUID } from 'node:crypto'
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
            fileURLToPath(new URL('../../../.local/qa-session.json', import.meta.url)),
          'utf8',
        ),
      ) as {
        email: string
        password: string
      })
    : { email: '', password: '' }

test('Admin Panel uses the central browser session and global logout', async ({ page }) => {
  await signInAt(page, 'http://localhost:7772/users', account)

  await expect(page).toHaveURL('http://localhost:7772/users', { timeout: 30_000 })
  await expect(page.getByRole('banner').getByText(account.email, { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Выйти' }).click()
  await expect(page).toHaveURL(/localhost:7701\/oidc\/logout/)
  await page.getByRole('button', { name: /выйти|подтвердить/i }).click()
  await expect(page).toHaveURL(/localhost:7772\/login/)
})

test('switches from Admin Panel to Task Tracker with one central login', async ({ page }) => {
  await signInAt(page, 'http://localhost:7772/users', account)
  await expect(page).toHaveURL('http://localhost:7772/users', { timeout: 30_000 })

  await page.getByRole('button', { name: 'Открыть список сервисов' }).click()
  await page.getByRole('menuitem', { name: /Task Tracker/ }).click()
  await expect(page).toHaveURL(/localhost:7722\//, { timeout: 30_000 })
  await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible()
  await page.reload()
  await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible()

  await page.goto('http://localhost:7772/users')
  await page.getByRole('button', { name: 'Выйти' }).click()
  await page.getByRole('button', { name: /выйти|подтвердить/i }).click()
  await expect(page).toHaveURL(/localhost:7772\/login/)
  await page.goto('http://localhost:7722/')
  await expect(page.getByRole('heading', { name: 'Вход в SDLC' })).toBeVisible({ timeout: 30_000 })
})

test('switches from Admin Panel to Wiki without another password prompt', async ({
  page,
  request,
}) => {
  await signInAt(page, 'http://localhost:7772/users', account)
  await expect(page).toHaveURL('http://localhost:7772/users', { timeout: 30_000 })

  await page.getByRole('button', { name: 'Открыть список сервисов' }).click()
  await page.getByRole('menuitem', { name: /Wiki/ }).click()
  await expect(page).toHaveURL(/localhost:7732\//, { timeout: 30_000 })
  await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible()
  await page.reload()
  await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible()

  expect(
    (
      await request.post('http://localhost:7731/api/v1/auth/login', {
        data: { email: account.email, password: account.password },
      })
    ).status(),
  ).toBe(404)
  expect(
    (
      await request.post('http://localhost:7731/api/v1/auth/register', {
        data: { email: 'unused@example.test', password: account.password },
      })
    ).status(),
  ).toBe(404)
})

test('switches from Admin Panel to Fleet Control without another password prompt', async ({
  page,
  request,
}) => {
  await signInAt(page, 'http://localhost:7772/users', account)
  await expect(page).toHaveURL('http://localhost:7772/users', { timeout: 30_000 })

  await page.getByRole('button', { name: 'Открыть список сервисов' }).click()
  await page.getByRole('menuitem', { name: /Fleet Control/ }).click()
  await expect(page).toHaveURL(/localhost:7742\//, { timeout: 30_000 })
  await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible()
  await page.reload()
  await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible()

  const login = await request.post('http://localhost:7741/api/v1/auth/login', {
    data: { email: account.email, password: account.password },
  })
  expect(login.status()).toBe(401)
  const register = await request.post('http://localhost:7741/api/v1/auth/register', {
    data: {
      email: 'unused@example.test',
      username: 'unused',
      display_name: 'Unused',
      password: account.password,
    },
  })
  expect(register.status()).toBe(403)
})

test('switches from Admin Panel to CI/CD without another password prompt', async ({
  page,
  request,
}) => {
  await signInAt(page, 'http://localhost:7772/users', account)
  await expect(page).toHaveURL('http://localhost:7772/users', { timeout: 30_000 })

  await page.getByRole('button', { name: 'Открыть список сервисов' }).click()
  await page.getByRole('menuitem', { name: /CI\/CD/ }).click()
  await expect(page).toHaveURL(/localhost:7712\//, { timeout: 30_000 })
  await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible({
    timeout: 30_000,
  })
  await page.reload()
  await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible()

  const login = await request.post('http://localhost:7711/api/v1/auth/login', {
    data: { username: account.email, password: account.password },
  })
  expect(login.status()).toBe(403)
})

test('switches from Admin Panel to Project Workflow without another password prompt', async ({
  page,
  request,
}) => {
  await signInAt(page, 'http://localhost:7772/users', account)
  await expect(page).toHaveURL('http://localhost:7772/users', { timeout: 30_000 })

  await page.getByRole('button', { name: 'Открыть список сервисов' }).click()
  await page.getByRole('menuitem', { name: /Project Workflow/ }).click()
  await expect(page).toHaveURL(/localhost:8812\//, { timeout: 30_000 })
  await expect(page.locator('details.service-menu summary')).toBeVisible({ timeout: 30_000 })
  await page.reload()
  await expect(page.locator('details.service-menu summary')).toBeVisible()

  const anonymous = await request.get('http://localhost:8812/api/workflows')
  expect(anonymous.status()).toBe(401)
})

test('all six switchers expose only UI services and navigate with one session', async ({
  page,
}) => {
  test.setTimeout(90_000)
  await signInAt(page, 'http://localhost:7772/users', account)
  await expect(page).toHaveURL('http://localhost:7772/users', { timeout: 30_000 })

  const services = [
    { name: 'Admin Panel', port: 7772 },
    { name: 'CI/CD', port: 7712 },
    { name: 'Task Tracker', port: 7722 },
    { name: 'Wiki', port: 7732 },
    { name: 'Fleet Control', port: 7742 },
    { name: 'Project Workflow', port: 8812 },
  ]
  for (let index = 0; index < services.length; index++) {
    const current = services[index]
    const next = services[(index + 1) % services.length]
    await expect(page).toHaveURL(new RegExp(`localhost:${current.port}/`), { timeout: 30_000 })
    if (current.name === 'Project Workflow') {
      await page.locator('details.service-menu summary').click()
    } else {
      await page.getByRole('button', { name: 'Открыть список сервисов' }).click()
    }
    const menu = page.getByRole('menu')
    await expect(menu.getByRole('menuitem')).toHaveCount(6)
    await expect(menu.getByRole('menuitem', { name: /Central Auth|Java Agent/ })).toHaveCount(0)
    const target = menu.getByRole('menuitem', { name: new RegExp(next.name.replace('/', '\\/')) })
    const href = await target.getAttribute('href')
    expect(href).toContain(`localhost:${next.port}`)
    expect(href).not.toMatch(/token|code|access_token/i)
    await target.click()
    if (next.name === 'Project Workflow') {
      await expect(page.locator('details.service-menu summary')).toBeVisible({ timeout: 30_000 })
    } else {
      await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible({
        timeout: 30_000,
      })
    }
  }
})

test('service switcher restores keyboard focus after Escape', async ({ page }) => {
  await signInAt(page, 'http://localhost:7722/', account)
  const trigger = page.getByRole('button', { name: 'Открыть список сервисов' })
  await trigger.focus()
  await page.keyboard.press('Enter')
  const menu = page.getByRole('menu')
  await expect(menu).toBeVisible()
  await page.keyboard.press('ArrowDown')
  await expect(menu.getByRole('menuitem').nth(1)).toBeFocused()
  await page.keyboard.press('Escape')
  await expect(menu).toBeHidden()
  await expect(trigger).toBeFocused()
})

test('service switcher opens by touch at mobile width', async ({ browser }) => {
  const context = await browser.newContext({
    hasTouch: true,
    viewport: { width: 375, height: 812 },
  })
  try {
    const page = await context.newPage()
    await signInAt(page, 'http://localhost:7722/', account)
    await page.getByRole('button', { name: 'Открыть список сервисов' }).tap()
    await expect(page.getByRole('menu').getByRole('menuitem')).toHaveCount(6)
  } finally {
    await context.close()
  }
})

test('managed user receives a one-use setup link and loses access when disabled', async ({
  page,
  request,
}) => {
  test.setTimeout(90_000)
  const password = `Qa-${randomUUID()}-A1`
  await signInAt(page, 'http://localhost:7772/users', account)
  await expect(page).toHaveURL('http://localhost:7772/users', { timeout: 30_000 })

  const operator = await request.post('http://localhost:7701/auth/login', {
    data: { email: account.email, password: account.password },
  })
  expect(operator.ok()).toBeTruthy()
  const { access_token: operatorToken } = (await operator.json()) as { access_token: string }
  const operatorHeaders = { Authorization: `Bearer ${operatorToken}` }
  const existingUsers = await request.get('http://localhost:7701/auth/users?q=qa-sso-', {
    headers: operatorHeaders,
  })
  expect(existingUsers.ok()).toBeTruthy()
  const reusable = (
    (await existingUsers.json()) as {
      id: string
      email: string
      display_name: string
      status: string
    }[]
  )
    .filter((user) => user.status === 'disabled' && user.display_name === 'QA SSO User')
    .sort((left, right) => left.email.localeCompare(right.email))[0]
  const email = reusable?.email ?? `qa-sso-${Date.now()}@example.test`
  const mailboxBefore = await request.get('http://127.0.0.1:7802/api/v1/messages?limit=100')
  expect(mailboxBefore.ok()).toBeTruthy()
  const previousMessageIds = new Set(
    ((await mailboxBefore.json()) as { messages: { ID: string }[] }).messages.map(
      (message) => message.ID,
    ),
  )

  let disabled = false
  try {
    if (reusable) {
      const restored = await request.post(
        `http://localhost:7701/auth/users/${reusable.id}/status`,
        {
          headers: operatorHeaders,
          data: { enabled: true },
        },
      )
      expect(restored.ok()).toBeTruthy()
      const resent = await request.post(
        `http://localhost:7701/auth/users/${reusable.id}/password-link`,
        { headers: operatorHeaders },
      )
      expect(resent.status()).toBe(204)
    } else {
      await page.getByRole('button', { name: 'Добавить' }).first().click()
      const dialog = page.getByRole('dialog')
      await dialog.getByLabel('Email').fill(email)
      await dialog.getByLabel('Имя').fill('QA SSO User')
      await dialog.getByRole('button', { name: 'Добавить' }).click()
      await expect(dialog).toBeHidden()
    }
    const filtered = page.waitForResponse((response) =>
      response.url().includes(`/api/v1/users?q=${encodeURIComponent(email)}`),
    )
    const search = page.getByPlaceholder('Имя или email')
    await search.fill(email)
    await expect(search).toHaveValue(email)
    expect((await filtered).ok()).toBeTruthy()
    await expect(page.getByText(email, { exact: false })).toBeVisible()
    if (!reusable) {
      await page.getByRole('link', { name: 'Аудит' }).click()
      await page.getByRole('combobox', { name: 'Тип сущности' }).selectOption('central_user')
      await page.getByRole('combobox', { name: 'Действие' }).selectOption('central_user.created')
      await expect(page.locator('summary').getByText('Добавлен пользователь').first()).toBeVisible()
    }
    const profileRequest = page.waitForRequest(
      (req) => req.url().includes('/api/v1/users/me') && Boolean(req.headers().authorization),
    )
    await page.goto('http://localhost:7722/')
    await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible({
      timeout: 30_000,
    })
    const profile = await profileRequest
    const taskDirectory = await request.get('http://localhost:7721/api/v1/users', {
      headers: { Authorization: profile.headers().authorization },
    })
    expect(taskDirectory.ok()).toBeTruthy()
    const { users: assignable } = (await taskDirectory.json()) as {
      users: { id: string; display_name: string }[]
    }
    expect(
      assignable.some((user) => user.display_name === 'QA SSO User' && Boolean(user.id)),
    ).toBeTruthy()
    const wikiDirectory = await request.get('http://localhost:7731/api/v1/users', {
      headers: { Authorization: profile.headers().authorization },
    })
    expect(wikiDirectory.ok()).toBeTruthy()
    const { users: wikiAssignable } = (await wikiDirectory.json()) as {
      users: { id: string; email: string; active: boolean }[]
    }
    expect(
      wikiAssignable.some((user) => user.email === email && user.active && Boolean(user.id)),
    ).toBeTruthy()
    const fleetDirectory = await request.get('http://localhost:7741/api/v1/users', {
      headers: { Authorization: profile.headers().authorization },
    })
    expect(fleetDirectory.ok()).toBeTruthy()
    const { users: fleetAssignable } = (await fleetDirectory.json()) as {
      users: { id: string; email: string; is_active: boolean }[]
    }
    expect(
      fleetAssignable.some((user) => user.email === email && user.is_active && Boolean(user.id)),
    ).toBeTruthy()
    const ciDirectory = await request.get('http://localhost:7711/api/v1/users', {
      headers: { Authorization: profile.headers().authorization },
    })
    expect(ciDirectory.ok()).toBeTruthy()
    const ciAssignable = (await ciDirectory.json()) as {
      id: string
      username: string
      enabled: boolean
    }[]
    expect(
      ciAssignable.some((user) => user.username === email && user.enabled && Boolean(user.id)),
    ).toBeTruthy()

    let messageId = ''
    await expect
      .poll(async () => {
        const listing = (await (
          await request.get('http://127.0.0.1:7802/api/v1/messages?limit=100')
        ).json()) as {
          messages: { ID: string; To: { Address: string }[] }[]
        }
        messageId =
          listing.messages.find(
            (item) =>
              !previousMessageIds.has(item.ID) && item.To.some((to) => to.Address === email),
          )?.ID ?? ''
        return messageId
      })
      .not.toBe('')
    const message = (await (
      await request.get(`http://127.0.0.1:7802/api/v1/message/${messageId}`)
    ).json()) as { Text: string }
    const setupLink = message.Text.match(
      /http:\/\/localhost:7701\/auth\/password\/setup#token=[^\s]+/,
    )?.[0]
    expect(setupLink).toBeTruthy()

    await page.goto(setupLink!, { waitUntil: 'domcontentloaded', timeout: 10_000 })
    await page.getByLabel('Новый пароль').fill(password)
    await page.getByRole('button', { name: 'Сохранить пароль' }).click()
    await expect(
      page.getByText('Пароль установлен. Теперь можно войти в любое приложение SDLC.'),
    ).toBeVisible({ timeout: 10_000 })
    const setupToken = new URLSearchParams(new URL(setupLink!).hash.slice(1)).get('token')
    expect(setupToken).toBeTruthy()
    const replay = await request.post('http://localhost:7701/auth/password/setup', {
      data: { token: setupToken, password },
    })
    expect(replay.status()).toBe(401)

    await page.goto('http://localhost:7772/users')
    await page.getByRole('button', { name: 'Выйти' }).click()
    await page.getByRole('button', { name: /выйти|подтвердить/i }).click()
    await signInAt(page, 'http://localhost:7732/', { email, password })
    await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible({
      timeout: 30_000,
    })

    const directory = await request.get(
      `http://localhost:7701/auth/users?q=${encodeURIComponent(email)}`,
      { headers: operatorHeaders },
    )
    const [user] = (await directory.json()) as { id: string }[]
    expect(user).toBeTruthy()
    const status = await request.post(`http://localhost:7701/auth/users/${user.id}/status`, {
      headers: operatorHeaders,
      data: { enabled: false },
    })
    expect(status.ok()).toBeTruthy()
    disabled = true
    const [taskAfter, wikiAfter, fleetAfter, ciAfter] = await Promise.all([
      request.get('http://localhost:7721/api/v1/users', { headers: operatorHeaders }),
      request.get('http://localhost:7731/api/v1/users', { headers: operatorHeaders }),
      request.get('http://localhost:7741/api/v1/users', { headers: operatorHeaders }),
      request.get('http://localhost:7711/api/v1/users', { headers: operatorHeaders }),
    ])
    for (const response of [taskAfter, wikiAfter, fleetAfter, ciAfter])
      expect(response.ok()).toBeTruthy()
    const taskUsers = (await taskAfter.json()) as { users: { display_name: string }[] }
    const wikiUsers = (await wikiAfter.json()) as { users: { email: string; active: boolean }[] }
    const fleetUsers = (await fleetAfter.json()) as { users: { email: string }[] }
    const ciUsers = (await ciAfter.json()) as { username: string }[]
    expect(taskUsers.users.some((user) => user.display_name === 'QA SSO User')).toBeFalsy()
    expect(wikiUsers.users.some((user) => user.email === email && user.active)).toBeFalsy()
    expect(fleetUsers.users.some((user) => user.email === email)).toBeFalsy()
    expect(ciUsers.some((user) => user.username === email)).toBeFalsy()
    await page.reload()
    await expect(page.getByRole('heading', { name: 'Вход в SDLC' })).toBeVisible({
      timeout: 30_000,
    })
  } finally {
    if (!disabled) {
      const directory = await request.get(
        `http://localhost:7701/auth/users?q=${encodeURIComponent(email)}`,
        { headers: operatorHeaders },
      )
      const users = (await directory.json()) as { id: string; status: string }[]
      if (users[0] && users[0].status !== 'disabled') {
        await request.post(`http://localhost:7701/auth/users/${users[0].id}/status`, {
          headers: operatorHeaders,
          data: { enabled: false },
        })
      }
    }
  }
})
