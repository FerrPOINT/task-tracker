import { chromium } from '@playwright/test'

const url = 'http://localhost:4173/issues/issue-1'
const themes = [
  { name: 'dark', label: 'issue-detail-dark' },
  { name: 'gray', label: 'issue-detail-gray' },
  { name: 'light', label: 'issue-detail-light' },
]

async function capture(viewport, name) {
  const browser = await chromium.launch()
  const page = await browser.newPage({ viewport })
  await page.goto(url)
  await page.getByText('Учёт времени').waitFor({ state: 'visible' })
  await page.screenshot({ path: `/root/.hermes/cache/images/${name}.png`, fullPage: true })
  await browser.close()
  console.log(`saved ${name}.png`)
}

async function themedScreenshots() {
  for (const theme of themes) {
    const browser = await chromium.launch()
    const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } })
    await page.goto(url)
    await page.evaluate((t) => {
      document.documentElement.setAttribute('data-theme', t)
      window.localStorage.setItem('theme', t)
    }, theme.name)
    await page.getByText('Учёт времени').waitFor({ state: 'visible' })
    await page.screenshot({ path: `/root/.hermes/cache/images/${theme.label}-fhd.png`, fullPage: true })
    await browser.close()
    console.log(`saved ${theme.label}-fhd.png`)
  }
}

async function dialog() {
  const browser = await chromium.launch()
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } })
  await page.goto(url)
  await page.getByText('Учёт времени').waitFor({ state: 'visible' })
  await page.getByRole('button', { name: 'Залогировать время' }).click()
  await page.getByRole('dialog').waitFor({ state: 'visible' })
  await page.screenshot({ path: '/root/.hermes/cache/images/log-work-dialog-open.png', fullPage: true })
  await browser.close()
  console.log('saved log-work-dialog-open.png')
}

await capture({ width: 375, height: 667 }, 'issue-detail-mobile')
await capture({ width: 1920, height: 1080 }, 'issue-detail-fhd')
await capture({ width: 2560, height: 1440 }, 'issue-detail-2k')
await themedScreenshots()
await dialog()
