import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './e2e',
  workers: 1,
  fullyParallel: false,
  timeout: 30_000,
  use: {
    baseURL: 'http://127.0.0.1:19873',
    channel: 'chrome',
    headless: true,
    viewport: { width: 1440, height: 900 },
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
})
