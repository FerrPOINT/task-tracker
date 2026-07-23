import { test, expect } from '@playwright/test'

test('login, create issue, see it in backlog and search', async ({ page }) => {
  const title = `E2E smoke ${Date.now()}`

  await page.goto('/login')
  await page.fill('input[type="email"]', 'demo@example.com')
  await page.fill('input[type="password"]', 'demo')
  await page.click('button:has-text("Войти")')
  await page.waitForURL('/')
  await expect(page.locator('text=Team Dashboard')).toBeVisible()

  await page.goto('/issues/create')
  await page.fill('input[placeholder*="Краткое описание"]', title)
  await page.click('button:has-text("Создать")')
  await page.waitForURL('/projects/TT/backlog')
  await expect(page.locator('text=' + title).first()).toBeVisible()

  await page.goto('/search')
  await page.fill('textarea', title)
  await page.click('button:has-text("Искать")')
  await expect(page.locator('text=' + title).first()).toBeVisible()
})
