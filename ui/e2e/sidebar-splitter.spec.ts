import { expect, test, type Page } from '@playwright/test';
import { createDocument } from './helpers';

// Resizable docked sidebar (effect-panel AC-07 – AC-11): a splitter
// on the sidebar's left edge sets its width within 240 px and the smaller
// of 560 px and half the window; the width persists per mode. The German
// name (AC-15) is checked in effect-panel.spec.ts.

const sidebar = (page: Page) => page.getByRole('complementary', { name: 'Panels' });
const splitter = (page: Page) => page.getByRole('separator', { name: 'Sidebar width' });

async function sidebarWidth(page: Page): Promise<number> {
  const box = await sidebar(page).boundingBox();
  if (!box) throw new Error('sidebar is not visible');
  return Math.round(box.width);
}

/** Drags the splitter horizontally by `dx` px (negative = to the left). */
async function dragSplitter(page: Page, dx: number): Promise<void> {
  const box = await splitter(page).boundingBox();
  if (!box) throw new Error('splitter is not visible');
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + dx / 2, y, { steps: 4 });
  await page.mouse.move(x + dx, y, { steps: 4 });
  await page.mouse.up();
}

/** The stored `layout:<mode>` pref, read straight from IndexedDB. */
function storedLayout(page: Page, mode: 'pixel' | 'image'): Promise<unknown> {
  return page.evaluate(
    (key) =>
      new Promise<unknown>((resolve) => {
        const open = indexedDB.open('pincel');
        open.onsuccess = () => {
          const get = open.result.transaction('prefs').objectStore('prefs').get(key);
          get.onsuccess = () => resolve((get.result as { value?: unknown } | undefined)?.value);
          get.onerror = () => resolve(null);
        };
        open.onerror = () => resolve(null);
      }),
    `layout:${mode}`,
  );
}

/** Writes `width` as the stored sidebar width of `mode`. */
function storeWidth(page: Page, mode: 'pixel' | 'image', width: unknown): Promise<void> {
  return page.evaluate(
    ([key, value]) =>
      new Promise<void>((resolve, reject) => {
        const open = indexedDB.open('pincel');
        open.onsuccess = () => {
          const tx = open.result.transaction('prefs', 'readwrite');
          tx.objectStore('prefs').put({
            key,
            value: { collapsed: {}, sidebar: true, sidebarWidth: value },
          });
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(new Error('pref write failed'));
        };
        open.onerror = () => reject(new Error('IndexedDB unavailable'));
      }),
    [`layout:${mode}`, width] as const,
  );
}

const storedWidth = async (page: Page, mode: 'pixel' | 'image') =>
  ((await storedLayout(page, mode)) as { sidebarWidth?: unknown } | undefined)?.sidebarWidth;

test('dragging the splitter resizes the sidebar within its limits (AC-07)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  expect(await sidebarWidth(page)).toBe(288);
  const canvasArea = page.getByRole('main');
  const areaBefore = (await canvasArea.boundingBox())?.width ?? 0;

  await dragSplitter(page, -100);
  expect(await sidebarWidth(page)).toBe(388);
  // The canvas area takes the remaining width.
  expect(Math.round((await canvasArea.boundingBox())?.width ?? 0)).toBe(
    Math.round(areaBefore - 100),
  );

  // 1280 px window: the maximum is 560 px (less than half the window).
  await dragSplitter(page, -600);
  expect(await sidebarWidth(page)).toBe(560);
  await dragSplitter(page, 800);
  expect(await sidebarWidth(page)).toBe(240);

  // 1100 px window: half the window (550 px) is the smaller maximum.
  await page.setViewportSize({ width: 1100, height: 720 });
  await dragSplitter(page, -600);
  expect(await sidebarWidth(page)).toBe(550);
});

test('a double-click on the splitter restores the default width (AC-08)', async ({ page }) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  await dragSplitter(page, -150);
  expect(await sidebarWidth(page)).toBe(438);
  await splitter(page).dblclick();
  expect(await sidebarWidth(page)).toBe(288);
});

test('the splitter is a focusable separator the arrow keys move (AC-09)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const sep = splitter(page);
  await expect(sep).toHaveAttribute('aria-orientation', 'vertical');
  await expect(sep).toHaveAttribute('aria-valuenow', '288');
  await expect(sep).toHaveAttribute('aria-valuemin', '240');
  await expect(sep).toHaveAttribute('aria-valuemax', '560');

  await sep.focus();
  await expect(sep).toBeFocused();
  await page.keyboard.press('ArrowLeft');
  await expect(sep).toHaveAttribute('aria-valuenow', '304');
  expect(await sidebarWidth(page)).toBe(304);
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  await expect(sep).toHaveAttribute('aria-valuenow', '240');
  // Clamped at the minimum.
  await page.keyboard.press('ArrowRight');
  await expect(sep).toHaveAttribute('aria-valuenow', '240');
  expect(await sidebarWidth(page)).toBe(240);
});

test('the width survives a reload, per mode (AC-10)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await dragSplitter(page, -112);
  expect(await sidebarWidth(page)).toBe(400);
  await expect.poll(() => storedWidth(page, 'pixel')).toBe(400);

  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await expect.poll(() => sidebarWidth(page)).toBe(400);

  // Image mode keeps its own width.
  await createDocument(page, 'image', { width: 16, height: 16 });
  await page.waitForTimeout(300);
  expect(await sidebarWidth(page)).toBe(288);
});

test('a missing, broken or out-of-range stored width falls back silently (AC-11)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 16, height: 16 });

  for (const broken of ['wide', null, Number.NaN, Number.POSITIVE_INFINITY]) {
    await storeWidth(page, 'image', broken);
    await createDocument(page, 'image', { width: 16, height: 16 });
    await page.waitForTimeout(300);
    expect(await sidebarWidth(page)).toBe(288);
  }

  // Above the maximum: the nearest limit, stored value untouched.
  await storeWidth(page, 'image', 9999);
  await createDocument(page, 'image', { width: 16, height: 16 });
  await expect.poll(() => sidebarWidth(page)).toBe(560);
  await page.setViewportSize({ width: 1100, height: 720 });
  await expect.poll(() => sidebarWidth(page)).toBe(550);
  // Collapsing a panel writes the layout again; the width stays as stored.
  await page.getByRole('button', { name: /^Colors/, expanded: true }).click();
  await expect
    .poll(async () => ((await storedLayout(page, 'image')) as { collapsed?: object })?.collapsed)
    .toEqual({ colors: true });
  expect(await storedWidth(page, 'image')).toBe(9999);
  await page.setViewportSize({ width: 1280, height: 720 });
  await expect.poll(() => sidebarWidth(page)).toBe(560);

  // Below the minimum: 240 px.
  await storeWidth(page, 'image', 100);
  await createDocument(page, 'image', { width: 16, height: 16 });
  await expect.poll(() => sidebarWidth(page)).toBe(240);
  expect(await storedWidth(page, 'image')).toBe(100);

  // No error surfaced anywhere.
  await expect(page.getByRole('alert')).toHaveCount(0);
});
