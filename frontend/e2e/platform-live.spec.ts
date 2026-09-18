import { mkdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'

test.skip(
  process.env.SDLC_LIVE_QA !== '1',
  'Requires the local QA bootstrap and running Compose fleet',
)

const account = (
  process.env.SDLC_LIVE_QA === '1'
    ? JSON.parse(
        readFileSync(
          fileURLToPath(new URL('../../../.local/qa-session.json', import.meta.url)),
          'utf8',
        ),
      )
    : { runId: '', email: '', username: '', password: '' }
) as {
  runId: string
  email: string
  username: string
  password: string
}
const projectKey = `QA${account.runId.slice(0, 6).toUpperCase()}`
const screenshotDir = fileURLToPath(new URL('../../../.local/screenshots/', import.meta.url))

const apps = [
  { key: 'admin-panel', label: 'Admin Panel', port: 7772, login: true },
  { key: 'ci-cd', label: 'CI/CD', port: 7712, login: true },
  { key: 'task-tracker', label: 'Task Tracker', port: 7722, login: true },
  { key: 'wiki', label: 'Wiki', port: 7732, login: true },
  { key: 'fleet-control', label: 'Fleet Control', port: 7742, login: true },
  { key: 'project-workflow', label: 'Project Workflow', port: 8812, login: false },
] as const

test.describe('live platform switcher', () => {
  test.describe.configure({ mode: 'serial' })
  let taskToken = ''
  let adminToken = ''

  test.beforeAll(async ({ request }) => {
    const login = await request.post('http://127.0.0.1:7721/api/v1/auth/login', {
      data: { email: account.email, password: account.password },
    })
    expect(login.ok()).toBeTruthy()
    const { access_token: token } = await login.json()
    taskToken = token
    const projects = await request.get('http://127.0.0.1:7721/api/v1/projects', {
      headers: { Authorization: `Bearer ${token}` },
    })
    const existing = (await projects.json()).projects.some(
      (project: { key: string }) => project.key === projectKey,
    )
    if (!existing) {
      const created = await request.post('http://127.0.0.1:7721/api/v1/projects', {
        headers: { Authorization: `Bearer ${token}` },
        data: { key: projectKey, name: `QA ${account.runId}`, description: 'Local platform smoke' },
      })
      expect(created.ok()).toBeTruthy()
    }
  })

  test.afterAll(async ({ request }) => {
    if (!taskToken) return
    const project = await request.get(`http://127.0.0.1:7721/api/v1/projects/${projectKey}`, {
      headers: { Authorization: `Bearer ${taskToken}` },
    })
    if (!project.ok()) return
    const data = await project.json()
    if (!String(data.name).startsWith(`QA ${account.runId}`)) return
    const removed = await request.delete(`http://127.0.0.1:7721/api/v1/projects/${projectKey}`, {
      headers: { Authorization: `Bearer ${taskToken}` },
    })
    expect(removed.ok()).toBeTruthy()
  })

  for (const [index, app] of apps.entries()) {
    test(`${app.label}: authenticated catalog and navigation`, async ({ page }) => {
      if (process.env.SDLC_LIVE_VISUAL === '1') test.setTimeout(120_000)
      const errors: string[] = []
      page.on('pageerror', (error) => errors.push(error.message))
      await page.goto(`http://localhost:${app.port}/`)

      if (app.login) {
        await expect(page).toHaveURL(/\/login$/)
        const fields = page.locator('input')
        await fields.nth(0).fill(app.key === 'ci-cd' ? account.username : account.email)
        await fields.nth(1).fill(account.password)
        await page.getByRole('button', { name: /^(Войти|Sign in)$/ }).click()
        await expect(page).not.toHaveURL(/\/login$/, { timeout: 15_000 })
        if (app.key === 'admin-panel') {
          adminToken = await page.evaluate(() => sessionStorage.getItem('base.admin.token') ?? '')
        }
      }

      if (process.env.SDLC_LIVE_VISUAL === '1') {
        mkdirSync(screenshotDir, { recursive: true })
        for (const theme of ['dark', 'gray', 'light']) {
          if (app.key === 'project-workflow') {
            await page.getByRole('combobox', { name: 'Тема' }).selectOption(theme)
          } else {
            for (let step = 0; step < 3; step++) {
              if ((await page.locator('html').getAttribute('data-theme')) === theme) break
              await page
                .getByRole('button', { name: /Тема:|Theme:|Переключить тему|Switch theme/i })
                .click()
            }
            await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
          }
          for (const [width, height] of [
            [375, 812],
            [768, 1024],
            [1280, 800],
            [1920, 1080],
          ]) {
            await page.setViewportSize({ width, height })
            await expect
              .poll(() =>
                page.evaluate(
                  () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
                ),
              )
              .toBeLessThanOrEqual(1)
            await page.screenshot({
              path: `${screenshotDir}/${app.key}-${theme}-${width}.png`,
              fullPage: true,
              animations: 'disabled',
            })
            if ((theme === 'dark' && width === 375) || (theme === 'light' && width === 1280)) {
              const results = await new AxeBuilder({ page }).analyze()
              expect(
                results.violations.filter(
                  (issue) => issue.impact === 'serious' || issue.impact === 'critical',
                ),
              ).toEqual([])
            }
          }
        }
      }

      const trigger =
        app.key === 'project-workflow'
          ? page.locator('summary[aria-label="Открыть список сервисов платформы"]')
          : page.getByRole('button', { name: /Открыть список сервисов/ })
      await trigger.click()
      const menu = page.getByRole('menu', { name: /сервисов|Сервисы платформы/i })
      await expect(menu).toBeVisible()
      const text = await menu.innerText()
      for (const service of apps) expect(text).toContain(service.label)
      expect(text).not.toMatch(/Central Auth|Java Agent/)

      const surface = await menu.evaluate((element) => {
        const style = getComputedStyle(element)
        return { background: style.backgroundColor, opacity: style.opacity }
      })
      expect(surface.opacity).toBe('1')
      expect(surface.background).not.toBe('rgba(0, 0, 0, 0)')
      expect(surface.background).not.toBe('transparent')

      const next = apps[(index + 1) % apps.length]
      await menu.getByRole('menuitem', { name: new RegExp(next.label.replace('/', '\\/')) }).click()
      await expect(page).toHaveURL(new RegExp(`^http://localhost:${next.port}/`))
      expect(page.url()).not.toMatch(/access_token|refresh_token|[?&]token=/)
      expect(errors).toEqual([])
    })
  }

  test('Task Tracker board stays within four viewports', async ({ page }) => {
    test.setTimeout(120_000)
    await page.goto('http://localhost:7722/login')
    await page.locator('input').nth(0).fill(account.email)
    await page.locator('input').nth(1).fill(account.password)
    await page.getByRole('button', { name: 'Войти' }).click()
    await expect(page).not.toHaveURL(/\/login$/, { timeout: 15_000 })
    await page.getByRole('button', { name: /проекты/i }).click()
    await page.getByRole('menuitem', { name: /все проекты/i }).click()
    await page.getByRole('link', { name: `QA ${account.runId}` }).click()
    await expect(page.getByRole('heading', { name: /Доска/ })).toBeVisible()

    for (const theme of ['dark', 'gray', 'light']) {
      for (let step = 0; step < 3; step++) {
        if ((await page.locator('html').getAttribute('data-theme')) === theme) break
        await page.getByRole('button', { name: /Тема:/ }).click()
      }
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme)
      for (const [width, height] of [
        [375, 812],
        [768, 1024],
        [1280, 800],
        [1920, 1080],
      ]) {
        await page.setViewportSize({ width, height })
        const overflow = await page.evaluate(
          () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
        )
        expect(overflow).toBeLessThanOrEqual(1)
        if (process.env.SDLC_LIVE_VISUAL === '1') {
          mkdirSync(screenshotDir, { recursive: true })
          await page.screenshot({
            path: `${screenshotDir}/task-board-${theme}-${width}.png`,
            fullPage: true,
            animations: 'disabled',
          })
          if ((theme === 'dark' && width === 375) || (theme === 'light' && width === 1280)) {
            const results = await new AxeBuilder({ page }).analyze()
            expect(
              results.violations.filter(
                (issue) => issue.impact === 'serious' || issue.impact === 'critical',
              ),
            ).toEqual([])
          }
        }
      }
    }
  })

  test('Admin Panel internal pages expose complete runtime catalog', async ({ page, request }) => {
    test.setTimeout(90_000)
    if (adminToken) {
      await page.addInitScript(
        (token) => sessionStorage.setItem('base.admin.token', token),
        adminToken,
      )
    } else {
      await page.goto('http://localhost:7772/login')
      await page.locator('input').nth(0).fill(account.email)
      await page.locator('input').nth(1).fill(account.password)
      await page.getByRole('button', { name: /Войти/ }).click()
      await expect(page).not.toHaveURL(/\/login$/, { timeout: 15_000 })
      adminToken = await page.evaluate(() => sessionStorage.getItem('base.admin.token') ?? '')
      await page.addInitScript(
        (token) => sessionStorage.setItem('base.admin.token', token),
        adminToken,
      )
    }
    const pages = [
      '/services',
      '/services/admin-panel',
      '/revisions',
      '/branding',
      '/role-bindings',
      '/audit',
      '/runtime',
    ]
    for (const path of pages) {
      await page.goto(`http://localhost:7772${path}`)
      await expect(page.locator('h1').first()).toBeVisible()
      for (const [width, height] of [
        [375, 812],
        [1280, 800],
      ]) {
        await page.setViewportSize({ width, height })
        if (process.env.SDLC_LIVE_VISUAL === '1') {
          await page.screenshot({
            path: `${screenshotDir}/admin-${path.slice(1).replace('/', '-')}-${width}.png`,
            fullPage: true,
            animations: 'disabled',
          })
        }
        await expect
          .poll(
            () =>
              page.evaluate(
                () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
              ),
            { message: `${path} at ${width}px overflows horizontally` },
          )
          .toBeLessThanOrEqual(1)
      }
    }
    const catalog = await request.get('http://127.0.0.1:7771/api/v1/runtime/services')
    expect(catalog.ok()).toBeTruthy()
    const payload = (await catalog.json()) as {
      services: { key: string; ui_url: string | null; health: string }[]
    }
    expect(payload.services).toHaveLength(8)
    expect(payload.services.filter((service) => service.ui_url)).toHaveLength(6)
    expect(payload.services.every((service) => service.health === 'healthy')).toBe(true)
    await expect(page.getByText('Java Agent').first()).toBeVisible()
  })

  test('Task Tracker project, issue, status, sprint and report', async ({ page, request }) => {
    test.setTimeout(90_000)
    page.setDefaultTimeout(10_000)
    await page.goto('http://localhost:7722/login')
    await page.locator('input').nth(0).fill(account.email)
    await page.locator('input').nth(1).fill(account.password)
    await page.getByRole('button', { name: 'Войти' }).click()
    await expect(page).not.toHaveURL(/\/login$/, { timeout: 15_000 })
    await page.getByRole('button', { name: /проекты/i }).click()
    await page.getByRole('menuitem', { name: /все проекты/i }).click()

    const projectName = `QA ${account.runId} updated`
    const projectLink = page.getByRole('link', { name: `QA ${account.runId}` })
    await projectLink
      .locator('xpath=../../../..')
      .getByRole('button', { name: 'Ещё действия' })
      .click()
    await page.getByRole('menuitem', { name: 'Изменить' }).click()
    await page.locator('#project-form-name').fill(projectName)
    await page.getByRole('dialog').getByRole('button', { name: 'Сохранить' }).click()
    await expect(page.getByRole('link', { name: projectName })).toBeVisible()

    await page.getByRole('link', { name: projectName }).click()
    await page.locator('a[href="/issues/create"]').first().click()
    await page.locator('#issue-project').selectOption(projectKey)
    const summary = `QA ${account.runId} issue`
    await page.locator('#issue-summary').fill(summary)
    await page.locator('#issue-description').fill('Live QA scenario')
    await page.locator('form').getByRole('button', { name: 'Создать' }).click()
    await expect(page.getByText(summary)).toBeVisible()
    await page.getByRole('link', { name: /доска/i }).first().click()
    const card = page.locator('article').filter({ hasText: summary })
    await expect(card).toBeVisible()
    const detailPath = await card.locator('a[href^="/issues/"]').first().getAttribute('href')
    expect(detailPath).toMatch(/^\/issues\//)
    await card.getByRole('button', { name: /Изменить статус/ }).click()
    await page.getByRole('menuitem', { name: /В работе/ }).click()
    await expect(card).toBeVisible()
    if (process.env.SDLC_LIVE_VISUAL === '1') {
      await page.setViewportSize({ width: 375, height: 812 })
      await page.screenshot({
        path: `${screenshotDir}/task-board-filled-375.png`,
        fullPage: true,
        animations: 'disabled',
      })
    }

    const sprintName = `QA ${account.runId} sprint`
    const sprint = await request.post(
      `http://127.0.0.1:7721/api/v1/projects/${projectKey}/sprints`,
      {
        headers: { Authorization: `Bearer ${taskToken}` },
        data: { name: sprintName, goal: 'Live report validation' },
      },
    )
    expect(sprint.ok()).toBeTruthy()
    await page.goto('http://localhost:7722/reports')
    await expect(page.getByRole('heading', { name: /Отчёт/ }).first()).toBeVisible()
    await page.locator('#report-project').selectOption({ label: projectName })
    await page.locator('#report-sprint').selectOption({ label: sprintName })
    await expect(page).toHaveURL(/sprint_id=/)

    for (const path of [
      detailPath!,
      `/projects/${projectKey}/backlog`,
      `/projects/${projectKey}/settings/custom-fields`,
      `/projects/${projectKey}/trash`,
      '/search',
      '/notifications',
      '/reports',
    ]) {
      await page.goto(`http://localhost:7722${path}`)
      await expect(page.locator('h1').first()).toBeVisible()
      await page.setViewportSize({ width: 375, height: 812 })
      await expect
        .poll(
          () =>
            page.evaluate(
              () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
            ),
          { message: `${path} at 375px overflows` },
        )
        .toBeLessThanOrEqual(1)
      if (process.env.SDLC_LIVE_VISUAL === '1') {
        await page.screenshot({
          path: `${screenshotDir}/task-${path.replaceAll('/', '-').slice(1)}-375.png`,
          fullPage: true,
          animations: 'disabled',
        })
      }
    }
  })

  test('Wiki publishes and revises a QA document', async ({ page, request }) => {
    test.setTimeout(180_000)
    page.setDefaultTimeout(10_000)
    const api = 'http://127.0.0.1:7731/api/v1'
    await page.goto('http://localhost:7732/login')
    await page.locator('input').nth(0).fill(account.email)
    await page.locator('input').nth(1).fill(account.password)
    let loginResponse
    for (let attempt = 0; attempt < 3; attempt++) {
      const response = page.waitForResponse(
        (candidate) =>
          candidate.url().includes('/api/v1/auth/login') && candidate.request().method() === 'POST',
      )
      await page.getByRole('button', { name: /Войти/ }).click()
      loginResponse = await response
      if (loginResponse.status() !== 429) break
      await page.waitForTimeout(16_000)
    }
    expect(loginResponse?.ok(), `Wiki login returned HTTP ${loginResponse?.status()}`).toBeTruthy()
    const { access_token: token } = await loginResponse!.json()
    await expect(page).not.toHaveURL(/\/login$/, { timeout: 15_000 })
    const headers = { Authorization: `Bearer ${token}` }
    const wikiFailures: string[] = []
    page.on('response', (response) => {
      if (response.url().includes('/api/v1/') && response.status() >= 400) {
        wikiFailures.push(`${response.status()} ${new URL(response.url()).pathname}`)
      }
    })
    const spaceKey = `QA${account.runId.slice(0, 4).toUpperCase()}${Date.now().toString(36).slice(-5).toUpperCase()}`
    let documentId = ''
    let spaceCreated = false
    async function archive(path: string) {
      for (let attempt = 0; attempt < 4; attempt++) {
        const response = await request.post(`${api}/${path}`, { headers })
        if (response.status() !== 429) {
          expect(response.ok(), `${response.status()} ${await response.text()}`).toBeTruthy()
          return
        }
        await page.waitForTimeout(16_000)
      }
      throw new Error(`Wiki QA cleanup remained rate limited: ${path}`)
    }
    try {
      const space = await request.post(`${api}/spaces`, {
        headers,
        data: { key: spaceKey, name: `QA ${account.runId} wiki`, description: 'Local QA' },
      })
      expect(space.ok(), `${space.status()} ${await space.text()}`).toBeTruthy()
      spaceCreated = true
      const created = await request.post(`${api}/spaces/${spaceKey}/documents`, {
        headers,
        data: {
          title: `QA ${account.runId} document`,
          content_markdown: '# First published revision',
        },
      })
      expect(created.ok(), `${created.status()} ${await created.text()}`).toBeTruthy()
      const doc = await created.json()
      documentId = doc.id
      const published = await request.post(`${api}/documents/${documentId}/publish`, {
        headers,
        data: { summary: 'QA first publication' },
      })
      expect(published.ok()).toBeTruthy()
      const firstRevision = await published.json()
      const updated = await request.put(`${api}/documents/${documentId}/draft`, {
        headers,
        data: { title: `QA ${account.runId} revised`, content_markdown: '# Revised QA content' },
      })
      expect(updated.ok()).toBeTruthy()
      const republished = await request.post(`${api}/documents/${documentId}/publish`, {
        headers,
        data: { base_revision_id: firstRevision.id, summary: 'QA second publication' },
      })
      expect(republished.ok()).toBeTruthy()
      const listed = await request.get(`${api}/spaces`, { headers })
      expect(listed.ok()).toBeTruthy()
      expect(
        (await listed.json()).spaces.some((space: { key: string }) => space.key === spaceKey),
      ).toBe(true)

      await page
        .getByRole('link', { name: /пространства/i })
        .first()
        .click()
      await expect(page).toHaveURL(/\/spaces$/, { timeout: 15_000 })
      await expect(
        page.getByText(`${spaceKey} · QA ${account.runId} wiki`),
        wikiFailures.join(', '),
      ).toBeVisible({ timeout: 20_000 })
      if (process.env.SDLC_LIVE_VISUAL === '1') {
        mkdirSync(screenshotDir, { recursive: true })
        await page.screenshot({
          path: `${screenshotDir}/wiki-qa-spaces.png`,
          fullPage: true,
          animations: 'disabled',
        })
      }
      await page.getByRole('link', { name: `QA ${account.runId} revised` }).click()
      await expect(page.getByRole('heading', { name: 'Revised QA content' })).toBeVisible()
    } finally {
      if (documentId) {
        await archive(`documents/${documentId}/archive`)
      }
      if (spaceCreated) {
        await archive(`spaces/${spaceKey}/archive`)
      }
    }
  })

  test('Fleet Control manages a QA agent without starting it', async ({ page, request }) => {
    page.setDefaultTimeout(10_000)
    const api = 'http://127.0.0.1:7741/api/v1'
    const login = await request.post(`${api}/auth/login`, {
      data: { email: account.email, password: account.password },
    })
    expect(login.ok()).toBeTruthy()
    const { access_token: token } = await login.json()
    const headers = { Authorization: `Bearer ${token}` }
    const name = `QA ${account.runId} agent`
    let agentId = ''
    try {
      const created = await request.post(`${api}/agents`, {
        headers,
        data: {
          kind: 'hermes',
          product_role: 'executor',
          role: 'developer',
          display_name: name,
          description: 'Local QA, not started',
        },
      })
      expect(created.ok(), `${created.status()} ${await created.text()}`).toBeTruthy()
      agentId = (await created.json()).id
      const updated = await request.patch(`${api}/agents/${agentId}`, {
        headers,
        data: { display_name: `${name} updated`, description: 'QA management verified' },
      })
      expect(updated.ok(), `${updated.status()} ${await updated.text()}`).toBeTruthy()

      await page.goto('http://localhost:7742/login')
      await page.locator('input').nth(0).fill(account.email)
      await page.locator('input').nth(1).fill(account.password)
      await page.getByRole('button', { name: /Sign in|Войти/ }).click()
      await expect(page).not.toHaveURL(/\/login$/, { timeout: 15_000 })
      await page
        .getByRole('link', { name: /Agents/i })
        .first()
        .click()
      await expect(page.getByText(`${name} updated`).first()).toBeVisible()
    } finally {
      if (agentId) {
        const archived = await request.delete(`${api}/agents/${agentId}`, { headers })
        expect(archived.ok(), `${archived.status()} ${await archived.text()}`).toBeTruthy()
      }
    }
  })

  test('Project Workflow creates and displays a QA flow', async ({ page, request }) => {
    page.setDefaultTimeout(10_000)
    const api = 'http://127.0.0.1:8812/api'
    let workflowId = 0
    try {
      const created = await request.post(`${api}/workflows`, {
        data: { name: `QA ${account.runId} workflow`, description: 'Local QA flow' },
      })
      expect(created.ok(), `${created.status()} ${await created.text()}`).toBeTruthy()
      const result = await created.json()
      expect(result.ok).toBe(true)
      workflowId = result.workflow_id
      const phase = await request.post(`${api}/phases`, {
        data: { workflow_id: workflowId, phase_order: 1, name: 'QA review phase' },
      })
      expect(phase.ok(), `${phase.status()} ${await phase.text()}`).toBeTruthy()
      expect((await phase.json()).ok).toBe(true)

      await page.goto('http://localhost:8812/workflows')
      await expect(page.getByRole('button', { name: `QA ${account.runId} workflow` })).toBeVisible()
      await page.getByRole('button', { name: `QA ${account.runId} workflow` }).click()
      await expect(page.locator('#workflowName')).toHaveValue(`QA ${account.runId} workflow`)
      await page.getByRole('combobox', { name: 'Тема' }).selectOption('light')
      await expect(page.locator('html')).toHaveAttribute('data-theme', 'light')
      await page.reload()
      await expect(page.getByRole('combobox', { name: 'Тема' })).toHaveValue('light')
      await page.getByRole('combobox', { name: 'Тема' }).selectOption('dark')
    } finally {
      if (workflowId) {
        const removed = await request.delete(`${api}/workflows/${workflowId}`)
        expect(removed.ok(), `${removed.status()} ${await removed.text()}`).toBeTruthy()
      }
    }
  })

  test('Admin Panel records QA role changes in audit', async ({ page, request }) => {
    page.setDefaultTimeout(10_000)
    const claimValue = `qa-audit-${account.runId}@example.test`
    if (adminToken) {
      await page.addInitScript(
        (token) => sessionStorage.setItem('base.admin.token', token),
        adminToken,
      )
      await page.goto('http://localhost:7772/')
    } else {
      await page.goto('http://localhost:7772/login')
      await page.locator('input').nth(0).fill(account.email)
      await page.locator('input').nth(1).fill(account.password)
      await page.getByRole('button', { name: /Войти/ }).click()
    }
    await expect(page).not.toHaveURL(/\/login$/, { timeout: 15_000 })

    try {
      await page.getByRole('link', { name: 'Привязки ролей' }).click()
      const form = page.locator('form').first()
      await form.locator('select').nth(0).selectOption('email')
      await form.locator('input').fill(claimValue)
      await form.locator('select').nth(1).selectOption('platform_viewer')
      await form.getByRole('button', { name: 'Создать' }).click()
      const row = page.getByRole('row').filter({ hasText: claimValue })
      await expect(row).toBeVisible()
      await row.getByRole('button', { name: 'Удалить' }).click()
      await expect(row).toBeHidden()
      await page.getByRole('link', { name: 'Аудит' }).click()
      await expect(page.getByText('role_binding.created').first()).toBeVisible()
      await expect(page.getByText('role_binding.deleted').first()).toBeVisible()
    } finally {
      const token = await page.evaluate(() => sessionStorage.getItem('base.admin.token'))
      if (token) {
        const headers = { Authorization: `Bearer ${token}` }
        const bindings = await request.get('http://127.0.0.1:7771/api/v1/role-bindings', {
          headers,
        })
        if (bindings.ok()) {
          for (const binding of (await bindings.json()).bindings as {
            id: string
            claim_value: string
          }[]) {
            if (binding.claim_value === claimValue) {
              const removed = await request.delete(
                `http://127.0.0.1:7771/api/v1/role-bindings/${binding.id}`,
                { headers },
              )
              expect(removed.ok()).toBeTruthy()
            }
          }
        }
      }
    }
  })

  test('isolated CI/CD QA job is visible with its real log', async ({ page }) => {
    test.skip(process.env.SDLC_CICD_QA !== '1', 'Requires the isolated CI/CD QA Compose project')
    page.setDefaultTimeout(10_000)
    const state = JSON.parse(
      readFileSync(
        fileURLToPath(new URL('../../../.local/cicd-qa-state.json', import.meta.url)),
        'utf8',
      ),
    ) as { projectId: string }
    await page.goto('http://localhost:17712/login')
    await page.locator('input').nth(0).fill(account.username)
    await page.locator('input').nth(1).fill(account.password)
    await page.getByRole('button', { name: /Войти|Sign in/ }).click()
    await expect(page).not.toHaveURL(/\/login$/, { timeout: 15_000 })
    await page
      .getByRole('link', { name: /Проекты/ })
      .first()
      .click()
    await page.getByRole('link', { name: `QA ${account.runId} CI` }).click()
    await expect(page).toHaveURL(new RegExp(`/projects/${state.projectId}/pipelines`))
    await page.getByRole('link', { name: /main/ }).first().click()
    await expect(page.getByText('qa_smoke').first()).toBeVisible()
    await page.getByRole('button', { name: /Логи/ }).first().click()
    await expect(page.getByText('SDLC_QA_JOB_OK')).toBeVisible()
  })
})
