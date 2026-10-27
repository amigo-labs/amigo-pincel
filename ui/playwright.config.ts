import { defineConfig, devices } from '@playwright/test';

// End-to-end tests run against the production bundle served by
// `vite preview` (build first: `pnpm build && pnpm test:e2e`), so they see
// the real `/app/` base path and the release-shaped wasm loading. The
// service worker is blocked: precaching the ~13 MiB of dev wasm on every
// test would only slow the run, and offline behaviour is not under test.
// docs/specs/pincel.md §7.4.
const port = 4173;

export default defineConfig({
  testDir: 'e2e',
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  // Playwright defaults to a single worker under CI, which ran the specs
  // serially for ~56s. The runner has 4 vCPUs and `vite preview` needs one,
  // so two is the safe step up: measured over all 62 tests, it takes the
  // suite from 1m24s to 49s with no flakes. Spread rather than a plain
  // `: undefined`, which exactOptionalPropertyTypes rejects -- leaving the
  // key out is what restores Playwright's own local default.
  ...(process.env.CI ? { workers: 2 } : {}),
  retries: 0,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  timeout: 30_000,
  use: {
    baseURL: `http://localhost:${port}/app/`,
    serviceWorkers: 'block',
    // English UI unless a spec opts into another language (i18n.spec.ts).
    locale: 'en-US',
    trace: 'retain-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: `pnpm preview --port ${port} --strictPort`,
    url: `http://localhost:${port}/app/`,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
