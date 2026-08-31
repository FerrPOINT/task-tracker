import { test, Page } from '@playwright/test'
import { seedIntegrationData, authenticatePage } from './setup'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'
const viewports = [
  { name: 'mobile', width: 375, height: 812 },
  { name: 'fullhd', width: 1920, height: 1080 },
  { name: '2k', width: 2560, height: 1440 },
]
const pages = [
  { path: '/login', name: 'login', marker: 'TaskTracker' },
  { path: '/register', name: 'register', marker: 'Зарегистрироваться' },
  { path: '/', name: 'dashboard', marker: 'Командный дашборд' },
  { path: '/projects', name: 'projects', marker: 'Проекты' },
  { path: '/projects/DEMO/board', name: 'board', marker: 'DEMO' },
  { path: '/projects/DEMO/backlog', name: 'backlog', marker: 'Backlog · DEMO' },
  { path: '/search', name: 'search', marker: 'Поиск задач' },
  { path: '/issues/create', name: 'issue-create', marker: 'Создать задачу' },
]

test.setTimeout(120_000)

test.beforeAll(async () => {
  await seedIntegrationData()
})

async function authenticate(p: Page) {
  await authenticatePage(p)
}

async function setThemeAndGoto(p: Page, theme: 'light' | 'dark', path: string, marker: string) {
  if (!['login', 'register'].includes(path.replace(/^\/?/, ''))) {
    await authenticate(p)
  }
  await p.goto(`${baseURL}/login`)
  await p.evaluate((t: 'light' | 'dark') => {
    window.localStorage.setItem('theme', t)
    document.documentElement.setAttribute('data-theme', t)
  }, theme)
  await p.goto(`${baseURL}${path}`)
  // SSE connection stays open forever, so networkidle never fires on authed pages.
  await p.waitForFunction((text: string) => document.body.innerText.includes(text), marker, {
    timeout: 10_000,
  })
  await p.waitForTimeout(300)
}

for (const page of pages) {
  test.describe(`${page.name}`, () => {
    for (const vp of viewports) {
      test(`${vp.name} light screenshot`, async ({ page: p }) => {
        await p.setViewportSize({ width: vp.width, height: vp.height })
        await setThemeAndGoto(p, 'light', page.path, page.marker)
        await p.screenshot({
          path: `/root/.hermes/cache/images/react-${page.name}-${vp.name}.png`,
          // Very long live pages (board/search on a narrow screen) can exceed
          // the browser 32,767px full-page capture limit; those stay
          // viewport-sized, everything else is captured full-page.
          fullPage: !((page.name === 'board' || page.name === 'search') && vp.name === 'mobile'),
          scale: 'css',
        })
      })
    }

    test('fullhd dark screenshot', async ({ page: p }) => {
      await p.setViewportSize({ width: 1920, height: 1080 })
      await setThemeAndGoto(p, 'dark', page.path, page.marker)
      await p.screenshot({
        path: `/root/.hermes/cache/images/react-${page.name}-dark.png`,
        fullPage: true,
        scale: 'css',
      })
    })
  })
}
