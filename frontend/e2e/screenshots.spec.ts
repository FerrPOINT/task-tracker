import { expect, test, Page } from '@playwright/test'
import { seedIntegrationData, authenticatePage } from './setup'

const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? 'http://localhost:4173'
const viewports = [
  { name: 'mobile', width: 375, height: 812 },
  { name: 'tablet', width: 768, height: 1024 },
  { name: 'desktop', width: 1280, height: 800 },
  { name: 'fullhd', width: 1920, height: 1080 },
]
const themes = ['light', 'gray', 'dark'] as const
const pages = [
  { path: '/login', name: 'login', marker: 'Войти' },
  { path: '/register', name: 'register', marker: 'Зарегистрироваться' },
  { path: '/', name: 'dashboard', marker: 'Командный дашборд' },
  { path: '/projects', name: 'projects', marker: 'Проекты' },
  { path: '/projects/DEMO/board', name: 'board', marker: 'DEMO' },
  { path: '/projects/DEMO/backlog', name: 'backlog', marker: 'Бэклог · DEMO' },
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

async function setThemeAndGoto(
  p: Page,
  theme: (typeof themes)[number],
  path: string,
  marker: string,
) {
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

async function expectNoUnexpectedOverflow(page: Page) {
  const result = await page.evaluate(() => {
    const rootOverflow = document.documentElement.scrollWidth - document.documentElement.clientWidth
    const nested = [...document.querySelectorAll<HTMLElement>('body *')]
      .filter((element) => {
        if (element.matches('textarea, select, [data-scroll-allowed="true"]')) return false
        const style = getComputedStyle(element)
        const scrollsX =
          /(auto|scroll)/.test(style.overflowX) && element.scrollWidth > element.clientWidth + 1
        const scrollsY =
          /(auto|scroll)/.test(style.overflowY) && element.scrollHeight > element.clientHeight + 1
        return scrollsX || scrollsY
      })
      .map((element) => element.outerHTML.slice(0, 160))
    return { rootOverflow, nested }
  })
  expect(result.rootOverflow).toBeLessThanOrEqual(1)
  expect(result.nested, `Unexpected nested scrollers:\n${result.nested.join('\n')}`).toEqual([])
}

for (const page of pages) {
  test.describe(`${page.name}`, () => {
    for (const vp of viewports) {
      for (const theme of themes) {
        test(`${vp.name} ${theme} screenshot`, async ({ page: p }, testInfo) => {
          await p.setViewportSize({ width: vp.width, height: vp.height })
          await setThemeAndGoto(p, theme, page.path, page.marker)
          await expectNoUnexpectedOverflow(p)
          await p.screenshot({
            path: testInfo.outputPath(`${page.name}-${vp.name}-${theme}.png`),
            fullPage: true,
            scale: 'css',
          })
        })
      }
    }
  })
}
