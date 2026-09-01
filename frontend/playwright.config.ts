import { defineConfig, devices } from '@playwright/test'

const previewHost = process.env.PLAYWRIGHT_HOST ?? '127.0.0.1'
const previewPort = process.env.PLAYWRIGHT_PORT ?? '4173'
const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? `http://${previewHost}:${previewPort}`
const useExternalServer = Boolean(process.env.PLAYWRIGHT_BASE_URL)

export default defineConfig({
  testDir: 'e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'list',
  use: {
    baseURL,
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    {
      name: 'firefox',
      use: { ...devices['Desktop Firefox'] },
    },
    {
      name: 'webkit',
      use: { ...devices['Desktop Safari'] },
    },
  ],
  webServer: useExternalServer
    ? undefined
    : {
        command: `pnpm exec vite preview --host ${previewHost} --port ${previewPort} --strictPort`,
        env: {
          VITE_API_BASE_URL: 'http://127.0.0.1:3456/api/v1',
        },
        url: baseURL,
        reuseExistingServer: false,
      },
})
