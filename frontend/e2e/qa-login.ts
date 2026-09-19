import { expect, test, type Locator, type Page } from '@playwright/test'

export async function signInAt(
  page: Page,
  url: string,
  account: { email: string; password: string },
  ready?: Locator,
) {
  test.setTimeout(Math.max(test.info().timeout, 150_000))
  for (let attempt = 0; attempt < 2; attempt++) {
    await page.goto(url)
    await page.getByLabel('Email').fill(account.email)
    await page.getByLabel('Пароль').fill(account.password)
    const loginResponse = page.waitForResponse(
      (response) =>
        response.url().startsWith('http://localhost:7701/oidc/login') &&
        response.request().method() === 'POST',
    )
    await page.getByRole('button', { name: 'Войти', exact: true }).click()
    const response = await loginResponse
    if (response.status() === 429 && attempt === 0) {
      await page.waitForTimeout(62_000)
      continue
    }
    expect(response.status(), 'Central Auth browser login').toBeLessThan(400)
    await expect(
      ready ?? page.getByRole('button', { name: 'Открыть список сервисов' }),
    ).toBeVisible({
      timeout: 30_000,
    })
    return
  }
}
