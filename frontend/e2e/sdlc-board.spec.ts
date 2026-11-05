import { expect, test, type Page } from '@playwright/test'
import { generateKeyPairSync, sign } from 'node:crypto'
const ids = { user: '00000000-0000-4000-8000-000000000001' }
const accessToken = `fixture.${Buffer.from(JSON.stringify({ sub: ids.user, exp: Math.floor(Date.now() / 1000) + 3600 })).toString('base64url')}.fixture`
async function installSsoMocks(page: Page) {
  const { privateKey, publicKey } = generateKeyPairSync('ec', { namedCurve: 'P-256' })
  const jwk = { ...publicKey.export({ format: 'jwk' }), kid: 'qa', alg: 'ES256', use: 'sig' }
  let issuer = 'http://localhost:7701'
  let nonce = ''
  await page.route('**/auth/access', (route) => {
    const authorized =
      route.request().method() === 'GET' &&
      route.request().headers().authorization === `Bearer ${accessToken}`
    const observed = Date.now()
    return route.fulfill({
      status: authorized ? 200 : 401,
      contentType: 'application/json',
      headers: { 'access-control-allow-origin': '*', 'cache-control': 'no-store' },
      body: JSON.stringify({
        schema_version: 1,
        issuer,
        subject: ids.user,
        role: 'admin',
        applications: ['task-tracker'],
        revision: 1,
        enforced: true,
        observed_at: new Date(observed).toISOString(),
        expires_at: new Date(observed + 60000).toISOString(),
      }),
    })
  })
  await page.route('**/oidc/authorize**', async (route) => {
    const url = new URL(route.request().url())
    issuer = url.origin
    nonce = url.searchParams.get('nonce') ?? ''
    const callback = new URL(url.searchParams.get('redirect_uri') ?? '/')
    callback.searchParams.set('code', 'qa-code')
    callback.searchParams.set('state', url.searchParams.get('state') ?? '')
    // WebKit cannot fulfill mocked redirects; Chromium needs a real redirect
    // so the synthetic Auth document does not acquire a public address space.
    if (page.context().browser()?.browserType().name() !== 'webkit') {
      await route.fulfill({ status: 302, headers: { location: callback.toString() } })
      return
    }
    await route.fulfill({
      status: 200,
      contentType: 'text/html',
      body: `<!doctype html><script>location.replace(${JSON.stringify(callback.toString())})</script>`,
    })
  })
  await page.route('**/oidc/token', async (route) => {
    const header = Buffer.from(JSON.stringify({ alg: 'ES256', typ: 'JWT', kid: 'qa' })).toString(
      'base64url',
    )
    const payload = Buffer.from(
      JSON.stringify({
        iss: issuer,
        aud: 'task-tracker',
        sub: ids.user,
        email: 'admin@task-tracker.local',
        nonce,
        iat: Math.floor(Date.now() / 1000),
        exp: Math.floor(Date.now() / 1000) + 3600,
      }),
    ).toString('base64url')
    const content = `${header}.${payload}`
    const signature = sign('sha256', Buffer.from(content), {
      key: privateKey,
      dsaEncoding: 'ieee-p1363',
    }).toString('base64url')
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      headers: { 'access-control-allow-origin': '*' },
      body: JSON.stringify({
        access_token: accessToken,
        id_token: `${content}.${signature}`,
        expires_in: 3600,
      }),
    })
  })
  await page.route('**/oidc/jwks', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      headers: { 'access-control-allow-origin': '*' },
      body: JSON.stringify({ keys: [jwk] }),
    }),
  )
}

const stages = [
  'Draft',
  'Backlog',
  'Analysis',
  'Architecture',
  'Development',
  'Review',
  'Testing',
  'Rework',
  'Deployment',
]
for (const width of [375, 1920, 2560]) {
  test(`SDLC board stages and backlog at ${width}`, async ({ page }, info) => {
    await page.setViewportSize({ width, height: width === 375 ? 812 : 1080 })
    await installSsoMocks(page)
    const writes: string[] = []
    const errors: string[] = []
    page.on('pageerror', (error) => errors.push(error.message))
    const issue = {
      id: '00000000-0000-4000-8000-000000000101',
      key: 'BOARD-1',
      summary: 'Проверить страницу состояния сервиса',
      description: `${'Полное описание задачи. '.repeat(24)}КОНЕЦ ОПИСАНИЯ`,
      time_spent_seconds: 0,
      issue_type: 'task',
      project_key: 'BOARD',
      project_name: 'Board fixture',
      status: 'SDLC Ready',
      status_id: '00000000-0000-4000-8000-000000000102',
      priority: 'Medium',
      labels: [],
      assignee_id: null,
      assignee_name: null,
      reporter_id: ids.user,
      reporter_name: 'Fixture owner',
      sprint_id: null,
      created_at: '2026-11-01T12:00:00Z',
      updated_at: '2026-11-01T12:00:00Z',
    }
    const sprint = {
      id: 'none',
      name: 'Backlog',
      goal: '',
      state: 'future',
      velocity: 0,
      remaining_days: null,
      issue_ids: [],
      start_date: null,
      end_date: null,
    }
    const project = {
      id: '00000000-0000-4000-8000-000000000103',
      key: 'BOARD',
      name: 'Board fixture',
      owner_id: ids.user,
      todo_count: 1,
      in_progress_count: 0,
      done_count: 0,
    }
    await page.route('**/api/v1/**', async (route) => {
      const request = route.request()
      const path = new URL(request.url()).pathname
      const reply = (data: unknown) =>
        route.fulfill({
          status: 200,
          contentType: 'application/json',
          headers: { 'access-control-allow-origin': '*' },
          body: JSON.stringify(data),
        })
      if (request.method() === 'OPTIONS')
        return route.fulfill({
          status: 204,
          headers: {
            'access-control-allow-origin': '*',
            'access-control-allow-headers': 'Authorization,Content-Type',
            'access-control-allow-methods': 'GET,OPTIONS',
          },
        })
      if (request.method() !== 'GET') {
        writes.push(path)
        return route.fulfill({ status: 403, body: 'Fixture writes forbidden' })
      }
      if (path === '/api/v1/users/me')
        return reply({
          id: ids.user,
          email: 'owner@example.test',
          username: 'owner',
          display_name: 'Fixture owner',
          is_system_admin: true,
        })
      if (path === `/api/v1/issues/${issue.id}`) return reply(issue)
      if (path === '/api/v1/users') return reply({ users: [] })
      if (path === '/api/v1/statuses') return reply([{ id: issue.status_id, name: issue.status }])
      if (path.endsWith('/labels')) return reply({ labels: [] })
      if (path.endsWith('/links')) return reply({ links: [] })
      if (path.endsWith('/worklogs')) return reply({ worklogs: [] })
      if (path.endsWith('/comments')) return reply({ comments: [] })
      if (path.endsWith('/attachments')) return reply({ attachments: [] })
      if (path.endsWith('/votes')) return reply({ count: 0, votes: [] })
      if (path.endsWith('/watchers')) return reply({ watchers: [] })
      if (path.endsWith('/custom-fields'))
        return reply(path.startsWith('/api/v1/projects/') ? { fields: [] } : { values: [] })
      if (path === '/api/v1/projects') return reply([project])
      if (path === '/api/v1/projects/BOARD') return reply(project)
      if (path === '/api/v1/projects/BOARD/board')
        return reply({
          project_id: project.id,
          project_key: 'BOARD',
          columns: stages.map((stage) => ({
            id: `stage:${stage}`,
            name: stage,
            stage,
            wip_limit: null,
            issue_ids: stage === 'Backlog' ? [issue.id] : [],
          })),
          task_states: [{ task_id: issue.id, stage: 'Backlog', status: 'Ready' }],
          issues: [issue],
          sprint,
          backlog_total: 1,
        })
      if (path === '/api/v1/projects/BOARD/backlog')
        return reply({
          project_id: project.id,
          project_key: 'BOARD',
          backlog_total: 1,
          backlog_offset: 0,
          backlog_limit: 100,
          sprint,
          sprint_issues: [],
          backlog_issues: [issue],
        })
      if (path.includes('unread')) return reply({ count: 0 })
      if (path.includes('notifications')) return reply({ notifications: [], unread_count: 0 })
      return reply([])
    })
    await page.goto('/projects/BOARD/board')
    await expect(page.getByRole('heading', { name: 'Стадии · BOARD' })).toBeVisible({
      timeout: 20000,
    })
    await expect(page.getByRole('article')).toContainText('BOARD-1')
    await expect(page.getByRole('article')).toContainText('Готова к работе')
    await expect(page.getByRole('article')).toHaveAttribute('draggable', 'false')
    await expect(
      page.getByRole('main').getByRole('button', { name: 'Создать', exact: true }),
    ).toBeVisible()
    if (width === 375) {
      await expect(page.getByRole('radio', { name: 'Бэклог (1)', exact: true })).toBeChecked()
      for (const label of [
        'Драфт',
        'Бэклог',
        'Анализ',
        'Архитектура',
        'Разработка',
        'Ревью',
        'Тестирование',
        'Доработка',
        'Деплой',
      ])
        await expect(
          page.getByRole('radio', {
            name: `${label} (${label === 'Бэклог' ? 1 : 0})`,
            exact: true,
          }),
        ).toBeVisible()
      await page.getByRole('radio', { name: 'Анализ (0)', exact: true }).click()
      await expect(page).toHaveURL(/status=stage%3AAnalysis/)
      await page.getByRole('radio', { name: 'Бэклог (1)', exact: true }).click()
    } else {
      const region = page.getByRole('region', { name: 'Колонки доски' })
      for (const label of [
        'Драфт',
        'Бэклог',
        'Анализ',
        'Архитектура',
        'Разработка',
        'Ревью',
        'Тестирование',
        'Доработка',
        'Деплой',
      ])
        await expect(region.getByText(label, { exact: true })).toBeVisible()
    }
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
    await page.getByRole('link', { name: 'Бэклог', exact: true }).last().click()
    await expect(page.getByRole('heading', { name: 'Бэклог · BOARD' })).toBeVisible()
    await expect(page.getByText('BOARD-1', { exact: true })).toBeVisible()
    await expect(page.getByText('Бэклог · 1', { exact: true })).toBeVisible()
    await page.screenshot({ path: info.outputPath(`backlog-${width}.png`), fullPage: true })
    await page.goBack()
    await expect(page.getByRole('heading', { name: 'Стадии · BOARD' })).toBeVisible()
    await page.screenshot({ path: info.outputPath(`board-${width}.png`), fullPage: true })
    await page.goto(`/issues/${issue.id}?tab=worklog`)
    await expect(page.getByRole('heading', { name: issue.summary })).toBeVisible()
    await expect(page.getByText(/КОНЕЦ ОПИСАНИЯ/)).toBeVisible()
    await expect(page.getByRole('button', { name: 'Показать описание полностью' })).toHaveCount(0)
    await expect(page.getByRole('tab', { name: 'Журнал работ' })).toHaveAttribute(
      'aria-selected',
      'true',
    )
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
    for (const message of [
      'Пока нет записей.',
      'Меток нет',
      'Нет настроенных полей.',
      'Связей нет',
    ])
      await expect(page.getByText(message, { exact: true })).toBeVisible()
    await expect(page.getByText(/Не удалось загрузить/)).toHaveCount(0)
    await page.screenshot({ path: info.outputPath(`issue-${width}.png`), fullPage: true })
    expect(writes).toEqual([])
    expect(errors).toEqual([])
  })
}
