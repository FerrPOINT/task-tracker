import { test } from '@playwright/test'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'
const viewports = [
  { name: 'mobile', width: 375, height: 812 },
  { name: 'fullhd', width: 1920, height: 1080 },
  { name: '2k', width: 2560, height: 1440 },
]
const pages = [
  { path: '/login', name: 'login', marker: 'TaskTracker' },
  { path: '/register', name: 'register', marker: 'Зарегистрироваться' },
  { path: '/', name: 'dashboard', marker: 'Team Dashboard' },
  { path: '/projects', name: 'projects', marker: 'Проекты' },
  { path: '/projects/TT/board', name: 'board', marker: 'TT Kanban' },
  { path: '/projects/TT/backlog', name: 'backlog', marker: 'Backlog · TT' },
  { path: '/search', name: 'search', marker: 'Поиск задач' },
  { path: '/issues/create', name: 'issue-create', marker: 'Создать задачу' },
]

async function authenticate(p: any) {
  const res = await p.request.post(`${baseURL}/api/v1/auth/login`, {
    data: { email: 'demo@example.com', password: 'demo' },
  })
  if (res.status() !== 200) throw new Error('screenshot auth failed')
  const { access_token, user_id, email } = await res.json()
  await p.evaluate(
    (payload: { token: string; userId: string; email: string }) => {
      window.localStorage.setItem('task-tracker-auth', JSON.stringify(payload))
    },
    { token: access_token, userId: user_id, email },
  )
}

async function setThemeAndGoto(p: any, theme: 'light' | 'dark', path: string, marker: string) {
  await p.goto(`${baseURL}/login`)
  if (!['login', 'register'].includes(path.replace(/^\/?/, ''))) {
    await authenticate(p)
    await p.goto(`${baseURL}${path}`)
  } else {
    await p.goto(`${baseURL}${path}`)
  }
  await p.evaluate((t: 'light' | 'dark') => {
    window.localStorage.setItem('theme', t)
    document.documentElement.setAttribute('data-theme', t)
  }, theme)
  await p.reload()
  await p.waitForLoadState('networkidle')
  await p.waitForFunction(
    (text: string) => document.body.innerText.includes(text),
    marker,
    { timeout: 5000 },
  )
}

for (const page of pages) {
  test.describe(`${page.name}`, () => {
    for (const vp of viewports) {
      test(`${vp.name} light screenshot`, async ({ page: p }) => {
        await p.setViewportSize({ width: vp.width, height: vp.height })
        await setThemeAndGoto(p, 'light', page.path, page.marker)
        await p.screenshot({
          path: `/root/.hermes/cache/images/react-${page.name}-${vp.name}.png`,
          fullPage: true,
        })
      })
    }

    test('fullhd dark screenshot', async ({ page: p }) => {
      await p.setViewportSize({ width: 1920, height: 1080 })
      await setThemeAndGoto(p, 'dark', page.path, page.marker)
      await p.screenshot({
        path: `/root/.hermes/cache/images/react-${page.name}-dark.png`,
        fullPage: true,
      })
    })
  })
}
