import { generateKeyPairSync, sign } from 'node:crypto'
import { test, expect, type Page, type Route } from '@playwright/test'

const mockUser = {
  id: '00000000-0000-0000-0000-000000000001',
  key: 'DEMO',
  name: 'Demo Project',
  issueId: 'issue-1',
}

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

  await page.route('**/oidc/authorize**', async (route) => {
    const url = new URL(route.request().url())
    issuer = url.origin
    nonce = url.searchParams.get('nonce') ?? ''
    const callback = new URL(url.searchParams.get('redirect_uri') ?? '/')
    callback.searchParams.set('code', 'qa-code')
    callback.searchParams.set('state', url.searchParams.get('state') ?? '')
    await route.fulfill({
      status: 302,
      headers: { location: callback.toString() },
      body: '',
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
    await page.route('**/api/v1/projects', (route) =>
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
    await page.route(`**/api/v1/issues/${mockUser.issueId}/comments**`, (route) =>
      routeJson(route, { comments: [] }),
    )
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
    await page.route(`**/api/v1/issues/${mockUser.issueId}/votes`, (route) =>
      routeJson(route, {
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
      routeJson(route, {
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
    await expect(page.getByText(mockUser.name)).toBeVisible()

    await page.goto(`${appBaseURL}/projects/${mockUser.key}/board`)
    await expect(page.getByText('Smoke issue').first()).toBeVisible()

    await page.goto(`${appBaseURL}/projects/${mockUser.key}/backlog`)
    await expect(page.getByRole('heading', { name: /backlog|бэклог/i })).toBeVisible()
    const createLinks = page.locator('main a[href^="/issues/create"]')
    await expect(createLinks).toHaveCount(2)
    await createLinks.last().click()
    await expect(page).toHaveURL(`${appBaseURL}/issues/create?project_key=${mockUser.key}`)
    await expect(page.locator('#issue-project')).toHaveValue(mockUser.key)

    await page.goto(`${appBaseURL}/projects/${mockUser.key}/board`)
    await expect(page.getByText('Smoke issue').first()).toBeVisible()
    await page.goto(`${appBaseURL}/issues/${mockUser.issueId}`)
    await expect(page.getByText('Issue detail smoke description')).toBeVisible()
    await expect(page.getByRole('button', { name: /vote|голос/i })).toBeVisible()
    await expect(
      page.getByRole('button', { name: /stop watching|перестать следить/i }),
    ).toBeVisible()
    await expect(page.getByText(/1 total|всего 1/i)).toBeVisible()

    for (const viewport of [
      { width: 375, height: 812 },
      { width: 1440, height: 900 },
      { width: 2560, height: 1440 },
    ]) {
      await page.setViewportSize(viewport)

      for (const [path, mode] of [
        ['/projects', 'wide'],
        [`/issues/create?project_key=${mockUser.key}`, 'reading'],
        [`/issues/${mockUser.issueId}`, 'detail-with-aside'],
      ] as const) {
        await page.goto(`${appBaseURL}${path}`)
        if (mode === 'wide') await expect(page.getByText(mockUser.name).first()).toBeVisible()
        if (mode === 'reading') {
          await expect(
            page.getByRole('heading', { name: /create issue|создать задачу/i }),
          ).toBeVisible()
        }
        if (mode === 'detail-with-aside') {
          await expect(page.getByText('Issue detail smoke description')).toBeVisible()
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
        await page.screenshot({
          path: testInfo.outputPath(`shell-${viewport.width}-${mode}.png`),
          fullPage: true,
        })
      }
    }
  })
})
