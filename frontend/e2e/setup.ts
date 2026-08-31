import type { Page } from '@playwright/test'
import {
  closeSync,
  existsSync,
  openSync,
  readFileSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs'

export const API_BASE = process.env.VITE_API_BASE_URL ?? 'http://127.0.0.1:3456/api/v1'

// Single shared seed per test-run (process-wide). Parallel workers/projects reuse it,
// avoiding simultaneous /auth/register + /auth/login calls that trip the auth rate limiter.
type ApiContext = {
  token: string
  userId: string
  projectId: string
  issueId: string
  issueKey: string
  /** epoch ms when the access token expires */
  expiresAt: number
}

let seedPromise: Promise<ApiContext> | null = null

// Cross-process seed lock: Playwright runs each --project in its own worker
// process, so the module-level seedPromise does not dedupe logins between
// projects. The lock file serializes seeds and lets the first finished
// process publish the token for the others to reuse.
const seedLockPath = '/tmp/tt-e2e-seed.lock'
const seedCachePath = '/tmp/tt-e2e-seed.json'

function acquireSeedLock(): number {
  const deadline = Date.now() + 90_000
  while (Date.now() < deadline) {
    try {
      const fd = openSync(seedLockPath, 'wx')
      return fd
    } catch {
      // Stale-lock takeover: a worker killed mid-seed would otherwise wedge
      // every other worker behind this lock forever.
      try {
        const stat = statSync(seedLockPath)
        if (Date.now() - stat.mtimeMs > 60_000) unlinkSync(seedLockPath)
      } catch {
        // lock disappeared — retry immediately
      }
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 500)
    }
  }
  throw new Error('seed lock timeout (90s)')
}

function releaseSeedLock(fd: number) {
  closeSync(fd)
  try {
    unlinkSync(seedLockPath)
  } catch {
    // already removed by stale takeover
  }
}

type AuthResponse = {
  access_token: string
  user_id: string
  email?: string
}

async function post(path: string, body: object, token?: string) {
  return fetchJsonWithRetry(`${API_BASE}${path}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(body),
  })
}

async function get(path: string, token: string) {
  return fetchJsonWithRetry(`${API_BASE}${path}`, {
    headers: { Authorization: `Bearer ${token}` },
  })
}

// NOTE: fetchJsonWithRetry is declared with `function` (hoisted) so the
// post/get helpers above can reference it before its textual definition.
function fetchJsonWithRetry(url: string, init: RequestInit, attempts = 24) {
  return _fetchJsonWithRetry(url, init, attempts)
}

async function _fetchJsonWithRetry(url: string, init: RequestInit, attempts = 24) {
  for (let i = 0; i < attempts; i++) {
    const res = await fetch(url, init)
    if (res.status === 429) {
      await new Promise((r) => setTimeout(r, 5_000))
      continue
    }
    return { status: res.status, data: await res.json().catch(() => ({})) }
  }
  throw new Error(`persistent 429 for ${url}`)
}

async function seed(): Promise<ApiContext> {
  const credentials = {
    email: 'demo@example.com',
    password: 'demo',
    username: 'demo',
    name: 'Demo User',
  }

  const registerRes = await fetchJsonWithRetry(`${API_BASE}/auth/register`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(credentials),
  })
  if (registerRes.status !== 201 && registerRes.status !== 409) {
    throw new Error(`register failed: ${registerRes.status} ${JSON.stringify(registerRes.data)}`)
  }

  const loginRes = await fetchJsonWithRetry(`${API_BASE}/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email: credentials.email, password: credentials.password }),
  })
  if (loginRes.status !== 200) {
    throw new Error(`login failed: ${loginRes.status} ${JSON.stringify(loginRes.data)}`)
  }
  const { access_token, user_id } = loginRes.data

  const projectsRes = await get('/projects', access_token)
  type ProjectItem = { id: string; key: string }
  const project = (projectsRes.data.projects as ProjectItem[] | undefined)?.find(
    (p) => p.key === 'DEMO',
  )
  let projectId: string
  if (project) {
    projectId = project.id
  } else {
    const createProjectRes = await post(
      '/projects',
      {
        key: 'DEMO',
        name: 'Demo Project',
        description: 'Playwright E2E project',
      },
      access_token,
    )
    if (createProjectRes.status !== 200 && createProjectRes.status !== 201) {
      throw new Error(
        `create project failed: ${createProjectRes.status} ${JSON.stringify(createProjectRes.data)}`,
      )
    }
    projectId = (createProjectRes.data as ProjectItem).id
  }

  // Fresh issue per run: prevents cross-run worklog accumulation on the seeded issue
  type IssueItem = { id: string; key: string; summary: string }
  const summary = `Smoke issue ${process.pid}-${Date.now() % 100000}`
  const createIssueRes = await post(
    '/issues',
    {
      project_key: 'DEMO',
      issue_type: 'task',
      summary,
      description: 'Created by E2E setup',
      priority: 'medium',
      reporter_id: user_id,
    },
    access_token,
  )
  if (createIssueRes.status !== 200 && createIssueRes.status !== 201) {
    throw new Error(
      `create issue failed: ${createIssueRes.status} ${JSON.stringify(createIssueRes.data)}`,
    )
  }
  const issue = createIssueRes.data as IssueItem

  const worklogRes = await post(
    `/issues/${issue.id}/worklogs`,
    {
      description: 'Initial work',
      started_at: new Date().toISOString(),
      duration_seconds: 180,
    },
    access_token,
  )
  if (worklogRes.status !== 200 && worklogRes.status !== 201) {
    throw new Error(
      `create worklog failed: ${worklogRes.status} ${JSON.stringify(worklogRes.data)}`,
    )
  }

  return {
    token: access_token,
    userId: user_id,
    projectId,
    issueId: issue.id,
    issueKey: issue.key,
    expiresAt: decodeJwtExpiryMs(access_token),
  }
}

function decodeJwtExpiryMs(jwt: string): number {
  try {
    const payload = JSON.parse(Buffer.from(jwt.split('.')[1]!, 'base64url').toString('utf8'))
    return typeof payload.exp === 'number' ? payload.exp * 1000 : Date.now()
  } catch {
    return Date.now()
  }
}

export async function seedIntegrationData(): Promise<ApiContext> {
  if (!seedPromise) {
    seedPromise = (async () => {
      const fd = acquireSeedLock()
      try {
        const cached = existsSync(seedCachePath)
          ? JSON.parse(readFileSync(seedCachePath, 'utf8'))
          : null
        const fresh =
          cached &&
          Date.now() - cached.at < 10 * 60_000 &&
          cached.ctx.expiresAt > Date.now() + 60_000
        if (fresh) return cached.ctx as ApiContext
        // Cache hit but the access token is close to expiry: re-login and
        // keep the seeded project/issue instead of failing mid-suite with 401s.
        if (cached && cached.ctx.expiresAt > Date.now()) {
          const loginRes = await fetchJsonWithRetry(`${API_BASE}/auth/login`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ email: 'demo@example.com', password: 'demo' }),
          })
          if (loginRes.status === 200) {
            const ctx = {
              ...cached.ctx,
              token: loginRes.data.access_token,
              userId: loginRes.data.user_id,
              expiresAt: decodeJwtExpiryMs(loginRes.data.access_token),
            }
            writeFileSync(seedCachePath, JSON.stringify({ at: Date.now(), ctx }))
            return ctx as ApiContext
          }
          // login failed (e.g. rate limited through): fall through to a full seed
        }
        const ctx = await seed()
        writeFileSync(seedCachePath, JSON.stringify({ at: Date.now(), ctx }))
        return ctx
      } finally {
        releaseSeedLock(fd)
      }
    })().catch((error) => {
      seedPromise = null
      throw error
    })
  }
  return seedPromise
}

export async function apiLogin() {
  return fetchJsonWithRetry(`${API_BASE}/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email: 'demo@example.com', password: 'demo' }),
  })
}

export async function authenticatePage(page: Page): Promise<AuthResponse> {
  for (let i = 0; i < 24; i++) {
    const res = await page.request.post(`${API_BASE}/auth/login`, {
      data: { email: 'demo@example.com', password: 'demo' },
    })
    const data = await res.json().catch(() => ({}))
    if (res.status() === 200) {
      return data as AuthResponse
    }
    if (res.status() === 429) {
      await new Promise((r) => setTimeout(r, 5_000))
      continue
    }
    throw new Error(`browser login failed: ${res.status()} ${JSON.stringify(data)}`)
  }
  throw new Error('browser login failed: persistent 429')
}

export async function apiGet(path: string, token: string) {
  return fetchJsonWithRetry(`${API_BASE}${path}`, {
    headers: { Authorization: `Bearer ${token}` },
  })
}

export async function apiPost(path: string, body: object, token?: string) {
  return fetchJsonWithRetry(`${API_BASE}${path}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(body),
  })
}

export const API_BASE_URL = API_BASE

/**
 * UI-form login shared by live specs. The auth rate limiter (5 req / 15 s per
 * IP) returns 429 with a Retry-After hint when several specs log in within
 * one run; retry instead of failing the test.
 */
export async function uiLogin(
  page: import('@playwright/test').Page,
  email = 'demo@example.com',
  password = 'demo',
) {
  const base = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'
  for (let attempt = 0; attempt < 8; attempt++) {
    await page.goto(`${base}/login`)
    await page.getByRole('textbox').nth(0).fill(email)
    await page.getByRole('textbox').nth(1).fill(password)
    await page.getByRole('button', { name: /войти|login/i }).click()
    const ok = await page
      .waitForURL((u) => !u.pathname.includes('/login'), { timeout: 10_000 })
      .then(() => true)
      .catch(() => false)
    if (ok) return
    // 429 backoff aligned with the auth limiter window
    await page.waitForTimeout(5_000)
  }
  throw new Error('uiLogin: still on /login after retries (rate limit?)')
}
