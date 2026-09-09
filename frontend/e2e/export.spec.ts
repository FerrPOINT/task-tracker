import { expect, test } from '@playwright/test'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'
const apiBaseUrl = process.env.VITE_API_BASE_URL ?? 'http://127.0.0.1:3456/api/v1'

test.describe('issue export against live backend', () => {
  test.setTimeout(120_000)

  test('downloads CSV and JSON for the selected project', async ({ page, request }) => {
    const credentials = {
      email: `export-${Date.now()}@example.test`,
      password: 'ExportPass1!',
      username: `export${Date.now()}`,
      name: 'Export E2E User',
    }
    const register = await request.post(`${apiBaseUrl}/auth/register`, { data: credentials })
    expect(register.status()).toBe(201)

    const login = await request.post(`${apiBaseUrl}/auth/login`, {
      data: { email: credentials.email, password: credentials.password },
    })
    expect(login.status()).toBe(200)
    const auth = (await login.json()) as { access_token: string; user_id: string }
    const headers = { Authorization: `Bearer ${auth.access_token}` }

    const projectKey = `E${Date.now().toString().slice(-9)}`
    const project = await request.post(`${apiBaseUrl}/projects`, {
      data: {
        key: projectKey,
        name: 'Export E2E Project',
        description: 'Playwright export coverage',
      },
      headers,
    })
    expect(project.status()).toBe(201)
    const projectBody = (await project.json()) as { id: string }

    const issue = await request.post(`${apiBaseUrl}/issues`, {
      data: {
        project_key: projectKey,
        issue_type: 'task',
        priority: 'medium',
        reporter_id: auth.user_id,
        summary: 'Exportable issue',
      },
      headers,
    })
    expect(issue.status()).toBe(201)

    await page.goto(`${baseURL}/login`)
    await page.getByRole('textbox').nth(0).fill(credentials.email)
    await page.getByRole('textbox').nth(1).fill(credentials.password)
    await page.getByRole('button', { name: /войти|login/i }).click()
    await page.waitForURL((url) => !url.pathname.includes('/login'))
    await page.goto(`${baseURL}/reports`)
    await page.getByLabel(/проект|project/i).selectOption(projectBody.id)

    for (const format of ['CSV', 'JSON']) {
      const [download] = await Promise.all([
        page.waitForEvent('download'),
        page.getByRole('button', { name: format, exact: true }).click(),
      ])
      expect(download.suggestedFilename()).toBe(`issues.${format.toLowerCase()}`)
    }
  })
})
