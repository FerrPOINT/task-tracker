import { randomUUID } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { request as httpRequest } from 'node:http'
import { fileURLToPath } from 'node:url'
import { expect, test } from '@playwright/test'

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

async function docker(method: string, path: string): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const request = httpRequest(
      { socketPath: '/var/run/docker.sock', method, path, timeout: 30_000 },
      (response) => {
        let body = ''
        response.setEncoding('utf8')
        response.on('data', (chunk: string) => (body += chunk))
        response.on('end', () => {
          if ((response.statusCode ?? 500) >= 400) {
            reject(new Error(`Docker ${method} ${path}: HTTP ${response.statusCode}`))
          } else {
            try {
              resolve(body ? JSON.parse(body) : null)
            } catch (error) {
              reject(error)
            }
          }
        })
      },
    )
    request.on('timeout', () => request.destroy(new Error('QA Docker request timed out')))
    request.on('error', reject)
    request.end()
  })
}

test('six product APIs fail closed during an isolated Central Auth outage', async ({ request }) => {
  test.skip(
    !process.env.SDLC_ISOLATED_QA_PROJECT,
    'Requires an explicitly isolated Compose project',
  )
  test.setTimeout(180_000)
  const project = process.env.SDLC_ISOLATED_QA_PROJECT!
  expect(project).toMatch(/^sdlc-(clean-install-qa|platform-smoke-[a-zA-Z0-9-]+)$/)
  const filters = encodeURIComponent(
    JSON.stringify({
      label: [`com.docker.compose.project=${project}`, 'com.docker.compose.service=auth'],
    }),
  )
  const containers = (await docker('GET', `/containers/json?filters=${filters}`)) as {
    Id: string
    Labels: Record<string, string>
  }[]
  expect(containers).toHaveLength(1)
  const container = containers[0]!
  expect(container.Labels['com.docker.compose.project']).toBe(project)
  expect(container.Labels['com.docker.compose.service']).toBe('auth')
  expect(container.Id).toMatch(/^[a-f0-9]{64}$/)
  const login = await request.post('http://localhost:7701/auth/login', { data: account })
  expect(login.status()).toBe(200)
  const { access_token: accessToken } = (await login.json()) as { access_token: string }
  const session = { Authorization: `Bearer ${accessToken}` }
  const issued = await request.post('http://localhost:7701/auth/tokens', {
    headers: session,
    data: {
      label: `qa-outage-${randomUUID()}`,
      scopes: [
        'admin-panel',
        'ci-cd',
        'task-tracker',
        'wiki',
        'fleet-control',
        'project-workflow',
      ].map((service) => `${service}:read`),
      expires_in_days: 1,
    },
  })
  expect(issued.status()).toBe(201)
  const personal = (await issued.json()) as { id: string; secret: string }
  const token = { Authorization: `Bearer ${personal.secret}` }
  const endpoints = [
    'http://localhost:7771/api/v1/auth/me',
    'http://localhost:7711/api/v1/users',
    'http://localhost:7721/api/v1/users',
    'http://localhost:7731/api/v1/users/me',
    'http://localhost:7741/api/v1/users/me',
    'http://localhost:7752/api/workflows',
  ]
  let stopped = false
  try {
    for (const headers of [session, token]) {
      for (const url of endpoints)
        expect((await request.get(url, { headers })).status(), url).toBe(200)
    }
    stopped = true
    await docker('POST', `/containers/${container.Id}/stop?t=2`)
    for (const headers of [session, token]) {
      for (const url of endpoints) {
        expect((await request.get(url, { headers })).status(), `Auth outage: ${url}`).toBe(503)
      }
    }
  } finally {
    if (stopped) await docker('POST', `/containers/${container.Id}/start`)
    await expect
      .poll(
        async () => {
          try {
            return (await request.get('http://localhost:7701/health', { timeout: 3000 })).status()
          } catch {
            return 0
          }
        },
        { timeout: 60_000 },
      )
      .toBe(200)
    expect(
      (
        await request.delete(`http://localhost:7701/auth/tokens/${personal.id}`, {
          headers: session,
        })
      ).status(),
    ).toBe(204)
  }
  for (const url of endpoints) {
    expect((await request.get(url, { headers: session })).status(), `Auth recovery: ${url}`).toBe(
      200,
    )
    expect((await request.get(url, { headers: token })).status(), `Token revoked: ${url}`).toBe(401)
  }
})

test('personal token reads six product APIs and cannot outlive revocation', async ({ request }) => {
  test.setTimeout(90_000)
  const login = await request.post('http://localhost:7701/auth/login', {
    data: { email: account.email, password: account.password },
  })
  expect(login.ok()).toBeTruthy()
  const { access_token: accessToken } = (await login.json()) as { access_token: string }
  const owner = { Authorization: `Bearer ${accessToken}` }
  const scopes = [
    'admin-panel',
    'ci-cd',
    'task-tracker',
    'wiki',
    'fleet-control',
    'project-workflow',
  ].map((service) => `${service}:read`)
  const issued = await request.post('http://localhost:7701/auth/tokens', {
    headers: owner,
    data: { label: `qa-pat-${randomUUID()}`, scopes, expires_in_days: 1 },
  })
  expect(issued.status()).toBe(201)
  const { id, secret } = (await issued.json()) as { id: string; secret: string }
  const bearer = { Authorization: `Bearer ${secret}` }
  const endpoints = [
    'http://localhost:7771/api/v1/auth/me',
    'http://localhost:7711/api/v1/users',
    'http://localhost:7721/api/v1/users',
    'http://localhost:7731/api/v1/users/me',
    'http://localhost:7741/api/v1/users/me',
    'http://localhost:7752/api/workflows',
  ]
  try {
    for (const url of endpoints) {
      const response = await request.get(url, { headers: bearer })
      expect(response.status(), new URL(url).host).toBe(200)
    }
  } finally {
    const revoked = await request.delete(`http://localhost:7701/auth/tokens/${id}`, {
      headers: owner,
    })
    expect(revoked.status()).toBe(204)
  }
  for (const url of endpoints) {
    const response = await request.get(url, { headers: bearer })
    expect(response.status(), new URL(url).host).toBe(401)
  }

  const narrowIssue = await request.post('http://localhost:7701/auth/tokens', {
    headers: owner,
    data: { label: `qa-pat-${randomUUID()}`, scopes: ['task-tracker:read'], expires_in_days: 1 },
  })
  expect(narrowIssue.status()).toBe(201)
  const { id: narrowId, secret: narrowSecret } = (await narrowIssue.json()) as {
    id: string
    secret: string
  }
  const narrow = { Authorization: `Bearer ${narrowSecret}` }
  try {
    expect((await request.get(endpoints[2], { headers: narrow })).status()).toBe(200)
    for (const url of endpoints.filter((_, index) => index !== 2)) {
      const response = await request.get(url, { headers: narrow })
      expect(response.status(), new URL(url).host).toBe(403)
    }
    const write = await request.post('http://localhost:7721/api/v1/projects', {
      headers: narrow,
      data: {},
    })
    expect(write.status()).toBe(403)
  } finally {
    const revoked = await request.delete(`http://localhost:7701/auth/tokens/${narrowId}`, {
      headers: owner,
    })
    expect(revoked.status()).toBe(204)
  }
})
