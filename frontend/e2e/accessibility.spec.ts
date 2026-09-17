import AxeBuilder from '@axe-core/playwright'
import { expect, test } from '@playwright/test'
import { authenticatePage, seedIntegrationData } from './setup'

const cases = [
  { name: 'projects', path: '/projects', heading: /Проекты|Projects/ },
  { name: 'board', path: '/projects/DEMO/board', heading: /Доска|Board/ },
  {
    name: 'issue create',
    path: '/issues/create?project_key=DEMO',
    heading: /Создать задачу|New issue/,
  },
]

const viewports = [
  { name: 'mobile', width: 375, height: 812 },
  { name: 'desktop', width: 1280, height: 800 },
]

test.setTimeout(120_000)

test.beforeAll(async () => {
  await seedIntegrationData()
})

for (const testCase of cases) {
  for (const viewport of viewports) {
    test(`${testCase.name} has no serious accessibility violations on ${viewport.name}`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport)
      await authenticatePage(page)
      await page.goto(testCase.path)
      await expect(page.getByRole('heading', { name: testCase.heading }).first()).toBeVisible({
        timeout: 10_000,
      })

      const results = await new AxeBuilder({ page }).analyze()
      const blockingViolations = results.violations.filter(
        (violation) => violation.impact === 'serious' || violation.impact === 'critical',
      )

      expect(blockingViolations).toEqual([])
    })
  }
}
