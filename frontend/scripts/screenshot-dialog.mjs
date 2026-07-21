import { chromium } from '@playwright/test'

const url = 'http://localhost:4173/issues/issue-1'
const out = '/root/.hermes/cache/images/log-work-dialog-open.png'

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } })
await page.goto(url)
await page.getByRole('button', { name: 'Залогировать время' }).click()
await page.getByRole('dialog').waitFor({ state: 'visible' })
await page.screenshot({ path: out, fullPage: true })
await browser.close()
console.log(`saved ${out}`)
