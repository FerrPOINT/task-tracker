import { mkdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'
import { signInAt } from './qa-login'

type AuditEvent = { action: string; entity_type: string }

test.skip(process.env.SDLC_LIVE_QA !== '1', 'Requires the running local SDLC fleet')
test.skip(({ browserName }) => browserName !== 'chromium', 'Single browser live smoke')
test.use({ trace: 'off' })

const screenshotDir = fileURLToPath(new URL('../../../.local/screenshots/', import.meta.url))
const qaAccount =
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
  test.setTimeout(600_000)
  mkdirSync(screenshotDir, { recursive: true })
  await signInAt(page, 'http://localhost:7772/', qaAccount)
  const runtimeErrors: string[] = []
  page.on('pageerror', (error) => runtimeErrors.push(`page: ${error.message}`))
  page.on('console', (message) => {
    if (message.type() === 'error') runtimeErrors.push(`console: ${message.text()}`)
  })
  page.on('requestfailed', (failed) => {
    const reason = failed.failure()?.errorText ?? 'unknown'
    if (!reason.includes('ERR_ABORTED')) {
      runtimeErrors.push(`request: ${failed.method()} ${failed.url()} (${reason})`)
    }
  })
  page.on('response', (response) => {
    if (response.status() >= 500) {
      runtimeErrors.push(`response: ${response.status()} ${response.url()}`)
    }
  })

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
        const nestedScrollers = await page.evaluate(() =>
          [...document.querySelectorAll<HTMLElement>('*')]
            .filter((element) => {
              if (element === document.body || element === document.documentElement) return false
              if (!element.getClientRects().length) return false
              const style = getComputedStyle(element)
              return (
                (/^(auto|scroll)$/.test(style.overflowX) &&
                  element.scrollWidth > element.clientWidth + 1) ||
                (/^(auto|scroll)$/.test(style.overflowY) &&
                  element.scrollHeight > element.clientHeight + 1)
              )
            })
            .map(
              (element) =>
                `${element.tagName.toLowerCase()}${element.id ? `#${element.id}` : ''}.${[
                  ...element.classList,
                ]
                  .slice(0, 3)
                  .join('.')}`,
            ),
        )
        expect(nestedScrollers, `Admin ${path} ${theme} ${width}px nested scrollers`).toEqual([])
        await page.screenshot({
          path: `${screenshotDir}/admin-${path.replaceAll('/', '-') || 'home'}-${theme}-${width}.png`,
          fullPage: true,
          animations: 'disabled',
        })
        const audit = await new AxeBuilder({ page }).analyze()
        expect(
          audit.violations
            .filter((issue) => issue.impact === 'serious' || issue.impact === 'critical')
            .map((issue) => ({
              id: issue.id,
              count: issue.nodes.length,
              nodes: issue.nodes.slice(0, 5).map((node) => ({
                target: node.target,
                reason: node.failureSummary,
              })),
            })),
          `Admin ${path} ${theme} ${width}px`,
        ).toEqual([])
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
  expect(runtimeErrors, 'Admin console, page, network and server errors').toEqual([])
})

test('Admin audit filters and token dialog work against the live API without writes', async ({
  page,
}) => {
  test.setTimeout(150_000)
  await page.setViewportSize({ width: 375, height: 812 })
  const auditResponse = (predicate: (url: URL) => boolean) =>
    page.waitForResponse((response) => {
      const url = new URL(response.url())
      return url.pathname === '/api/v1/audit-events' && predicate(url)
    })
  const first = auditResponse(() => true)
  await signInAt(page, 'http://localhost:7772/audit', qaAccount)
  const firstPage = (await (await first).json()) as { events: AuditEvent[]; total: number }
  expect(firstPage.total).toBeGreaterThanOrEqual(firstPage.events.length)
  await expect(
    page.getByText(`${firstPage.total ? 1 : 0}–${firstPage.events.length} из ${firstPage.total}`),
  ).toBeVisible()

  const mutations: string[] = []
  page.on('request', (request) => {
    if (request.url().startsWith('http://localhost:7771/api/v1/') && request.method() !== 'GET') {
      mutations.push(`${request.method()} ${new URL(request.url()).pathname}`)
    }
  })

  if (firstPage.total > 20) {
    const second = auditResponse((url) => url.searchParams.get('offset') === '20')
    await page.getByRole('button', { name: 'Вперёд' }).click()
    const secondPage = (await (await second).json()) as { events: AuditEvent[]; total: number }
    await expect(
      page.getByText(`21–${20 + secondPage.events.length} из ${secondPage.total}`),
    ).toBeVisible()
  }

  const users = auditResponse(
    (url) =>
      url.searchParams.get('entity_type') === 'central_user' && !url.searchParams.has('action'),
  )
  await page.getByRole('combobox', { name: 'Тип сущности' }).selectOption('central_user')
  const userPage = (await (await users).json()) as { events: AuditEvent[]; total: number }
  expect(userPage.events.every((event) => event.entity_type === 'central_user')).toBe(true)
  expect(userPage.total).toBeGreaterThanOrEqual(userPage.events.length)

  const created = auditResponse((url) => url.searchParams.get('action') === 'central_user.created')
  await page.getByRole('combobox', { name: 'Действие' }).selectOption('central_user.created')
  const createdPage = (await (await created).json()) as { events: AuditEvent[]; total: number }
  expect(createdPage.events.every((event) => event.action === 'central_user.created')).toBe(true)

  const requests: string[] = []
  page.on('request', (request) => {
    if (new URL(request.url()).pathname === '/api/v1/audit-events') requests.push(request.url())
  })
  await page.getByRole('combobox', { name: 'Действие' }).selectOption('custom')
  await page.getByRole('textbox', { name: 'Точный код действия' }).fill('branding.published')
  await page.waitForTimeout(150)
  expect(requests).toEqual([])
  const exact = auditResponse((url) => url.searchParams.get('action') === 'branding.published')
  await page.getByRole('button', { name: 'Применить' }).click()
  const exactPage = (await (await exact).json()) as { events: AuditEvent[]; total: number }
  expect(exactPage).toMatchObject({ events: [], total: 0 })
  await expect(page.getByText('0–0 из 0')).toBeVisible()

  await page.goto('http://localhost:7772/tokens')
  const create = page.getByRole('button', { name: 'Создать', exact: true })
  await create.click()
  const dialog = page.getByRole('dialog')
  const targets = await dialog
    .locator('fieldset label')
    .evaluateAll((labels) => labels.map((label) => label.getBoundingClientRect()))
  expect(targets.length).toBeGreaterThanOrEqual(12)
  expect(targets.every(({ width, height }) => width >= 40 && height >= 40)).toBe(true)
  const pulseScopes = dialog.locator('fieldset div').filter({ hasText: 'Service Pulse' })
  await expect(pulseScopes).toBeVisible()
  await expect(pulseScopes.locator('label')).toHaveCount(2)
  await expect
    .poll(() =>
      page.evaluate(
        () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
      ),
    )
    .toBeLessThanOrEqual(1)
  await page.keyboard.press('Escape')
  await expect(dialog).toBeHidden()
  await expect(create).toBeFocused()
  expect(mutations).toEqual([])
})
