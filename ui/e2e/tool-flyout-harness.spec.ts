import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { fileURLToPath } from 'node:url';
import { createServer, type ViteDevServer } from 'vite';
import type { HarnessHooks } from './harness/tool-rail';

// The shared tool rail mounted alone with made-up groups
// (e2e/harness/tool-rail.html): no shipping group has a disabled variant,
// so the disabled-entry rules of tool-variant-flyout (AC-05, AC-12) and the
// close-on-unmount / document-identity rules (AC-14) are checked here on
// the real component, served by a Vite dev server this file starts.

let server: ViteDevServer | null = null;
let url = '';

test.beforeAll(async () => {
  server = await createServer({
    configFile: false,
    root: fileURLToPath(new URL('..', import.meta.url)),
    plugins: [svelte(), tailwindcss()],
    logLevel: 'error',
    optimizeDeps: { noDiscovery: true, include: [] },
    server: { host: '127.0.0.1', port: 0, strictPort: false },
  });
  await server.listen();
  const base = server.resolvedUrls?.local[0];
  if (!base) throw new Error('harness server has no URL');
  url = `${base}e2e/harness/tool-rail.html`;
});

test.afterAll(async () => {
  await server?.close();
});

async function openHarness(page: Page): Promise<void> {
  await page.goto(url);
  await expect(rail(page).locator('[data-group="variants"]')).toBeVisible();
}

/** Runs one harness hook in the page. */
function hook(page: Page, run: (h: HarnessHooks) => void): Promise<void> {
  return page.evaluate(`(${run.toString()})(window.harness)`);
}

function rail(page: Page): Locator {
  return page.getByRole('toolbar', { name: 'Tools' });
}

function menu(page: Page): Locator {
  return page.getByRole('menu', { name: 'Variants', exact: true });
}

function entry(page: Page, name: string): Locator {
  return menu(page).getByRole('menuitemradio', { name, exact: true });
}

function active(page: Page): Locator {
  return page.getByRole('status', { name: 'Active tool' });
}

test('a disabled variant is shown and announced disabled and cannot be chosen (AC-05, AC-12)', async ({ page }) => {
  await openHarness(page);
  await rail(page).locator('[data-group="variants"]').click({ button: 'right' });
  await expect(menu(page)).toBeVisible();
  await expect(entry(page, 'Two')).toBeDisabled();
  await expect(menu(page).getByRole('menuitemradio', { name: 'Two', disabled: true })).toHaveCount(1);
  await expect(menu(page).getByRole('menuitemradio', { disabled: false })).toHaveCount(2);
  await expect(menu(page)).toMatchAriaSnapshot(`
    - menu "Variants":
      - menuitemradio "One" [checked=false]
      - menuitemradio "Two" [checked=false] [disabled]
      - menuitemradio "Three" [checked=false]
  `);

  // Keyboard: focus never lands on it.
  await expect(entry(page, 'One')).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(entry(page, 'Three')).toBeFocused();

  // Pointer: a click on it chooses nothing and keeps the flyout open
  // (focus falls back to the menu; the arrows start from the first entry).
  await entry(page, 'Two').click({ force: true });
  await expect(menu(page)).toBeVisible();
  await expect(active(page)).toHaveText('solo');
  await page.keyboard.press('ArrowDown');
  await expect(entry(page, 'One')).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(entry(page, 'Three')).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(entry(page, 'One')).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(entry(page, 'Three')).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(menu(page)).toBeHidden();
  await expect(active(page)).toHaveText('three');
});

test('releasing a long press over a disabled entry or empty space chooses nothing (AC-03, AC-05)', async ({
  page,
}) => {
  await openHarness(page);
  const button = await rail(page).locator('[data-group="variants"]').boundingBox();
  if (!button) throw new Error('rail button not visible');
  await page.mouse.move(button.x + button.width / 2, button.y + button.height / 2);
  await page.mouse.down();
  await expect(menu(page)).toBeVisible();
  const two = await entry(page, 'Two').boundingBox();
  if (!two) throw new Error('entry not visible');
  await page.mouse.move(two.x + two.width / 2, two.y + two.height / 2, { steps: 4 });
  await page.mouse.up();
  await expect(menu(page)).toBeVisible();
  await expect(active(page)).toHaveText('solo');

  await page.keyboard.press('Escape');
  await page.mouse.move(button.x + button.width / 2, button.y + button.height / 2);
  await page.mouse.down();
  await expect(menu(page)).toBeVisible();
  await page.mouse.move(600, 400, { steps: 4 });
  await page.mouse.up();
  await expect(menu(page)).toBeVisible();
  await expect(active(page)).toHaveText('solo');
});

test('an entry enabled or disabled while open shows its new state at once (AC-05)', async ({ page }) => {
  await openHarness(page);
  await rail(page).locator('[data-group="variants"]').click({ button: 'right' });
  await expect(entry(page, 'Two')).toBeDisabled();
  await hook(page, (h) => h.setDisabled('two', false));
  await expect(entry(page, 'Two')).toBeEnabled();
  await hook(page, (h) => h.setDisabled('three', true));
  await expect(entry(page, 'Three')).toBeDisabled();
  await expect(menu(page)).toBeVisible();
});

test('the flyout closes when the document is replaced or the rail unmounts (AC-14)', async ({ page }) => {
  await openHarness(page);
  const group = rail(page).locator('[data-group="variants"]');
  await group.click({ button: 'right' });
  await expect(menu(page)).toBeVisible();
  await hook(page, (h) => h.replaceDocument());
  await expect(menu(page)).toBeHidden();
  await expect(active(page)).toHaveText('solo');

  await group.click({ button: 'right' });
  await expect(menu(page)).toBeVisible();
  await hook(page, (h) => h.unmount());
  await expect(rail(page)).toHaveCount(0);
  await expect(page.getByRole('menu')).toHaveCount(0);
});
