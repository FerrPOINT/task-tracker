const API_BASE = process.env.VITE_API_BASE_URL ?? 'http://127.0.0.1:3456/api/v1'

const credentials = {
  email: 'demo@example.com',
  password: 'demo',
  username: 'demo',
  name: 'Demo User',
}

export type ApiContext = {
  token: string
  refreshToken: string
  userId: string
  projectId: string
  issueId: string
  issueKey: string
}

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

export async function seedIntegrationData(): Promise<ApiContext> {
  const registerRes = await post('/auth/register', credentials)
  if (registerRes.status !== 201 && registerRes.status !== 409) {
    throw new Error(`register failed: ${registerRes.status} ${JSON.stringify(registerRes.data)}`)
  }

  const loginRes = await post('/auth/login', credentials)
  if (loginRes.status !== 200) {
    throw new Error(`login failed: ${loginRes.status} ${JSON.stringify(loginRes.data)}`)
  }
  const { access_token, refresh_token, user_id } = loginRes.data

  const projectsRes = await get('/projects', access_token)
  type ProjectItem = { id: string; key: string }
  let project = (projectsRes.data.projects as ProjectItem[] | undefined)?.find(
    (p) => p.key === 'DEMO',
  )
  if (!project) {
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
    project = createProjectRes.data as ProjectItem
  }

  const issuesRes = await get(`/projects/${project.id}/issues`, access_token)
  type IssueItem = { id: string; key: string; summary: string }
  let issue = (issuesRes.data.issues as IssueItem[] | undefined)?.find(
    (i) => i.summary === 'Smoke issue',
  )

  if (!issue) {
    const createIssueRes = await post(
      '/issues',
      {
        project_key: 'DEMO',
        issue_type: 'task',
        summary: 'Smoke issue',
        description: 'Created by E2E setup',
        priority: 'medium',
        status_id: '00000000-0000-0000-0000-000000000001',
        reporter_id: user_id,
      },
      access_token,
    )
    if (createIssueRes.status === 200 || createIssueRes.status === 201) {
      issue = createIssueRes.data as IssueItem
    } else {
      // Race: another parallel worker created the issue; re-fetch
      const retryRes = await get(`/projects/${project.id}/issues`, access_token)
      issue = (retryRes.data.issues as IssueItem[] | undefined)?.find(
        (i) => i.summary === 'Smoke issue',
      )
      if (!issue) {
        throw new Error(
          `create issue failed: ${createIssueRes.status} ${JSON.stringify(createIssueRes.data)}`,
        )
      }
    }
  }

  await post(
    `/issues/${issue.id}/worklogs`,
    {
      description: 'Initial work',
      started_at: new Date().toISOString(),
      duration_seconds: 180,
    },
    access_token,
  )

  return {
    token: access_token,
    refreshToken: refresh_token,
    userId: user_id,
    projectId: project.id,
    issueId: issue.id,
    issueKey: issue.key,
  }
}
