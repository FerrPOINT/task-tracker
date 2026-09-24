import { mkdirSync, readFileSync } from 'node:fs'
import { Buffer } from 'node:buffer'
import { fileURLToPath } from 'node:url'
import { expect, test, type APIRequestContext, type Page } from '@playwright/test'
import { signInAt } from './qa-login'

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
        runId: string
      })
    : { email: '', password: '', runId: '' }

async function centralHeaders(request: APIRequestContext) {
  const login = await request.post('http://localhost:7701/auth/login', {
    data: { email: account.email, password: account.password },
  })
  expect(login.ok()).toBeTruthy()
  const { access_token } = (await login.json()) as { access_token: string }
  return { Authorization: `Bearer ${access_token}` }
}

async function enter(page: Page, url: string) {
  await signInAt(page, url, account)
}

test('Task Tracker creates and edits a project, issue, sprint and report', async ({
  page,
  request,
}) => {
  test.setTimeout(120_000)
  const headers = await centralHeaders(request)
  const api = 'http://localhost:7721/api/v1'
  const key = `Q${Date.now().toString(36).slice(-7).toUpperCase()}`
  const name = `QA ${account.runId} ${key}`
  let created = false
  try {
    const project = await request.post(`${api}/projects`, {
      headers,
      data: { key, name, description: 'QA SSO smoke' },
    })
    expect(project.ok(), await project.text()).toBeTruthy()
    created = true
    const issue = await request.post(`${api}/issues`, {
      headers,
      data: {
        project_key: key,
        issue_type: 'Task',
        summary: `${name} issue`,
        priority: 'Medium',
      },
    })
    expect(issue.ok(), await issue.text()).toBeTruthy()
    const data = (await issue.json()) as { id: string; key: string }

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
      headers,
      data: { name: `${name} sprint`, goal: 'QA report' },
    })
    expect(sprint.ok(), await sprint.text()).toBeTruthy()
    const sprintId = ((await sprint.json()) as { id: string }).id
    const assigned = await request.post(`${api}/projects/${key}/sprints/${sprintId}/issues`, {
      headers,
      data: { issue_id: data.id },
    })
    expect(assigned.ok(), await assigned.text()).toBeTruthy()
    const started = await request.post(`${api}/projects/${key}/sprints/${sprintId}/start`, {
      headers,
    })
    expect(started.ok(), await started.text()).toBeTruthy()
    const report = await request.get(`${api}/reports/burndown?sprint_id=${sprintId}`, { headers })
    expect(report.ok(), await report.text()).toBeTruthy()
    expect(((await report.json()) as { sprint_name: string }).sprint_name).toBe(`${name} sprint`)
    const closed = await request.post(`${api}/projects/${key}/sprints/${sprintId}/close`, {
      headers,
    })
    expect(closed.ok(), await closed.text()).toBeTruthy()
  } finally {
    if (created) {
      const removed = await request.delete(`${api}/projects/${key}`, { headers })
      expect(removed.ok(), await removed.text()).toBeTruthy()
    }
  }
})

test('Task Tracker manages issue labels, links, attachments and worklogs', async ({
  page,
  request,
}, testInfo) => {
  test.setTimeout(120_000)
  const headers = await centralHeaders(request)
  const api = 'http://localhost:7721/api/v1'
  const key = `Q${Date.now().toString(36).slice(-7).toUpperCase()}`
  const name = `QA ${account.runId} detail ${key}`
  let created = false
  try {
    const project = await request.post(`${api}/projects`, {
      headers,
      data: { key, name, description: 'QA issue detail smoke' },
    })
    expect(project.ok(), await project.text()).toBeTruthy()
    created = true

    const issues = [] as { id: string; key: string }[]
    for (const summary of [`${name} source`, `${name} target`]) {
      const response = await request.post(`${api}/issues`, {
        headers,
        data: { project_key: key, issue_type: 'Task', summary, priority: 'Medium' },
      })
      expect(response.ok(), await response.text()).toBeTruthy()
      issues.push((await response.json()) as { id: string; key: string })
    }
    const [source, target] = issues
    expect(source).toBeDefined()
    expect(target).toBeDefined()

    await enter(page, `http://localhost:7722/issues/${source!.id}`)
    await expect(page.getByRole('heading', { name: `${name} source` })).toBeVisible()

    const labels = page.getByTestId('label-editor')
    await labels.getByRole('button', { name: 'Новая метка' }).click()
    await labels.getByRole('textbox', { name: 'Название метки' }).fill(`qa-${account.runId}`)
    await labels.getByRole('button', { name: 'Добавить' }).click()
    await expect(labels.getByTestId('issue-label')).toContainText(`qa-${account.runId}`)

    const links = page.getByTestId('link-editor')
    await links.getByRole('button', { name: 'Добавить связь' }).click()
    const invalidKey = `${key}-9999`
    await links.getByRole('textbox', { name: 'Ключ задачи' }).fill(invalidKey)
    await links.getByTestId('link-submit').click()
    await expect(links.getByRole('alert')).toContainText(invalidKey)
    await expect(links.getByRole('textbox', { name: 'Ключ задачи' })).toHaveValue(invalidKey)
    await links.getByRole('textbox', { name: 'Ключ задачи' }).fill(target!.key)
    await links.getByTestId('link-submit').click()
    await expect(links.getByRole('link', { name: target!.key })).toBeVisible()

    await page.getByRole('tab', { name: 'Вложения' }).click()
    const attachments = page.getByTestId('attachment-panel')
    const fileA = `qa-${account.runId}-one.txt`
    const fileB = `qa-${account.runId}-two.txt`
    await attachments.getByTestId('attachment-input').setInputFiles([
      { name: fileA, mimeType: 'text/plain', buffer: Buffer.from('QA first attachment') },
      { name: fileB, mimeType: 'text/plain', buffer: Buffer.from('QA second attachment') },
    ])
    await expect(attachments.getByTestId('attachment-row')).toHaveCount(2)
    const downloadStarted = page.waitForEvent('download')
    await attachments.getByRole('button', { name: `Скачать ${fileA}` }).click()
    expect((await downloadStarted).suggestedFilename()).toBe(fileA)
    await attachments.getByRole('button', { name: `Удалить ${fileA}` }).click()
    await page.getByRole('alertdialog').getByRole('button', { name: 'Подтвердить' }).click()
    await expect(attachments.getByTestId('attachment-row')).toHaveCount(1)

    await page.getByRole('button', { name: 'Записать время' }).click()
    const workDialog = page.getByRole('dialog')
    await workDialog.getByRole('button', { name: 'Запустить таймер' }).click()
    await expect(workDialog.getByText(/^[1-9]\d*s$/)).toBeVisible({ timeout: 5_000 })
    await workDialog.getByRole('button', { name: 'Остановить таймер' }).click()
    await expect(workDialog.getByRole('textbox', { name: 'Затрачено времени' })).not.toBeEmpty()
    await workDialog.getByRole('textbox', { name: 'Затрачено времени' }).fill('1m')
    const comment = `QA ${account.runId} worklog`
    await workDialog.getByRole('textbox', { name: 'Комментарий' }).fill(comment)
    await workDialog.getByRole('button', { name: 'Сохранить' }).click()
    await expect(workDialog).toBeHidden()
    await page.getByRole('tab', { name: 'Журнал работ' }).click()
    await expect(page.getByText(comment, { exact: true }).filter({ visible: true })).toBeVisible()
    await expect(page.locator('[data-sonner-toast]')).toHaveCount(0, { timeout: 10_000 })

    for (const [width, height] of [
      [375, 812],
      [768, 1024],
      [1280, 800],
      [1920, 1080],
      [2560, 1440],
    ] as const) {
      await page.setViewportSize({ width, height })
      await expect(page.getByText(comment, { exact: true }).filter({ visible: true })).toBeVisible()
      const scroll = await page.evaluate(() => ({
        width: document.documentElement.clientWidth,
        content: document.documentElement.scrollWidth,
      }))
      expect(scroll.content, `issue detail overflow at ${width}px`).toBeLessThanOrEqual(
        scroll.width,
      )
      for (const [name, button] of [
        ['label', labels.getByRole('button', { name: `Убрать метку qa-${account.runId}` })],
        ['link', links.getByRole('button', { name: `Удалить связь с ${target!.key}` })],
        [
          'worklog',
          page.getByRole('button', { name: 'Изменить запись' }).filter({ visible: true }),
        ],
      ] as const) {
        const box = await button.boundingBox()
        expect(box?.width, `${name} touch width at ${width}px`).toBeGreaterThanOrEqual(40)
        expect(box?.height, `${name} touch height at ${width}px`).toBeGreaterThanOrEqual(40)
      }
      if (width === 375 || width === 1920 || width === 2560) {
        await page.screenshot({
          path: testInfo.outputPath(`issue-detail-${width}.png`),
          fullPage: true,
        })
      }
    }

    await page.getByRole('button', { name: 'Изменить запись' }).filter({ visible: true }).click()
    const editDialog = page.getByRole('dialog')
    const revisedComment = `${comment} revised`
    await editDialog.getByRole('textbox', { name: 'Комментарий' }).fill(revisedComment)
    await editDialog.getByRole('button', { name: 'Сохранить' }).click()
    await expect(editDialog).toBeHidden()
    await expect(
      page.getByText(revisedComment, { exact: true }).filter({ visible: true }),
    ).toBeVisible()
    await page.getByRole('button', { name: 'Удалить запись' }).filter({ visible: true }).click()
    await page.getByRole('alertdialog').getByRole('button', { name: 'Подтвердить' }).click()
    await expect(page.getByText('Пока нет записей.')).toBeVisible()

    await page.getByRole('tab', { name: 'Комментарии' }).click()
    const commentBody = `QA ${account.runId} comment`
    await page.getByRole('textbox', { name: 'Комментарий' }).fill(commentBody)
    await page.getByRole('button', { name: 'Добавить комментарий' }).click()
    await expect(page.getByText(commentBody, { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Изменить', exact: true }).last().click()
    await page.getByRole('textbox', { name: 'Комментарий' }).fill(`${commentBody} revised`)
    await page.getByRole('button', { name: 'Сохранить' }).click()
    await expect(page.getByText(`${commentBody} revised`, { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Удалить', exact: true }).click()
    await page.getByRole('alertdialog').getByRole('button', { name: 'Подтвердить' }).click()
    await expect(page.getByText('Пока нет комментариев.')).toBeVisible()
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
  let documentArchived = false
  try {
    const space = await request.post(`${api}/spaces`, {
      headers,
      data: { key, name: `QA ${key}`, description: 'SSO smoke' },
    })
    expect(space.ok(), await space.text()).toBeTruthy()
    spaceCreated = true
    const draft = await request.post(`${api}/spaces/${key}/documents`, {
      headers,
      data: {
        title: `QA ${key} document`,
        content_markdown: '# First revision',
      },
    })
    expect(draft.ok(), await draft.text()).toBeTruthy()
    documentId = ((await draft.json()) as { id: string }).id
    const published = await request.post(`${api}/documents/${documentId}/publish`, {
      headers,
      data: { summary: 'QA publication' },
    })
    expect(published.ok(), await published.text()).toBeTruthy()
    const revision = ((await published.json()) as { id: string }).id
    const edited = await request.put(`${api}/documents/${documentId}/draft`, {
      headers,
      data: {
        title: `QA ${key} revised`,
        content_markdown: '# Revised QA content',
      },
    })
    expect(edited.ok(), await edited.text()).toBeTruthy()
    const republished = await request.post(`${api}/documents/${documentId}/publish`, {
      headers,
      data: { base_revision_id: revision, summary: 'QA revision' },
    })
    expect(republished.ok(), await republished.text()).toBeTruthy()
    await enter(page, 'http://localhost:7732/spaces')
    await page.getByRole('searchbox', { name: 'Найти пространство' }).fill(key)
    await page.getByRole('button', { name: new RegExp(`QA ${key}`) }).click()
    await page.getByRole('link', { name: `QA ${key} revised` }).click()
    await expect(page.getByRole('heading', { name: 'Revised QA content' })).toBeVisible()
    await page.getByRole('button', { name: 'Правка' }).click()
    await page.getByRole('button', { name: 'Архивировать' }).click()
    const archiveDialog = page.getByRole('alertdialog')
    await expect(archiveDialog).toBeVisible()
    const screenshotDir = fileURLToPath(
      new URL('../../../.local/screenshots/wiki-archive-confirm/', import.meta.url),
    )
    mkdirSync(screenshotDir, { recursive: true })
    for (const [width, height] of [
      [2560, 1440],
      [1920, 1080],
      [375, 812],
    ] as const) {
      await page.setViewportSize({ width, height })
      const bounds = await archiveDialog.boundingBox()
      expect(bounds, `archive dialog bounds at ${width}px`).not.toBeNull()
      expect(bounds!.x).toBeGreaterThanOrEqual(0)
      expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(width)
      expect(bounds!.y).toBeGreaterThanOrEqual(0)
      expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(height)
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
        ),
      ).toBeLessThanOrEqual(1)
      await page.screenshot({
        path: `${screenshotDir}/${width}.png`,
        fullPage: true,
      })
    }
    await archiveDialog.getByRole('button', { name: 'Подтвердить' }).click()
    await expect(archiveDialog).toBeHidden()
    await expect(page.getByText('Документ архивирован')).toBeVisible()
    await expect(page.getByRole('button', { name: 'Архивировать' })).toHaveCount(0)
    const archivedDocument = await request.get(`${api}/documents/${documentId}`, { headers })
    expect(archivedDocument.ok(), await archivedDocument.text()).toBeTruthy()
    expect(((await archivedDocument.json()) as { status: string }).status).toBe('archived')
    documentArchived = true
  } finally {
    if (documentId && !documentArchived) {
      await request.post(`${api}/documents/${documentId}/archive`, { headers })
    }
    if (spaceCreated) await request.post(`${api}/spaces/${key}/archive`, { headers })
  }
})

test('Wiki user directory is read-only and links to central management', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await enter(page, 'http://localhost:7732/users')
  await expect(page.getByRole('heading', { name: 'Профили Wiki' })).toBeVisible()
  const manage = page.getByRole('link', { name: 'Учётные записи' })
  await expect(manage).toBeVisible()
  expect(await manage.getAttribute('href')).toMatch(/:7772\/users$/)
  await expect(page.getByRole('textbox', { name: 'Поиск пользователей' })).toBeVisible()
  await expect(page.getByLabel(/пароль|роль/i)).toHaveCount(0)
  await expect(
    page.getByRole('button', { name: /создать пользователя|сохранить роль/i }),
  ).toHaveCount(0)
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  )
  expect(overflow).toBeLessThanOrEqual(1)
  await page.setViewportSize({ width: 1280, height: 800 })
  await expect(manage).toBeVisible()
  const desktopOverflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  )
  expect(desktopOverflow).toBeLessThanOrEqual(1)
})

test('CI/CD creates, edits and removes a QA project in the UI', async ({ page, request }) => {
  test.setTimeout(120_000)
  const headers = await centralHeaders(request)
  const api = 'http://localhost:7711/api/v1'
  const name = `QA ${account.runId} CI ${Date.now()}`
  const updatedName = `${name} updated`
  let projectId = ''
  try {
    await page.setViewportSize({ width: 375, height: 812 })
    await enter(page, 'http://localhost:7712/projects')
    await page.getByRole('button', { name: 'Создать проект' }).click()
    const createForm = page.getByRole('form', { name: 'Создать проект' })
    await createForm.getByLabel('Название').fill(name)
    await createForm.getByLabel('URL репозитория').fill('https://example.test/qa.git')
    const created = page.waitForResponse(
      (response) =>
        response.url().endsWith('/api/v1/projects') && response.request().method() === 'POST',
    )
    await createForm.getByRole('button', { name: 'Создать проект' }).click()
    const createdResponse = await created
    expect(createdResponse.ok(), await createdResponse.text()).toBeTruthy()
    projectId = ((await createdResponse.json()) as { id: string }).id
    await expect(createForm).toBeHidden()

    await page.getByRole('searchbox', { name: 'Найти проект' }).fill(name)
    await expect(
      page.getByRole('link', { name: `Открыть пайплайны проекта ${name}` }),
    ).toBeVisible()
    await page.getByRole('button', { name: `Действия с проектом ${name}` }).click()
    await page.getByRole('menuitem', { name: 'Изменить' }).click()
    const editForm = page.getByRole('form', { name: `Изменить проект ${name}` })
    await editForm.getByLabel('Название').fill(updatedName)
    const saved = page.waitForResponse(
      (response) =>
        response.url().endsWith(`/api/v1/projects/${projectId}`) &&
        response.request().method() === 'PATCH',
    )
    await editForm.getByRole('button', { name: 'Сохранить' }).click()
    expect((await saved).ok()).toBeTruthy()
    const stored = await request.get(`${api}/projects/${projectId}`, { headers })
    expect(stored.ok()).toBeTruthy()
    expect(((await stored.json()) as { name: string }).name).toBe(updatedName)

    await page.getByRole('searchbox', { name: 'Найти проект' }).fill(updatedName)
    await page.getByRole('button', { name: `Действия с проектом ${updatedName}` }).click()
    await page.getByRole('menuitem', { name: 'Удалить' }).click()
    const dialog = page.getByRole('alertdialog')
    await expect(dialog).toContainText(updatedName)
    const removed = page.waitForResponse(
      (response) =>
        response.url().endsWith(`/api/v1/projects/${projectId}`) &&
        response.request().method() === 'DELETE',
    )
    await dialog.getByRole('button', { name: 'Удалить' }).click()
    expect((await removed).ok()).toBeTruthy()
    await expect(dialog).toBeHidden()
    expect((await request.get(`${api}/projects/${projectId}`, { headers })).status()).toBe(404)
  } finally {
    if (projectId) {
      const existing = await request.get(`${api}/projects/${projectId}`, { headers })
      if (existing.ok()) {
        const current = (await existing.json()) as { name: string }
        expect([name, updatedName]).toContain(current.name)
        const cleanup = await request.delete(`${api}/projects/${projectId}`, { headers })
        expect(cleanup.ok(), await cleanup.text()).toBeTruthy()
      }
    }
  }
})

test('Fleet Control manages a QA agent without starting it', async ({ page, request }) => {
  const headers = await centralHeaders(request)
  const api = 'http://localhost:7741/api/v1'
  const name = `QA ${account.runId} ${Date.now()}`
  let agentId = ''
  try {
    const created = await request.post(`${api}/agents`, {
      headers,
      data: {
        kind: 'hermes',
        product_role: 'executor',
        role: 'developer',
        display_name: name,
        description: 'SSO QA; not started',
      },
    })
    expect(created.ok(), await created.text()).toBeTruthy()
    agentId = ((await created.json()) as { id: string }).id
    const edited = await request.patch(`${api}/agents/${agentId}`, {
      headers,
      data: {
        display_name: `${name} updated`,
        description: 'QA management verified',
      },
    })
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
  const manage = page.getByRole('link', { name: /Admin Panel/ })
  await expect(manage).toBeVisible()
  expect(await manage.getAttribute('href')).toMatch(/:7772\/users$/)
  await expect(
    page.locator('select[aria-label^="Role for"], select[aria-label^="Роль для"]'),
  ).toHaveCount(0)
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  )
  expect(overflow).toBeLessThanOrEqual(1)
  await page.setViewportSize({ width: 1280, height: 800 })
  await expect(manage).toBeVisible()
})
