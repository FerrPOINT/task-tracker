import { test, expect } from '@playwright/test'

test('time tracking panel and worklog flow', async ({ page }) => {
  await page.goto('/issues/issue-1')

  await expect(page.getByText('Учёт времени')).toBeVisible()
  await expect(page.getByTestId('time-tracking-summary')).toHaveText('3h потрачено / 8h оценка / 4h осталось')

  await expect(page.getByRole('tab', { name: 'Worklog' })).toBeVisible()
  await expect(page.getByRole('cell', { name: 'API integration' })).toBeVisible()

  await page.getByRole('button', { name: 'Залогировать время' }).click()
  await expect(page.getByRole('dialog')).toBeVisible()

  await page.getByLabel('Затраченное время').fill('30m')
  await page.getByLabel('Оставшаяся оценка').fill('3h 30m')
  await page.getByLabel('Дата начала').fill('2026-07-21')
  await page.getByLabel('Комментарий').fill('E2E worklog')
  await page.getByRole('button', { name: 'Сохранить' }).click()
  await expect(page.getByRole('dialog')).not.toBeVisible()

  await expect(page.getByTestId('time-tracking-summary')).toHaveText('3h 30m потрачено / 8h оценка / 3h 30m осталось')

  await page.getByRole('button', { name: 'Редактировать запись' }).first().click()
  await page.getByLabel('Затраченное время').fill('1h')
  await page.getByRole('button', { name: 'Сохранить' }).click()

  await expect(page.getByRole('cell', { name: '1h' }).first()).toBeVisible()

  await page.getByRole('button', { name: 'Удалить запись' }).first().click()
  await page.getByRole('button', { name: 'Удалить' }).click()

  await expect(page.getByRole('cell', { name: 'E2E worklog' })).toHaveCount(0)
})

test('timer adds time to input', async ({ page }) => {
  await page.goto('/issues/issue-1')

  await page.getByRole('button', { name: 'Залогировать время' }).click()
  await page.getByLabel('Запустить таймер').click()
  await page.waitForTimeout(1100)
  await page.getByLabel('Остановить таймер').click()

  await expect(page.getByLabel('Затраченное время')).toHaveValue(/1s|1m/)
})
