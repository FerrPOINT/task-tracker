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
          fileURLToPath(new URL('../../../.local/qa-session.json', import.meta.url)),
          'utf8',
        ),
      ) as { email: string; password: string; runId: string })
    : { email: '', password: '', runId: '' }
const screenshotDir = fileURLToPath(
  new URL('../../../.local/screenshots/task-pages/', import.meta.url),
)
const appUrl = 'http://localhost:7722'
const apiUrl = 'http://localhost:7721/api/v1'
const viewports = [
  [375, 812],
  [768, 1024],
  [1280, 800],
  [1920, 1080],
] as const

test('Task Tracker routes remain usable across themes and viewports', async ({ page, request }) => {
  test.setTimeout(900_000)
  mkdirSync(screenshotDir, { recursive: true })
  const login = await request.post('http://localhost:7701/auth/login', {
    data: { email: account.email, password: account.password },
  })
  expect(login.ok(), await login.text()).toBeTruthy()
  const { access_token } = (await login.json()) as { access_token: string }
  const headers = { Authorization: `Bearer ${access_token}` }
  const key = `Q${Date.now().toString(36).slice(-7).toUpperCase()}`
  const name = `QA ${account.runId} route matrix`
  let projectCreated = false
  let sprintId = ''

  try {
    const project = await request.post(`${apiUrl}/projects`, {
      headers,
      data: { key, name, description: 'Temporary live route matrix fixture' },
    })
    expect(project.ok(), await project.text()).toBeTruthy()
    projectCreated = true
    const issue = await request.post(`${apiUrl}/issues`, {
      headers,
      data: { project_key: key, issue_type: 'Task', summary: `${name} issue`, priority: 'Medium' },
    })
    expect(issue.ok(), await issue.text()).toBeTruthy()
    const issueId = ((await issue.json()) as { id: string }).id
    const sprint = await request.post(`${apiUrl}/projects/${key}/sprints`, {
      headers,
      data: { name: `${name} sprint`, goal: 'Route matrix' },
    })
    expect(sprint.ok(), await sprint.text()).toBeTruthy()
    sprintId = ((await sprint.json()) as { id: string }).id
    const assigned = await request.post(`${apiUrl}/projects/${key}/sprints/${sprintId}/issues`, {
      headers,
      data: { issue_id: issueId },
    })
    expect(assigned.ok(), await assigned.text()).toBeTruthy()
    const started = await request.post(`${apiUrl}/projects/${key}/sprints/${sprintId}/start`, {
      headers,
    })
    expect(started.ok(), await started.text()).toBeTruthy()

    await signInAt(page, `${appUrl}/projects/${key}/board`, account)
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

    const routes = [
      { key: 'dashboard', path: '/' },
      { key: 'projects', path: '/projects' },
      { key: 'board', path: `/projects/${key}/board` },
      { key: 'backlog', path: `/projects/${key}/backlog` },
      { key: 'trash', path: `/projects/${key}/trash` },
      { key: 'custom-fields', path: `/projects/${key}/settings/custom-fields` },
      { key: 'search', path: '/search' },
      { key: 'notifications', path: '/notifications' },
      { key: 'reports', path: '/reports' },
      { key: 'admin', path: '/admin' },
      { key: 'issue-create', path: '/issues/create' },
      { key: 'issue-detail', path: `/issues/${issueId}` },
    ]

    for (const theme of ['dark', 'gray', 'light'] as const) {
      for (let step = 0; step < 3; step++) {
        if ((await page.locator('html').getAttribute('data-theme')) === theme) break
        await page
          .getByRole('button', { name: /Тема:|Theme:|Переключить тему|Switch theme/ })
          .click()
      }
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
      for (const route of routes) {
        await page.goto(`${appUrl}${route.path}`)
        await expect(page.getByRole('button', { name: 'Открыть список сервисов' })).toBeVisible()
        await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible()
        await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
        const routeViewports =
          route.key === 'issue-detail' ? [...viewports, [1024, 900] as const] : viewports
        for (const [width, height] of routeViewports) {
          await page.setViewportSize({ width, height })
          const overflow = await page.evaluate(
            () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
          )
          expect(
            overflow,
            `${route.key} ${theme} ${width}px document overflow`,
          ).toBeLessThanOrEqual(1)
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
          expect(nestedScrollers, `${route.key} ${theme} ${width}px nested scrollers`).toEqual([])
          if (route.key === 'issue-detail' && width === 1024) {
            const smallTargets = await page
              .locator('main button, main select, main [role="tab"]')
              .evaluateAll((elements) =>
                elements
                  .map((element) => {
                    const box = element.getBoundingClientRect()
                    return {
                      name: (element.getAttribute('aria-label') ?? element.textContent ?? '')
                        .trim()
                        .slice(0, 60),
                      width: box.width,
                      height: box.height,
                    }
                  })
                  .filter(
                    (target) =>
                      target.width > 0 &&
                      target.height > 0 &&
                      (target.width < 40 || target.height < 40),
                  ),
              )
            expect(smallTargets, `issue-detail ${theme} 1024px touch targets`).toEqual([])
          }
          await page.screenshot({
            path: `${screenshotDir}/${route.key}-${theme}-${width}.png`,
            fullPage: true,
            animations: 'disabled',
          })
          const axe = await new AxeBuilder({ page }).analyze()
          expect(
            axe.violations
              .filter(
                (violation) => violation.impact === 'serious' || violation.impact === 'critical',
              )
              .map((violation) => ({
                id: violation.id,
                count: violation.nodes.length,
                nodes: violation.nodes.slice(0, 5).map((node) => ({
                  target: node.target,
                  html: node.html.slice(0, 180),
                  reason: node.failureSummary,
                })),
              })),
            `${route.key} ${theme} ${width}px accessibility`,
          ).toEqual([])
        }
      }
    }
    expect(runtimeErrors, 'Console, page, network and server errors').toEqual([])
  } finally {
    if (sprintId) {
      await request.post(`${apiUrl}/projects/${key}/sprints/${sprintId}/close`, { headers })
    }
    if (projectCreated) {
      await request.delete(`${apiUrl}/projects/${key}`, { headers })
    }
  }
})
