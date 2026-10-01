import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'
import { signInAt } from './qa-login'

test.skip(process.env.SDLC_LIVE_QA !== '1', 'Requires real Central Auth and Task Tracker')
test.skip(({ browserName }) => browserName !== 'chromium', 'Single browser live acceptance')
test.use({ trace: 'off', hasTouch: true })

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
      ) as { email: string; password: string; runId: string })
    : { email: '', password: '', runId: '' }
const appUrl = 'http://localhost:7722'
const apiUrl = 'http://localhost:7721/api/v1'
const evidenceDir =
  process.env.SDLC_HEADER_EVIDENCE_DIR ??
  fileURLToPath(new URL('../../../.local/screenshots/task-header/', import.meta.url))

test('real Task header preserves context, actions, SSO and accessible responsive geometry', async ({
  page,
  request,
}) => {
  test.setTimeout(600_000)
  mkdirSync(evidenceDir, { recursive: true })
  const login = await request.post('http://localhost:7701/auth/login', {
    data: { email: account.email, password: account.password },
  })
  expect(login.ok(), 'Operator authentication').toBeTruthy()
  const { access_token } = (await login.json()) as { access_token: string }
  const headers = { Authorization: `Bearer ${access_token}` }
  const key = `Q${Date.now().toString(36).slice(-7).toUpperCase()}`
  const name = `QA ${account.runId} длинное название проекта для проверки контекста шапки`
  const created = await request.post(`${apiUrl}/projects`, { headers, data: { key, name } })
  expect(created.ok(), 'Temporary QA project created').toBeTruthy()
  const runtimeErrors: string[] = []
  const cases: { route: string; theme: string; width: number }[] = []
  try {
    await signInAt(page, `${appUrl}/projects/${key}/board`, account)
    page.on('pageerror', (error) => runtimeErrors.push(`page: ${error.message}`))
    page.on('console', (message) => {
      if (message.type() === 'error') runtimeErrors.push(`console: ${message.text()}`)
    })
    page.on('requestfailed', (failed) => {
      if (!failed.failure()?.errorText.includes('ERR_ABORTED'))
        runtimeErrors.push(`request: ${failed.method()} ${new URL(failed.url()).pathname}`)
    })
    page.on('response', (response) => {
      if (response.status() >= 400)
        runtimeErrors.push(`HTTP ${response.status()}: ${new URL(response.url()).pathname}`)
    })
    const routes = [
      { key: 'dashboard', path: '/' },
      { key: 'board', path: `/projects/${key}/board` },
      { key: 'create', path: `/issues/create?project_key=${key}` },
    ]
    for (const theme of ['dark', 'gray', 'light']) {
      for (let step = 0; step < 3; step++) {
        if ((await page.locator('html').getAttribute('data-theme')) === theme) break
        await page.getByRole('button', { name: /Тема:|Theme:/ }).click()
      }
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
      for (const route of routes) {
        await page.goto(`${appUrl}${route.path}`)
        await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible()
        for (const width of [320, 375, 767, 768, 1023, 1024, 1279, 1280, 1440, 1920, 2560]) {
          await page.setViewportSize({ width, height: width >= 1920 ? 1080 : 812 })
          const header = page.locator('[data-platform-header]')
          await expect(header).toHaveCount(1)
          const geometry = await header.evaluate((element) => {
            const box = element.getBoundingClientRect()
            const controls = [...element.querySelectorAll<HTMLElement>('button,a')]
              .filter(
                (control) =>
                  control.getClientRects().length &&
                  getComputedStyle(control).visibility !== 'hidden',
              )
              .map((control) => {
                const rect = control.getBoundingClientRect()
                return { x: rect.x, right: rect.right, width: rect.width, height: rect.height }
              })
            return {
              height: box.height,
              controls,
              slots: [...element.querySelectorAll('[data-platform-header-slot]')].map((slot) =>
                slot.getAttribute('data-platform-header-slot'),
              ),
              overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
            }
          })
          expect(geometry.height).toBe(60)
          expect(geometry.slots).toEqual(['leading', 'services', 'context', 'actions'])
          expect(geometry.overflow, `${route.key} ${theme} ${width}px`).toBeLessThanOrEqual(1)
          for (const [index, control] of geometry.controls.entries()) {
            expect(control.height).toBeGreaterThanOrEqual(width < 768 ? 44 : 40)
            expect(control.width).toBeGreaterThanOrEqual(width < 768 ? 44 : 40)
            expect(control.x).toBeGreaterThanOrEqual(0)
            expect(control.right).toBeLessThanOrEqual(width)
            if (index) expect(control.x).toBeGreaterThanOrEqual(geometry.controls[index - 1].right)
          }
          await expect(
            header.getByRole('button', {
              name: 'Открыть список сервисов: Task Tracker',
              exact: true,
            }),
          ).toBeVisible()
          const axe = await new AxeBuilder({ page }).include('[data-platform-header]').analyze()
          expect(
            axe.violations.filter((v) => v.impact === 'serious' || v.impact === 'critical'),
          ).toEqual([])
          await page.screenshot({
            path: `${evidenceDir}/${route.key}-${theme}-${width}.png`,
            fullPage: true,
            animations: 'disabled',
          })
          cases.push({ route: route.key, theme, width })
        }
      }
    }

    await page.goto(`${appUrl}/projects/${key}/board`)
    await page.setViewportSize({ width: 1280, height: 800 })
    const picker = page.getByRole('banner').getByRole('button', { name: 'Проекты', exact: true })
    await expect(picker).toHaveAttribute('title', name)
    await picker.click()
    await expect(
      page.getByRole('menu').getByRole('menuitem', { name: new RegExp(key) }),
    ).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(picker).toBeFocused()
    await page.getByRole('banner').getByRole('link', { name: 'Создать', exact: true }).click()
    await expect(page).toHaveURL(`${appUrl}/issues/create?project_key=${key}`)

    for (const width of [375, 2560]) {
      await page.setViewportSize({ width, height: 812 })
      const trigger = page.getByRole('button', {
        name: 'Открыть список сервисов: Task Tracker',
        exact: true,
      })
      await trigger.focus()
      await page.keyboard.press('Enter')
      const menu = page.getByRole('menu')
      await expect(menu.getByRole('menuitem')).toHaveCount(6)
      await expect(menu.getByRole('img', { name: 'Работает', exact: true })).toHaveCount(6)
      for (const [index, label] of [
        'Admin Panel',
        'CI/CD',
        'Task Tracker',
        'Wiki',
        'Fleet Control',
        'Project Workflow',
      ].entries()) {
        await expect(menu.getByRole('menuitem').nth(index)).toContainText(label)
      }
      await expect(menu.getByRole('menuitem', { name: /Task Tracker/ })).toHaveAttribute(
        'aria-disabled',
        'true',
      )
      await expect(
        menu.getByRole('menuitem', { name: /Central Auth|Java Agent|Pulse/ }),
      ).toHaveCount(0)
      await page.keyboard.press('ArrowDown')
      await page.keyboard.press('Escape')
      await expect(trigger).toBeFocused()
      expect(await page.locator('#root').evaluate((root) => root.inert)).toBe(false)
      await trigger.tap()
      await expect(menu).toBeVisible()
      await page.evaluate(
        () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())),
      )
      const background = await menu.evaluate((element) => getComputedStyle(element).backgroundColor)
      expect(background).toMatch(/^rgb\(|^color\(/)
      await page.screenshot({
        path: `${evidenceDir}/services-light-${width}.png`,
        fullPage: true,
        animations: 'disabled',
      })
      await page.touchscreen.tap(width - 8, 300)
      await expect(menu).toBeHidden()
      await expect(trigger).toBeFocused()
    }

    await page.goto(`${appUrl}/projects/${key}/board`)
    await page.setViewportSize({ width: 375, height: 812 })
    const drawerTrigger = page.getByRole('button', { name: 'Открыть меню' })
    await drawerTrigger.tap()
    const drawer = page.getByRole('dialog')
    await expect(drawer.getByRole('link', { name: 'Создать', exact: true })).toHaveAttribute(
      'href',
      `/issues/create?project_key=${key}`,
    )
    await page.keyboard.press('Escape')
    await expect(drawerTrigger).toBeFocused()
    await drawerTrigger.tap()
    await drawer.getByRole('link', { name: 'Создать', exact: true }).tap()
    await expect(page).toHaveURL(`${appUrl}/issues/create?project_key=${key}`)
    await expect(drawer).toBeHidden()

    await page.getByTestId('notification-trigger').tap()
    await expect(page.getByRole('menu')).toBeVisible()
    await page.getByRole('menuitem', { name: 'Все уведомления', exact: true }).tap()
    await expect(page).toHaveURL(`${appUrl}/notifications`)
    const profile = page.getByRole('button', { name: 'Аккаунт' })
    await profile.tap()
    await expect(page.getByRole('menuitem', { name: 'Администрирование' })).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(profile).toBeFocused()
    await page.screenshot({ path: `${evidenceDir}/notifications-light-375.png`, fullPage: true })
    expect(runtimeErrors).toEqual([])
    await profile.tap()
    await page.getByRole('menuitem', { name: 'Выйти', exact: true }).tap()
    await expect(page).toHaveURL(/localhost:7701\/oidc\/logout\?client_id=task-tracker/)
    await page.getByRole('button', { name: 'Выйти из всех приложений', exact: true }).tap()
    await expect(page).toHaveURL(/localhost:7722\/login\?logged_out/)
    await page.goto(`${appUrl}/projects`, { waitUntil: 'commit' })
    await expect(page).toHaveURL(/localhost:7701\/oidc\/authorize/)
    await expect(page.getByLabel('Пароль')).toBeVisible()
    writeFileSync(
      `${evidenceDir}/results.json`,
      JSON.stringify(
        {
          mock_api: false,
          retries: test.info().retry,
          cases,
          runtime_errors: runtimeErrors,
          interactions: [
            'desktop project picker/create',
            'mobile contextual create',
            'service keyboard/touch/outside/focus',
            'notification navigation',
            'profile',
            'central logout/re-entry',
          ],
        },
        null,
        2,
      ),
    )
  } finally {
    const deleted = await request.delete(`${apiUrl}/projects/${key}`, { headers })
    expect(deleted.ok(), 'Own temporary project removed through API').toBeTruthy()
  }
})
