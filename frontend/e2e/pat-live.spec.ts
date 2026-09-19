import { randomUUID } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { expect, test } from '@playwright/test'

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
      ) as {
        email: string
        password: string
      })
    : { email: '', password: '' }

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
    'http://localhost:8812/api/workflows',
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
