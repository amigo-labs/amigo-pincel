import { defineConfig, devices } from '@playwright/test';

// End-to-end tests run against the static build served by `vite preview`
// (build first: `pnpm build && pnpm test:e2e`), so they see the prerendered
// HTML the Cloudflare Worker serves. docs/specs/website.md, Technical notes.
// `PINCEL_SITE_E2E_PORT` moves the preview server off the default port, so
// two checkouts (or the editor's suite on 4173) can run side by side without
// one reusing the other's server and testing the wrong build.
const port = Number(process.env.PINCEL_SITE_E2E_PORT ?? 4174);

export default defineConfig({
  testDir: 'e2e',
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: 0,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  timeout: 30_000,
  use: {
    baseURL: `http://localhost:${port}`,
    locale: 'en-US',
    trace: 'retain-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: `pnpm preview --port ${port} --strictPort`,
    url: `http://localhost:${port}/`,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
