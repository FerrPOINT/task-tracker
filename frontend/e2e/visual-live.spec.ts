import { mkdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import AxeBuilder from '@axe-core/playwright'
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
      })
    : { email: '', password: '' }
const screenshotDir = fileURLToPath(new URL('../../../.local/screenshots/', import.meta.url))
const apps = [
  { key: 'admin', url: 'http://localhost:7772/users' },
  { key: 'ci', url: 'http://localhost:7712/projects' },
  { key: 'task-board', url: 'http://localhost:7722/' },
  { key: 'wiki', url: 'http://localhost:7732/spaces' },
  { key: 'fleet', url: 'http://localhost:7742/agents' },
  { key: 'workflow', url: 'http://localhost:7752/workflows' },
] as const

for (const app of apps) {
  test(`${app.key} fits four viewports in three themes without serious accessibility errors`, async ({
    page,
    request,
  }) => {
    test.setTimeout(180_000)
    mkdirSync(screenshotDir, { recursive: true })
    let appUrl = app.url
    let taskProjectKey: string | undefined
    let taskHeaders: { Authorization: string } | undefined
    const runtimeErrors: string[] = []
    try {
      if (app.key === 'task-board') {
        const login = await request.post('http://localhost:7701/auth/login', {
          data: { email: account.email, password: account.password },
        })
        expect(login.ok(), await login.text()).toBeTruthy()
        const { access_token: token } = (await login.json()) as { access_token: string }
        taskHeaders = { Authorization: `Bearer ${token}` }
        taskProjectKey = `QV${Date.now().toString(36).slice(-7).toUpperCase()}`
        const created = await request.post('http://localhost:7721/api/v1/projects', {
          headers: taskHeaders,
          data: {
            key: taskProjectKey,
            name: `QA visual ${taskProjectKey}`,
            description: 'QA visual board smoke',
          },
        })
        expect(created.ok(), await created.text()).toBeTruthy()
        appUrl = `http://localhost:7722/projects/${taskProjectKey}/board`
      }

      await signInAt(page, 'http://localhost:7772/users', account)
      page.on('pageerror', (error) => runtimeErrors.push(`page: ${error.message}`))
      page.on('console', (message) => {
        if (message.type() === 'error') runtimeErrors.push(`console: ${message.text()}`)
      })
      page.on('requestfailed', (failedRequest) => {
        const reason = failedRequest.failure()?.errorText ?? 'unknown'
        if (!reason.includes('ERR_ABORTED')) {
          runtimeErrors.push(
            `request: ${failedRequest.method()} ${failedRequest.url()} (${reason})`,
          )
        }
      })

      await page.goto(appUrl)
      if (app.key === 'workflow') {
        await expect(page.locator('details.service-menu summary')).toBeVisible({ timeout: 30_000 })
      } else {
        await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible({
          timeout: 30_000,
        })
      }
      const pageContent =
        app.key === 'workflow'
          ? page.locator('#workflowForm')
          : page.getByRole('heading', { level: 1 }).first()
      await expect(pageContent).toBeVisible({ timeout: 30_000 })
      for (const theme of ['dark', 'gray', 'light'] as const) {
        if (app.key === 'workflow') {
          await page.getByRole('combobox', { name: 'Тема' }).selectOption(theme)
        } else {
          for (let step = 0; step < 3; step++) {
            if ((await page.locator('html').getAttribute('data-theme')) === theme) break
            await page
              .getByRole('button', { name: /Тема:|Theme:|Переключить тему|Switch theme/ })
              .click({ timeout: 10_000 })
          }
        }
        await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
        for (const [width, height] of [
          [375, 812],
          [768, 1024],
          [1280, 800],
          [1920, 1080],
        ]) {
          await page.setViewportSize({ width, height })
          await expect(pageContent).toBeVisible({ timeout: 10_000 })
          await page.waitForTimeout(1_000)
          await expect
            .poll(
              () =>
                page.evaluate(
                  () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
                ),
              { message: `${app.key} ${theme} ${width}px overflow` },
            )
            .toBeLessThanOrEqual(1)
          const nestedScrollers = await page.evaluate(() =>
            [...document.querySelectorAll<HTMLElement>('*')]
              .filter((element) => {
                if (element === document.body || element === document.documentElement) return false
                if (!element.getClientRects().length) return false
                const style = getComputedStyle(element)
                const scrollsX =
                  /^(auto|scroll)$/.test(style.overflowX) &&
                  element.scrollWidth > element.clientWidth + 1
                const scrollsY =
                  /^(auto|scroll)$/.test(style.overflowY) &&
                  element.scrollHeight > element.clientHeight + 1
                return scrollsX || scrollsY
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
          expect(nestedScrollers, `${app.key} ${theme} ${width}px nested scrollers`).toEqual([])
          if (app.key === 'fleet' && theme === 'dark' && width === 375) {
            const navigation = page.getByRole('button', {
              name: /Открыть навигацию|Open navigation/,
            })
            await navigation.click()
            await page
              .getByRole('dialog')
              .getByRole('link', { name: /Настройки|Settings/, exact: true })
              .click()
            await expect(page).toHaveURL('http://localhost:7742/settings')
            await navigation.click()
            await page
              .getByRole('dialog')
              .getByRole('link', { name: /Агенты|Agents/, exact: true })
              .click()
            await expect(page).toHaveURL(appUrl)
          }
          await page.screenshot({
            path: `${screenshotDir}/sso-${app.key}-${theme}-${width}.png`,
            fullPage: true,
            animations: 'disabled',
          })
          const result = await new AxeBuilder({ page }).analyze()
          expect(
            result.violations
              .filter((issue) => issue.impact === 'serious' || issue.impact === 'critical')
              .map((issue) => ({
                id: issue.id,
                impact: issue.impact,
                count: issue.nodes.length,
                targets: issue.nodes.slice(0, 5).map((node) => node.target),
              })),
            `${app.key} ${theme} ${width}px`,
          ).toEqual([])
        }
      }
      expect(runtimeErrors, 'Console, page and network errors').toEqual([])
    } finally {
      if (taskProjectKey && taskHeaders) {
        const removed = await request.delete(
          `http://localhost:7721/api/v1/projects/${taskProjectKey}`,
          { headers: taskHeaders },
        )
        expect(removed.ok(), await removed.text()).toBeTruthy()
      }
    }
  })
}
