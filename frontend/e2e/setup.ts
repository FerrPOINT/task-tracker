const API_BASE = process.env.VITE_API_BASE_URL ?? 'http://127.0.0.1:3456/api/v1'

// Single shared seed per test-run (process-wide). Parallel workers/projects reuse it,
// avoiding simultaneous /auth/register + /auth/login calls that trip the auth rate limiter.
type ApiContext = {
  token: string
  refreshToken: string
  userId: string
  projectId: string
  issueId: string
  issueKey: string
}

let seedPromise: Promise<ApiContext> | null = null

async function post(path: string, body: object, token?: string) {
  const res = await fetch(`${API_BASE}${path}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(body),
  })
  return { status: res.status, data: await res.json().catch(() => ({})) }
}

async function get(path: string, token: string) {
  const res = await fetch(`${API_BASE}${path}`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  return { status: res.status, data: await res.json().catch(() => ({})) }
}

async function fetchJsonWithRetry(url: string, init: RequestInit, attempts = 6) {
  for (let i = 0; i < attempts; i++) {
    const res = await fetch(url, init)
    if (res.status === 429) {
      await new Promise((r) => setTimeout(r, 15_000))
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
  const { access_token, refresh_token, user_id } = loginRes.data

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
    refreshToken: refresh_token,
    userId: user_id,
    projectId,
    issueId: issue.id,
    issueKey: issue.key,
  }
}

export async function seedIntegrationData(): Promise<ApiContext> {
  if (!seedPromise) {
    seedPromise = seed().catch((error) => {
      seedPromise = null // allow retry on next call after a failure
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
