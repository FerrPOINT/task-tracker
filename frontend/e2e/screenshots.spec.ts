import { test } from '@playwright/test'

const baseURL = 'http://localhost:4173'
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
  { path: '/projects/project-1/board', name: 'board', marker: 'TT Kanban' },
  { path: '/projects/project-1/backlog', name: 'backlog', marker: 'Backlog · Task Tracker' },
  { path: '/search', name: 'search', marker: 'Поиск задач' },
  { path: '/issues/create', name: 'issue-create', marker: 'Создать задачу' },
]

async function setThemeAndGoto(p: any, theme: 'light' | 'dark', path: string, marker: string) {
  await p.goto(`${baseURL}${path}`)
  await p.evaluate((t: 'light' | 'dark') => {
    window.localStorage.setItem('theme', t)
    document.documentElement.setAttribute('data-theme', t)
  }, theme)
  await p.reload()
  await p.waitForLoadState('networkidle')
  await p.waitForFunction(
    (text: string) => document.body.innerText.includes(text),
    marker,
    { timeout: 5000 }
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
