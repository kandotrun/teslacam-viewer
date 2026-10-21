import { defineConfig } from '@playwright/test'

const port = process.env.PLAYWRIGHT_PORT ?? '4173'
if (!/^\d+$/u.test(port)) throw new Error('PLAYWRIGHT_PORT must be numeric')
const localBaseURL = `http://127.0.0.1:${port}`
const requestedBaseURL = process.env.PLAYWRIGHT_BASE_URL
const baseURL = requestedBaseURL ?? localBaseURL

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  workers: 1,
  reporter: [['line'], ['json', { outputFile: '.omo/evidence/teslacam-viewer/json/playwright-results.json' }]],
  use: {
    baseURL,
    browserName: 'chromium',
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
    viewport: { width: 1280, height: 1000 },
  },
  webServer:
    requestedBaseURL === undefined
      ? {
          command: `npm run build && npx vite preview --host 127.0.0.1 --port ${port}`,
          reuseExistingServer: false,
          timeout: 120_000,
          url: localBaseURL,
        }
      : undefined,
})
