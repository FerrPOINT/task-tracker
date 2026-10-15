import { generateKeyPairSync, sign } from 'node:crypto'
import { mkdirSync } from 'node:fs'
import nodePath from 'node:path'
import { createServer } from 'node:http'
import { test, expect, type Page, type Route } from '@playwright/test'

const mockUser = {
  id: '00000000-0000-0000-0000-000000000001',
  key: 'DEMO',
  name: 'Demo Project',
  issueId: 'issue-1',
}
const namespaceEnabled = process.env.VITE_NAMESPACE_ENABLED === 'true'

const markdownComment =
  '# Итоги\n\n**Результат**\n\nПервая строка\nВторая строка\n\n- Первый\n- Второй\n\n' +
  'ДлинныйИдентификатор'.repeat(24) +
  '\n\n```text\n  ' +
  'code_block_'.repeat(40) +
  '\n```\n\n' +
  '<script>alert(1)</script>\n\n[Опасно](javascript:alert%281%29)'

function routeJson(route: Route, body: unknown, status = 200) {
  return route.fulfill({
    status,
    contentType: 'application/json',
    body: JSON.stringify(body),
  })
}

async function installSsoMocks(page: Page) {
  const { privateKey, publicKey } = generateKeyPairSync('ec', { namedCurve: 'P-256' })
  const jwk = { ...publicKey.export({ format: 'jwk' }), kid: 'qa', alg: 'ES256', use: 'sig' }
  let issuer = 'http://localhost:7701'
  let nonce = ''

  const authorizationServer = createServer((request, response) => {
    const url = new URL(request.url!, issuer)
    const callback = new URL(url.searchParams.get('redirect_uri')!)
    callback.searchParams.set('code', 'qa-code')
    callback.searchParams.set('state', url.searchParams.get('state') ?? '')
    response.writeHead(302, { location: callback.href })
    response.end()
  })
  await new Promise<void>((resolve, reject) => {
    authorizationServer.once('error', reject)
    authorizationServer.listen(0, '127.0.0.1', resolve)
  })
  const address = authorizationServer.address()
  if (!address || typeof address === 'string') throw new Error('Missing SSO fixture address')
  page.once('close', () => authorizationServer.close())

  await page.route('**/oidc/authorize**', async (route) => {
    const url = new URL(route.request().url())
    issuer = url.origin
    nonce = url.searchParams.get('nonce') ?? ''
    await route.continue({ url: `http://127.0.0.1:${address.port}/authorize${url.search}` })
  })
  await page.route('**/oidc/token', async (route) => {
    const header = Buffer.from(JSON.stringify({ alg: 'ES256', typ: 'JWT', kid: 'qa' })).toString(
      'base64url',
    )
    const payload = Buffer.from(
      JSON.stringify({
        iss: issuer,
        aud: 'task-tracker',
        sub: mockUser.id,
        email: 'demo@example.com',
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
    return routeJson(route, {
      access_token: 'demo-token',
      id_token: `${content}.${signature}`,
      expires_in: 3600,
    })
  })
  await page.route('**/oidc/jwks', (route) => routeJson(route, { keys: [jwk] }))
}

test.describe('smoke', () => {
  for (const outcome of ['validation400', 'validation422', 'unknown'] as const) {
    test(`issue creation preserves safe retry semantics after ${outcome}`, async ({
      page,
      baseURL,
    }, testInfo) => {
      await installSsoMocks(page)
      const appBaseURL = (baseURL ?? 'http://localhost:4173').replace('127.0.0.1', 'localhost')
      const registry = '11111111-1111-4111-8111-111111111111'
      const namespace = '22222222-2222-4222-8222-222222222222'
      const project = {
        id: '00000000-0000-0000-0000-000000000010',
        key: mockUser.key,
        name: mockUser.name,
        description: '',
        owner_id: mockUser.id,
        owner_name: 'Demo User',
        created_at: '2026-10-14T00:00:00Z',
        todo_count: 0,
        in_progress_count: 0,
        done_count: 0,
      }
      const context = {
        label: project.name,
        resource_key: project.key,
        binding: {
          schema_version: 1,
          operation_id: '33333333-3333-4333-8333-333333333333',
          generation: 1,
          state: 'active',
          drained: true,
          namespace: { registry_instance_id: registry, namespace_id: namespace },
          resource: { instance_id: registry, resource_id: project.id, kind: 'tracker_project' },
        },
      }
      const writes: { body: Record<string, unknown>; key: string | undefined }[] = []
      await page.route('**/api/v1/**', async (route) => {
        const request = route.request()
        const path = new URL(request.url()).pathname.replace('/api/v1', '')
        if (path === '/issues' && request.method() === 'POST') {
          writes.push({ body: request.postDataJSON(), key: request.headers()['idempotency-key'] })
          if (writes.length === 1 && outcome === 'unknown') return route.abort('failed')
          if (writes.length === 1 || (outcome === 'unknown' && writes.length === 2)) {
            return routeJson(
              route,
              { error: { code: 'VALIDATION_ERROR', message: 'private API details' } },
              outcome === 'validation400' ? 400 : 422,
            )
          }
          return routeJson(route, { id: 'created', project_key: project.key }, 201)
        }
        if (path === '/users/me')
          return routeJson(route, {
            id: mockUser.id,
            email: 'demo@example.com',
            username: 'demo',
            display_name: 'Demo User',
          })
        if (path === '/users') return routeJson(route, { users: [] })
        if (path === '/projects') return routeJson(route, { projects: [project] })
        if (path === `/projects/${project.key}`) return routeJson(route, project)
        if (path === '/namespace-contexts') return routeJson(route, [context])
        if (path === `/namespace-contexts/${registry}/${namespace}`)
          return routeJson(route, context)
        if (path === '/issue-types')
          return routeJson(route, [{ id: 'task', name: 'Task', is_subtask: false }])
        if (path.endsWith('/custom-fields')) return routeJson(route, { fields: [] })
        if (path.endsWith('/members')) return routeJson(route, { members: [] })
        return route.fulfill({ status: 404 })
      })
      const search = namespaceEnabled
        ? `registry_instance_id=${registry}&namespace_id=${namespace}&project_key=${project.key}`
        : `project_key=${project.key}`
      await page.goto(`${appBaseURL}/issues/create?${search}`)
      const summary = page.getByLabel(/^(заголовок|summary)\s*\*?$/i)
      const submit = page.getByRole('button', { name: /^(создать|create)$/i })
      await summary.fill('Original issue')
      await expect(submit).toBeEnabled()
      await submit.click()
      await expect(page.getByRole('alert').first()).toBeVisible()
      await expect(submit).toBeEnabled()
      expect(writes).toHaveLength(1)
      await expect(summary).toHaveValue('Original issue')
      await expect(page.locator('body')).not.toContainText('private API details')
      const frozen = page.getByText('Повтор отправит исходную задачу с прежним ключом операции.')
      if (outcome === 'unknown') await expect(frozen).toBeVisible()
      else await expect(frozen).toBeHidden()
      if (outcome === 'validation400') {
        for (const width of [375, 1920, 2560]) {
          await page.setViewportSize({ width, height: width === 375 ? 812 : 1080 })
          await page.screenshot({
            path: testInfo.outputPath(`issue-rejection-${width}.png`),
            fullPage: true,
          })
        }
      }
      await summary.fill('Corrected issue')
      await submit.click()
      if (outcome === 'unknown') {
        await expect.poll(() => writes.length).toBe(2)
        await expect(submit).toBeEnabled()
        await expect(frozen).toBeVisible()
        await submit.click()
      }
      await expect.poll(() => new URL(page.url()).pathname).toBe(`/projects/${project.key}/backlog`)
      if (namespaceEnabled)
        expect(new URL(page.url()).searchParams.get('namespace_id')).toBe(namespace)
      expect(writes).toHaveLength(outcome === 'unknown' ? 3 : 2)
      expect(writes[0].key).toBeTruthy()
      if (outcome === 'unknown') {
        for (const request of writes.slice(1)) expect(request).toEqual(writes[0])
      } else {
        expect(writes[1].body.summary).toBe('Corrected issue')
        expect(writes[1].key).not.toBe(writes[0].key)
      }
    })
  }

  test('keeps the issue draft through project creation errors and authoritative catalogue refresh', async ({
    page,
    baseURL,
  }, testInfo) => {
    await installSsoMocks(page)
    const appBaseURL = (baseURL ?? 'http://localhost:4173').replace('127.0.0.1', 'localhost')
    const project = {
      id: '00000000-0000-0000-0000-000000000010',
      key: 'NEW',
      name: 'First Project',
      description: '',
      owner_id: mockUser.id,
      owner_name: 'Demo User',
      created_at: '2026-10-14T00:00:00Z',
      todo_count: 0,
      in_progress_count: 0,
      done_count: 0,
    }
    let projects: (typeof project)[] = []
    let denied = true
    let issueWrites = 0
    let projectWrites = 0
    await page.route('**/api/v1/**', (route) => {
      const request = route.request()
      const path = new URL(request.url()).pathname.replace('/api/v1', '')
      if (path === '/projects' && request.method() === 'GET') return routeJson(route, { projects })
      if (path === '/projects' && request.method() === 'POST') {
        projectWrites++
        if (denied) return routeJson(route, { message: 'Forbidden' }, 403)
        projects = [{ ...project, name: 'Renamed Project' }]
        return routeJson(route, project, 201)
      }
      if (path === '/issues' && request.method() === 'POST') issueWrites++
      if (path === '/users/me')
        return routeJson(route, {
          id: mockUser.id,
          email: 'demo@example.com',
          username: 'demo',
          display_name: 'Demo User',
        })
      if (path === '/users') return routeJson(route, { users: [] })
      if (path === '/issue-types')
        return routeJson(route, [{ id: 'task', name: 'Task', is_subtask: false }])
      if (path.endsWith('/custom-fields')) return routeJson(route, { fields: [] })
      if (path.endsWith('/members')) return routeJson(route, { members: [] })
      if (path === '/namespace-contexts') return routeJson(route, [])
      return route.fulfill({ status: 404 })
    })
    await page.goto(`${appBaseURL}/issues/create`)
    await page.getByRole('button', { name: /^(создать проект|create project)$/i }).click()
    const dialog = page.getByRole('dialog')
    // The issue draft stays outside the modal and survives both responses.
    await dialog.getByRole('button', { name: /^(отмена|cancel)$/i }).click()
    await page.getByLabel(/^(заголовок|summary)\s*\*?$/i).fill('Draft before project')
    await page.getByRole('button', { name: /^(создать проект|create project)$/i }).click()
    await dialog.getByLabel(/^(ключ|key)\s*\*/i).fill('NEW')
    await dialog.getByLabel(/^(название|name)\s*\*/i).fill('First Project')
    await dialog.getByRole('button', { name: /^(создать проект|create project)$/i }).click()
    await expect(dialog.getByRole('alert')).toBeVisible()
    await expect(dialog.getByLabel(/^(название|name)\s*\*/i)).toHaveValue('First Project')
    denied = false
    await dialog.getByRole('button', { name: /^(создать проект|create project)$/i }).click()
    await expect(dialog).toBeHidden()
    await expect(page.getByLabel(/^(проект|project)\s*\*?$/i)).toContainText('Renamed Project')
    await expect(page.getByLabel(/^(заголовок|summary)\s*\*?$/i)).toHaveValue(
      'Draft before project',
    )
    await expect(page.getByRole('button', { name: /^(создать|create)$/i })).toBeEnabled()
    expect(projectWrites).toBe(2)
    expect(issueWrites).toBe(0)
    for (const width of [375, 1920, 2560]) {
      await page.setViewportSize({ width, height: width === 375 ? 812 : 1080 })
      await page.screenshot({
        path: testInfo.outputPath(`inline-project-${width}.png`),
        fullPage: true,
      })
    }
    projects = []
    const readback = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === '/api/v1/projects' &&
        response.request().method() === 'GET',
    )
    await page.evaluate(() => window.dispatchEvent(new Event('visibilitychange')))
    await readback
    await expect(page.getByRole('button', { name: /^(создать|create)$/i })).toBeDisabled()
    await expect(page.getByLabel(/^(заголовок|summary)\s*\*?$/i)).toHaveValue(
      'Draft before project',
    )
    expect(issueWrites).toBe(0)
  })

  test('login then navigate through dashboard, projects, board and create issue', async ({
    page,
    baseURL,
  }, testInfo) => {
    test.setTimeout(120_000)
    const appBaseURL = (baseURL ?? 'http://localhost:4173').replace('127.0.0.1', 'localhost')
    await installSsoMocks(page)
    await page.route('**/api/v1/auth/login', (route) =>
      routeJson(route, {
        access_token: 'demo-token',
        token_type: 'Bearer',
        user_id: mockUser.id,
        email: 'demo@example.com',
      }),
    )
    await page.route('**/api/v1/dashboard', (route) => routeJson(route, { assigned_issues: [] }))
    await page.route('**/api/v1/users/me', (route) =>
      routeJson(route, {
        id: mockUser.id,
        email: 'demo@example.com',
        username: 'demo',
        display_name: 'Demo User',
      }),
    )
    await page.route('**/api/v1/statuses', (route) =>
      routeJson(route, [
        {
          id: 'todo',
          name: 'To Do',
          category: 'todo',
          position: 0,
          is_default: true,
          is_closed: false,
        },
        {
          id: 'inprogress',
          name: 'In Progress',
          category: 'inprogress',
          position: 1,
          is_default: false,
          is_closed: false,
        },
        {
          id: 'done',
          name: 'Done',
          category: 'done',
          position: 2,
          is_default: false,
          is_closed: true,
        },
      ]),
    )
    await page.route('**/api/v1/transitions', (route) => routeJson(route, []))
    await page.route('**/api/v1/users', (route) =>
      routeJson(route, {
        users: [{ id: mockUser.id, username: 'demo', display_name: 'Demo User' }],
      }),
    )
    await page.route('**/api/v1/issue-types**', (route) =>
      routeJson(route, [
        {
          id: 'task',
          name: 'Task',
          description: 'Standard task',
          icon: 'check-square',
          color: '#6b78e5',
          hierarchy_level: 0,
          is_subtask: false,
        },
      ]),
    )
    await page.route(/\/api\/v1\/projects(?:\?.*)?$/, (route) =>
      routeJson(route, {
        projects: [
          {
            id: '00000000-0000-0000-0000-000000000010',
            key: mockUser.key,
            name: mockUser.name,
            description: 'Smoke test project',
            owner_id: mockUser.id,
            todo_count: 1,
            in_progress_count: 0,
            done_count: 0,
          },
        ],
      }),
    )
    let projectDetails = {
      id: '00000000-0000-0000-0000-000000000010',
      key: mockUser.key,
      name: mockUser.name,
      description: 'Smoke test project',
      owner_id: mockUser.id,
      todo_count: 1,
      in_progress_count: 0,
      done_count: 0,
    }
    let failSave = true
    const registryId = '00000000-0000-0000-0000-000000000020'
    const namespaceId = '00000000-0000-0000-0000-000000000021'
    const projectContext = () => ({
      label: projectDetails.name,
      resource_key: projectDetails.key,
      binding: {
        schema_version: 1,
        operation_id: '00000000-0000-0000-0000-000000000022',
        generation: 1,
        state: 'active',
        drained: true,
        namespace: { registry_instance_id: registryId, namespace_id: namespaceId },
        resource: {
          instance_id: registryId,
          resource_id: projectDetails.id,
          kind: 'tracker_project',
        },
      },
    })
    await page.route(/\/api\/v1\/namespace-contexts(?:\?.*)?$/, (route) =>
      routeJson(route, [projectContext()]),
    )
    await page.route(`**/api/v1/namespace-contexts/${registryId}/${namespaceId}`, (route) =>
      routeJson(route, projectContext()),
    )
    await page.route(`**/api/v1/projects/${mockUser.key}`, (route) => {
      if (route.request().method() === 'PATCH') {
        if (failSave) return routeJson(route, { error: 'unavailable' }, 500)
        const values = route.request().postDataJSON()
        expect(values).toEqual({ description: 'Updated description' })
        projectDetails = { ...projectDetails, ...values }
      }
      return routeJson(route, projectDetails)
    })
    await page.route('**/api/v1/projects/**/members**', (route) =>
      routeJson(route, {
        members: [
          {
            project_id: '00000000-0000-0000-0000-000000000010',
            user_id: mockUser.id,
            role: 'owner',
          },
        ],
      }),
    )
    await page.route('**/api/v1/projects/*/board', (route) =>
      routeJson(route, {
        columns: [
          { id: 'todo', name: 'To Do', wip_limit: null, issue_ids: [mockUser.issueId] },
          { id: 'inprogress', name: 'In Progress', wip_limit: null, issue_ids: [] },
          { id: 'done', name: 'Done', wip_limit: null, issue_ids: [] },
        ],
        issues: [
          {
            id: mockUser.issueId,
            key: `${mockUser.key}-1`,
            summary: 'Smoke issue',
            description: '',
            issue_type: 'Task',
            status: 'To Do',
            priority: 'Medium',
            labels: [],
            assignee_id: null,
            assignee_name: null,
            reporter_id: mockUser.id,
            reporter_name: 'Demo User',
            project_name: mockUser.name,
          },
        ],
        sprint: {
          id: 'sprint-1',
          name: 'Sprint 1',
          goal: '',
          state: 'active',
          velocity: 0,
          remaining_days: 14,
          issue_ids: [mockUser.issueId],
        },
      }),
    )
    await page.route('**/api/v1/projects/*/backlog**', (route) =>
      routeJson(route, {
        project_id: '00000000-0000-0000-0000-000000000010',
        project_key: mockUser.key,
        sprint: {
          id: 'sprint-1',
          name: 'Sprint 1',
          goal: '',
          state: 'active',
          issue_ids: [],
          velocity: 0,
          remaining_days: 14,
          start_date: null,
          end_date: null,
        },
        sprint_issues: [],
        backlog_issues: [],
        backlog_total: 0,
        backlog_offset: 0,
        backlog_limit: 100,
      }),
    )
    await page.route('**/api/v1/projects/*/sprints', (route) =>
      routeJson(route, {
        sprints: [
          {
            id: 'sprint-1',
            name: 'Sprint 1',
            goal: '',
            state: 'active',
            issue_ids: [],
            velocity: 0,
            remaining_days: 14,
            start_date: null,
            end_date: null,
          },
        ],
      }),
    )
    await page.route('**/api/v1/projects/*/custom-fields', (route) =>
      routeJson(route, { fields: [] }),
    )
    await page.route('**/api/v1/projects/*/labels', (route) => routeJson(route, { labels: [] }))
    await page.route(`**/api/v1/issues/${mockUser.issueId}`, (route) =>
      routeJson(route, {
        id: mockUser.issueId,
        key: `${mockUser.key}-1`,
        summary: 'Smoke issue',
        description: 'Issue detail smoke description',
        issue_type: 'Task',
        status: 'To Do',
        status_id: 'todo',
        priority: 'Medium',
        labels: [],
        assignee_id: null,
        assignee_name: null,
        reporter_id: '00000000-0000-0000-0000-000000000002',
        reporter_name: 'Reporter User',
        project_key: mockUser.key,
        project_name: mockUser.name,
        sprint_id: null,
        original_estimate_seconds: null,
        remaining_estimate_seconds: null,
        time_spent_seconds: 0,
      }),
    )
    const commentWrites: string[] = []
    page.on('request', (request) => {
      if (
        /\/api\/v1\/(?:issues\/[^/]+\/comments|comments\/)/.test(new URL(request.url()).pathname) &&
        ['POST', 'PUT', 'PATCH', 'DELETE'].includes(request.method())
      ) {
        commentWrites.push(request.method())
      }
    })
    await page.route(`**/api/v1/issues/${mockUser.issueId}/comments**`, (route) => {
      if (route.request().method() !== 'GET') {
        return routeJson(route, { error: 'Read-only fixture' }, 405)
      }
      return routeJson(route, {
        comments: [
          {
            id: 'comment-markdown',
            issue_id: mockUser.issueId,
            author_id: mockUser.id,
            author_name: 'Demo User',
            body: markdownComment,
            created_at: '2026-09-01T10:00:00Z',
            updated_at: '2026-09-01T10:00:00Z',
          },
        ],
      })
    })
    await page.route(`**/api/v1/issues/${mockUser.issueId}/worklogs**`, (route) =>
      routeJson(route, { worklogs: [] }),
    )
    await page.route(`**/api/v1/issues/${mockUser.issueId}/attachments`, (route) =>
      routeJson(route, { attachments: [] }),
    )
    await page.route(`**/api/v1/issues/${mockUser.issueId}/labels`, (route) =>
      routeJson(route, { labels: [] }),
    )
    await page.route(`**/api/v1/issues/${mockUser.issueId}/links`, (route) =>
      routeJson(route, { links: [] }),
    )
    await page.route(`**/api/v1/issues/${mockUser.issueId}/custom-fields`, (route) =>
      routeJson(route, { values: [] }),
    )
    let failVotes = false
    let failWatchers = false
    const engagementWrites: string[] = []
    page.on('request', (request) => {
      if (
        /\/api\/v1\/issues\/[^/]+\/(?:vote|watch)$/.test(new URL(request.url()).pathname) &&
        request.method() !== 'GET'
      ) {
        engagementWrites.push(request.method())
      }
    })
    await page.route(`**/api/v1/issues/${mockUser.issueId}/votes`, (route) =>
      failVotes
        ? routeJson(route, { error: 'private engagement upstream failure' }, 500)
        : routeJson(route, {
            count: 1,
            votes: [
              {
                user_id: '00000000-0000-0000-0000-000000000003',
                username: 'voter',
                display_name: 'Voter User',
                voted_at: '2026-09-01T10:00:00Z',
              },
            ],
          }),
    )
    await page.route(`**/api/v1/issues/${mockUser.issueId}/watchers`, (route) =>
      failWatchers
        ? routeJson(route, { error: 'private engagement upstream failure' }, 500)
        : routeJson(route, {
            watchers: [
              {
                user_id: mockUser.id,
                username: 'demo',
                display_name: 'Demo User',
              },
            ],
          }),
    )
    await page.route('**/api/v1/notifications', (route) =>
      routeJson(route, { notifications: [], unread_count: 0 }),
    )
    await page.route('**/api/v1/events**', (route) => routeJson(route, ''))
    await page.route('**/api/v1/auth/refresh', (route) =>
      routeJson(route, {
        access_token: 'demo-token',
        token_type: 'Bearer',
        user_id: mockUser.id,
        email: 'demo@example.com',
      }),
    )
    await page.route('**/api/v1/notifications**', (route) =>
      routeJson(route, { notifications: [], unread_count: 0 }),
    )

    await page.goto(`${appBaseURL}/`)
    await expect(page).toHaveURL(`${appBaseURL}/`, { timeout: 15_000 })
    await expect(
      page.getByRole('heading', { name: /dashboard|team dashboard|мои задачи|командный дашборд/i }),
    ).toBeVisible()

    await page.goto(`${appBaseURL}/projects`)
    await expect(page.getByRole('main').getByText(mockUser.name, { exact: true })).toBeVisible()

    await page.getByRole('link', { name: /(?:settings|настройки проекта):/i }).click()
    await expect.poll(() => new URL(page.url()).pathname).toBe(`/projects/${mockUser.key}/settings`)
    expect(new URL(page.url()).searchParams.get('registry_instance_id')).toBe(
      namespaceEnabled ? registryId : null,
    )
    expect(new URL(page.url()).searchParams.get('namespace_id')).toBe(
      namespaceEnabled ? namespaceId : null,
    )
    const description = page.getByLabel(/^(description|описание)$/i)
    await description.fill('Updated description')
    await page.getByRole('button', { name: /^(save|сохранить)$/i }).click()
    await expect(page.getByRole('alert')).toContainText(/save|сохранить/i)
    await expect(description).toHaveValue('Updated description')
    projectDetails = { ...projectDetails, name: 'Renamed by another editor' }
    await page.evaluate(() => window.dispatchEvent(new Event('visibilitychange')))
    await expect(page.getByLabel(/^(name|название)$/i)).toHaveValue('Renamed by another editor')
    await expect(description).toHaveValue('Updated description')
    failSave = false
    await page.getByRole('button', { name: /^(save|сохранить)$/i }).click()
    await expect(page.getByRole('button', { name: /^(save|сохранить)$/i })).toBeDisabled()
    expect(projectDetails.name).toBe('Renamed by another editor')
    for (const width of [375, 1920, 2560]) {
      await page.setViewportSize({ width, height: 1080 })
      await page.screenshot({
        path: testInfo.outputPath(`project-settings-${width}.png`),
        fullPage: true,
      })
    }
    projectDetails = { ...projectDetails, name: mockUser.name }
    await page.getByRole('link', { name: /custom fields|кастомные поля/i }).click()
    await expect(page.getByRole('textbox', { name: /field name|название поля/i })).toHaveCount(0)
    await page.getByRole('button', { name: /add field|добавить поле/i }).click()
    await expect(page.getByRole('textbox', { name: /field name|название поля/i })).toBeVisible()
    await page.getByRole('button', { name: /cancel|отмена/i }).click()
    await expect(page.getByRole('textbox', { name: /field name|название поля/i })).toHaveCount(0)

    await page.goto(`${appBaseURL}/projects/${mockUser.key}/board`)
    await expect(page.getByText('Smoke issue').first()).toBeVisible()

    await page.goto(
      `${appBaseURL}/projects/${mockUser.key}/backlog${namespaceEnabled ? `?registry_instance_id=${registryId}&namespace_id=${namespaceId}` : ''}`,
    )
    await expect(page.getByRole('heading', { name: /backlog|бэклог/i })).toBeVisible()
    const createLinks = page.locator('main a[href^="/issues/create"]')
    await expect(createLinks).toHaveCount(2)
    await createLinks.last().click()
    await expect.poll(() => new URL(page.url()).pathname).toBe('/issues/create')
    expect(new URL(page.url()).searchParams.get('project_key')).toBe(mockUser.key)
    expect(new URL(page.url()).searchParams.get('registry_instance_id')).toBe(
      namespaceEnabled ? registryId : null,
    )
    expect(new URL(page.url()).searchParams.get('namespace_id')).toBe(
      namespaceEnabled ? namespaceId : null,
    )
    await expect(page.locator('#issue-project')).toHaveAttribute('value', mockUser.key)

    await page.goto(`${appBaseURL}/projects/${mockUser.key}/board`)
    await expect(page.getByText('Smoke issue').first()).toBeVisible()
    await page.goto(`${appBaseURL}/issues/${mockUser.issueId}`)
    await expect(page.getByText('Issue detail smoke description')).toBeVisible()
    const engagement = page.getByTestId('issue-engagement-panel')
    await expect(engagement.getByText(/^(1 total|всего 1)$/i)).toBeVisible()
    await expect(
      engagement.getByRole('button', { name: /stop watching|перестать следить/i }),
    ).toBeEnabled()
    failVotes = true
    failWatchers = true
    await page.evaluate(() => window.dispatchEvent(new Event('visibilitychange')))
    // The production query policy retries failed GETs at 1/2/4/8 seconds.
    await expect(
      engagement.getByText(/Не удалось загрузить голоса|Unable to load votes/),
    ).toBeVisible({ timeout: 25_000 })
    await expect(
      engagement.getByText(/Не удалось загрузить наблюдателей|Unable to load watchers/),
    ).toBeVisible()
    const voteAction = engagement.getByRole('button', { name: /^(vote|голос)$/i })
    const watchAction = engagement.getByRole('button', {
      name: /^(stop watching|перестать следить)$/i,
    })
    await expect(voteAction).toBeDisabled()
    await expect(watchAction).toBeDisabled()
    await expect(
      engagement.getByText(/^(0 total|всего 0|No watchers|Наблюдателей нет)$/i),
    ).toHaveCount(0)
    await expect(engagement.getByText(/^(1 total|всего 1)$/i)).toHaveCount(0)
    await expect(engagement.getByText('private engagement upstream failure')).toHaveCount(0)
    for (const width of [375, 1920, 2560]) {
      await page.setViewportSize({ width, height: 1080 })
      await expect(
        engagement.getByText(/Не удалось загрузить голоса|Unable to load votes/),
      ).toBeVisible()
      await expect(
        engagement.getByText(/Не удалось загрузить наблюдателей|Unable to load watchers/),
      ).toBeVisible()
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
        true,
      )
      await page.screenshot({
        path: testInfo.outputPath(`engagement-error-${width}.png`),
        fullPage: true,
      })
    }
    failVotes = false
    await engagement
      .getByRole('button', { name: /retry|повторить/i })
      .first()
      .click()
    await expect(engagement.getByText(/^(1 total|всего 1)$/i)).toBeVisible()
    await expect(voteAction).toBeEnabled()
    await expect(watchAction).toBeDisabled()
    failWatchers = false
    await engagement.getByRole('button', { name: /retry|повторить/i }).click()
    await expect(
      engagement.getByRole('button', { name: /stop watching|перестать следить/i }),
    ).toBeEnabled()
    expect(engagementWrites).toEqual([])
    await expect(page.getByRole('button', { name: /vote|голос/i })).toBeVisible()
    await expect(
      page.getByRole('button', { name: /stop watching|перестать следить/i }),
    ).toBeVisible()
    await expect(page.getByText(/1 total|всего 1/i)).toBeVisible()

    for (const viewport of [
      { width: 375, height: 812 },
      { width: 1440, height: 900 },
      { width: 1920, height: 1080 },
      { width: 2560, height: 1440 },
    ]) {
      await page.setViewportSize(viewport)

      for (const [path, mode] of [
        ['/projects', 'wide'],
        [`/projects/${mockUser.key}/settings`, 'reading'],
        [`/projects/${mockUser.key}/settings/custom-fields`, 'reading'],
        [`/issues/create?project_key=${mockUser.key}`, 'reading'],
        [`/issues/${mockUser.issueId}`, 'detail-with-aside'],
      ] as const) {
        await page.goto(`${appBaseURL}${path}`)
        if (mode === 'wide')
          await expect(
            page.getByRole('main').getByText(mockUser.name, { exact: true }),
          ).toBeVisible()
        if (path.startsWith('/projects/') && mode === 'reading')
          await expect(page.getByRole('heading', { name: new RegExp(mockUser.name) })).toBeVisible()
        if (path.startsWith('/issues/create')) {
          await expect(
            page.getByRole('heading', { name: /create issue|создать задачу/i }),
          ).toBeVisible()
        }
        if (mode === 'detail-with-aside') {
          await expect(page.getByText('Issue detail smoke description')).toBeVisible()
          for (const [tabName, tabKey] of [
            [/comments|комментарии/i, 'comments'],
            [/activity|активность/i, 'activity'],
          ] as const) {
            await page.getByRole('tab', { name: tabName }).click()
            await expect(page.getByRole('tab', { name: tabName })).toHaveAttribute(
              'aria-selected',
              'true',
            )
            await page
              .getByRole('tablist')
              .evaluate((element) =>
                Promise.allSettled(
                  element.getAnimations({ subtree: true }).map((animation) => animation.finished),
                ),
              )
            const panel = page.getByRole('tabpanel')
            await expect(panel.getByRole('heading', { name: 'Итоги', level: 1 })).toBeVisible()
            await expect(panel.locator('strong')).toHaveText('Результат')
            await expect(panel.getByRole('listitem')).toHaveCount(2)
            await expect(panel.locator('pre code')).toHaveText(
              '  ' + 'code_block_'.repeat(40) + '\n',
            )
            await expect(panel.locator('script, [href^="javascript:"]')).toHaveCount(0)
            await expect(panel.getByText('Первая строка Вторая строка')).toHaveCSS(
              'white-space',
              'pre-line',
            )
            await expect
              .poll(() =>
                page.evaluate(
                  () =>
                    document.documentElement.scrollWidth <= document.documentElement.clientWidth,
                ),
              )
              .toBe(true)
            let previousGeometry: string | undefined
            await expect
              .poll(async () => {
                const box = await panel.boundingBox()
                const geometry = JSON.stringify(box)
                const stable = box !== null && geometry === previousGeometry
                previousGeometry = geometry
                return stable
              })
              .toBe(true)
            if (viewport.width !== 1440) {
              await page.getByRole('main').evaluate((element) => {
                element.scrollTop = 0
              })
              await page.evaluate(() => window.scrollTo(0, 0))
              await page.screenshot({
                path: testInfo.outputPath(`markdown-${viewport.width}-${tabKey}.png`),
                fullPage: true,
              })
            }
          }
        }

        if (viewport.width >= 1440 && path.endsWith('/settings')) {
          const header = page
            .getByRole('region', { name: `${mockUser.name} · ${mockUser.key}`, exact: true })
            .locator('.base-sidebar-group-header')
          const action = await header
            .getByRole('link', { name: /settings|настройки/i })
            .boundingBox()
          const caret = await header.locator('.base-sidebar-group-chevron').boundingBox()
          const row = await header.boundingBox()
          expect(action).not.toBeNull()
          expect(caret).not.toBeNull()
          expect(action!.x).toBeGreaterThan(row!.x)
          expect(action!.x + action!.width).toBeLessThanOrEqual(caret!.x)
          expect(caret!.x + caret!.width).toBeLessThanOrEqual(row!.x + row!.width)
          await page
            .getByRole('button', { name: /collapse sidebar|свернуть боковую панель/i })
            .click()
          const logo = await page.getByRole('banner').locator('.base-shell-mark').boundingBox()
          const rail = await page.getByRole('complementary').boundingBox()
          expect(
            Math.abs(logo!.x + logo!.width / 2 - (rail!.x + rail!.width / 2)),
          ).toBeLessThanOrEqual(1)
          await page
            .getByRole('button', { name: /expand sidebar|развернуть боковую панель/i })
            .click()
        }
        const layout = page.locator('[data-page-layout]')
        await expect(layout).toHaveAttribute('data-page-layout', mode)
        const geometry = await layout.evaluate((element) => {
          const frame = element.parentElement
          const frameStyle = frame ? getComputedStyle(frame) : null
          return {
            documentFits:
              document.documentElement.scrollWidth <= document.documentElement.clientWidth,
            layoutWidth: element.getBoundingClientRect().width,
            availableWidth:
              (frame?.getBoundingClientRect().width ?? 0) -
              Number.parseFloat(frameStyle?.paddingLeft ?? '0') -
              Number.parseFloat(frameStyle?.paddingRight ?? '0'),
          }
        })

        expect(geometry.documentFits).toBe(true)
        if (mode === 'reading') {
          expect(geometry.layoutWidth).toBeLessThanOrEqual(761)
        } else {
          expect(Math.abs(geometry.layoutWidth - geometry.availableWidth)).toBeLessThanOrEqual(1)
        }
        await page.waitForTimeout(1000)
        const testOutputPath = testInfo.outputPath(`shell-${viewport.width}-${mode}.png`)
        await page.screenshot({
          path: testOutputPath,
          fullPage: true,
        })
        if (
          process.env.UPDATE_README_SCREENSHOTS === '1' &&
          (viewport.width === 375 || viewport.width === 1920)
        ) {
          const viewportDir = `${viewport.width}x${viewport.height}`
          const readmeOutputDir = nodePath.resolve(
            process.cwd(),
            '..',
            'docs',
            'screenshots',
            viewportDir,
          )
          mkdirSync(readmeOutputDir, { recursive: true })
          await page.screenshot({
            path: nodePath.join(readmeOutputDir, `${mode}.png`),
            fullPage: true,
          })
        }
      }
    }
    expect(commentWrites).toEqual([])
  })
})
