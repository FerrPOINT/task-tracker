import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { expect, test, type APIRequestContext, type Page } from '@playwright/test'
import { signInAt } from './qa-login'

test.skip(process.env.SDLC_LIVE_QA !== '1', 'Requires the running local SDLC fleet')
test.skip(({ browserName }) => browserName !== 'chromium', 'Single browser live smoke')
test.use({ trace: 'off' })

const account = process.env.SDLC_LIVE_QA === '1'
  ? JSON.parse(readFileSync(fileURLToPath(new URL('../../../.local/qa-session.json', import.meta.url)), 'utf8')) as {
      email: string; password: string; runId: string
    }
  : { email: '', password: '', runId: '' }

async function centralHeaders(request: APIRequestContext) {
  const login = await request.post('http://localhost:7701/auth/login', {
    data: { email: account.email, password: account.password },
  })
  expect(login.ok()).toBeTruthy()
  const { access_token } = await login.json() as { access_token: string }
  return { Authorization: `Bearer ${access_token}` }
}

async function enter(page: Page, url: string) {
  await signInAt(page, url, account)
}

test('Task Tracker creates and edits a project, issue, sprint and report', async ({ page, request }) => {
  test.setTimeout(120_000)
  const headers = await centralHeaders(request)
  const api = 'http://localhost:7721/api/v1'
  const key = `Q${Date.now().toString(36).slice(-7).toUpperCase()}`
  const name = `QA ${account.runId} ${key}`
  let created = false
  try {
    const project = await request.post(`${api}/projects`, { headers, data: { key, name, description: 'QA SSO smoke' } })
    expect(project.ok(), await project.text()).toBeTruthy()
    created = true
    const issue = await request.post(`${api}/issues`, { headers, data: {
      project_key: key, issue_type: 'Task', summary: `${name} issue`, priority: 'Medium',
    } })
    expect(issue.ok(), await issue.text()).toBeTruthy()
    const data = await issue.json() as { id: string; key: string }

    await enter(page, `http://localhost:7722/projects/${key}/board`)
    const card = page.locator('article').filter({ hasText: `${name} issue` })
    await expect(card).toBeVisible()
    await card.getByRole('button', { name: /Изменить статус/ }).click()
    await page.getByRole('menuitem', { name: /В работе/ }).click()
    await card.locator('a[href^="/issues/"]').first().click()
    await expect(page.locator('#issue-status option:checked')).toHaveText('В работе')
    await page.getByRole('button', { name: 'Изменить', exact: true }).first().click()
    await page.getByRole('textbox', { name: 'Заголовок' }).fill(`${name} revised`)
    await page.getByRole('button', { name: 'Сохранить' }).click()
    await expect(page.getByRole('heading', { name: `${name} revised` })).toBeVisible()

    const sprint = await request.post(`${api}/projects/${key}/sprints`, {
      headers, data: { name: `${name} sprint`, goal: 'QA report' },
    })
    expect(sprint.ok(), await sprint.text()).toBeTruthy()
    const sprintId = (await sprint.json() as { id: string }).id
    const assigned = await request.post(`${api}/projects/${key}/sprints/${sprintId}/issues`, {
      headers, data: { issue_id: data.id },
    })
    expect(assigned.ok(), await assigned.text()).toBeTruthy()
    const started = await request.post(`${api}/projects/${key}/sprints/${sprintId}/start`, { headers })
    expect(started.ok(), await started.text()).toBeTruthy()
    const report = await request.get(`${api}/reports/burndown?sprint_id=${sprintId}`, { headers })
    expect(report.ok(), await report.text()).toBeTruthy()
    expect((await report.json() as { sprint_name: string }).sprint_name).toBe(`${name} sprint`)
    const closed = await request.post(`${api}/projects/${key}/sprints/${sprintId}/close`, { headers })
    expect(closed.ok(), await closed.text()).toBeTruthy()
  } finally {
    if (created) {
      const removed = await request.delete(`${api}/projects/${key}`, { headers })
      expect(removed.ok(), await removed.text()).toBeTruthy()
    }
  }
})

test('Wiki publishes and revises a page', async ({ page, request }) => {
  test.setTimeout(90_000)
  const headers = await centralHeaders(request)
  const api = 'http://localhost:7731/api/v1'
  const key = `QA${Date.now().toString(36).slice(-7).toUpperCase()}`
  let spaceCreated = false
  let documentId = ''
  try {
    const space = await request.post(`${api}/spaces`, { headers, data: { key, name: `QA ${key}`, description: 'SSO smoke' } })
    expect(space.ok(), await space.text()).toBeTruthy()
    spaceCreated = true
    const draft = await request.post(`${api}/spaces/${key}/documents`, { headers, data: {
      title: `QA ${key} document`, content_markdown: '# First revision',
    } })
    expect(draft.ok(), await draft.text()).toBeTruthy()
    documentId = (await draft.json() as { id: string }).id
    const published = await request.post(`${api}/documents/${documentId}/publish`, {
      headers, data: { summary: 'QA publication' },
    })
    expect(published.ok(), await published.text()).toBeTruthy()
    const revision = (await published.json() as { id: string }).id
    const edited = await request.put(`${api}/documents/${documentId}/draft`, { headers, data: {
      title: `QA ${key} revised`, content_markdown: '# Revised QA content',
    } })
    expect(edited.ok(), await edited.text()).toBeTruthy()
    const republished = await request.post(`${api}/documents/${documentId}/publish`, {
      headers, data: { base_revision_id: revision, summary: 'QA revision' },
    })
    expect(republished.ok(), await republished.text()).toBeTruthy()
    await enter(page, 'http://localhost:7732/spaces')
    await page.getByRole('link', { name: `QA ${key} revised` }).click()
    await expect(page.getByRole('heading', { name: 'Revised QA content' })).toBeVisible()
  } finally {
    if (documentId) await request.post(`${api}/documents/${documentId}/archive`, { headers })
    if (spaceCreated) await request.post(`${api}/spaces/${key}/archive`, { headers })
  }
})

test('Wiki user directory is read-only and links to central management', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await enter(page, 'http://localhost:7732/users')
  await expect(page.getByRole('heading', { name: 'Пользователи' })).toBeVisible()
  const manage = page.getByRole('link', { name: 'Управление пользователями' })
  await expect(manage).toBeVisible()
  expect(await manage.getAttribute('href')).toMatch(/:7772\/users$/)
  await expect(page.getByRole('textbox', { name: 'Поиск пользователей' })).toBeVisible()
  await expect(page.getByLabel(/пароль|роль/i)).toHaveCount(0)
  await expect(page.getByRole('button', { name: /создать пользователя|сохранить роль/i })).toHaveCount(0)
  const overflow = await page.evaluate(() =>
    document.documentElement.scrollWidth - document.documentElement.clientWidth,
  )
  expect(overflow).toBeLessThanOrEqual(1)
  await page.setViewportSize({ width: 1280, height: 800 })
  await expect(manage).toBeVisible()
  const desktopOverflow = await page.evaluate(() =>
    document.documentElement.scrollWidth - document.documentElement.clientWidth,
  )
  expect(desktopOverflow).toBeLessThanOrEqual(1)
})

test('Fleet Control manages a QA agent without starting it', async ({ page, request }) => {
  const headers = await centralHeaders(request)
  const api = 'http://localhost:7741/api/v1'
  const name = `QA ${account.runId} ${Date.now()}`
  let agentId = ''
  try {
    const created = await request.post(`${api}/agents`, { headers, data: {
      kind: 'hermes', product_role: 'executor', role: 'developer', display_name: name,
      description: 'SSO QA; not started',
    } })
    expect(created.ok(), await created.text()).toBeTruthy()
    agentId = (await created.json() as { id: string }).id
    const edited = await request.patch(`${api}/agents/${agentId}`, { headers, data: {
      display_name: `${name} updated`, description: 'QA management verified',
    } })
    expect(edited.ok(), await edited.text()).toBeTruthy()
    await enter(page, 'http://localhost:7742/agents')
    await expect(page.getByText(`${name} updated`).first()).toBeVisible()
  } finally {
    if (agentId) {
      const removed = await request.delete(`${api}/agents/${agentId}`, { headers })
      expect(removed.ok(), await removed.text()).toBeTruthy()
    }
  }
})

test('Fleet user settings are read-only in central mode', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await enter(page, 'http://localhost:7742/settings?tab=users')
  const manage = page.getByRole('link', { name: 'Manage in Admin Panel' })
  await expect(manage).toBeVisible()
  expect(await manage.getAttribute('href')).toMatch(/:7772\/users$/)
  await expect(page.locator('select[aria-label^="Role for"]')).toHaveCount(0)
  const overflow = await page.evaluate(() =>
    document.documentElement.scrollWidth - document.documentElement.clientWidth,
  )
  expect(overflow).toBeLessThanOrEqual(1)
  await page.setViewportSize({ width: 1280, height: 800 })
  await expect(manage).toBeVisible()
})
