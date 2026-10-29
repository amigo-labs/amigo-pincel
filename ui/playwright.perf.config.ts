import { defineConfig, devices } from '@playwright/test';

// Browser performance script (docs/specs/performance-benchmarks.md AC-03),
// run by `pnpm perf` after `pnpm build` — never by `pnpm test:e2e` or the
// PR gate (AC-05). Like the e2e suite it measures the built bundle through
// `vite preview`; for representative numbers build the release wasm first
// (`pnpm wasm:build:release`). Scenarios run one at a time so they don't
// compete for the CPU. Results land in `e2e-perf/results/browser.json`.
const port = Number(process.env.PINCEL_E2E_PORT ?? 4173);

export default defineConfig({
  testDir: 'e2e-perf',
  testMatch: /.*\.perf\.ts$/,
  globalSetup: './e2e-perf/global-setup.ts',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: 'list',
  timeout: 600_000,
  use: {
    baseURL: `http://localhost:${port}/app/`,
    serviceWorkers: 'block',
    locale: 'en-US',
    trace: 'off',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: `pnpm preview --port ${port} --strictPort`,
    url: `http://localhost:${port}/app/`,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
