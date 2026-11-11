import { expect, test, type Locator, type Page } from '@playwright/test';
import { imageCanvas, menu, pixelCanvas, waitForEditor } from './helpers';
import { diskPaths, emit, installTauriMock, putFile, setMock, type MockWindow } from './tauri-mock';

// Live MCP calls in the desktop app (docs/specs/mcp-live-desktop.md),
// against the mocked Tauri bridge: the test plays the endpoint, emitting
// `mcp-call` events as src-tauri/src/mcp forwards them and reading the
// replies the UI sends back through `mcp_call_result`.

interface Wire {
  ok: boolean;
  json?: Record<string, unknown>;
  error?: string;
}

let nextId = 1;

/** Forwards the tool call `tool` to the UI; returns its call id. */
async function send(page: Page, tool: string, args: Record<string, unknown> = {}): Promise<number> {
  const id = nextId++;
  await emit(page, 'mcp-call', { id, tool, args, chat: null });
  return id;
}

/** The UI's reply to the call `id`, once it arrives. */
async function reply(page: Page, id: number): Promise<Wire> {
  await page.waitForFunction((i) => i in (window as unknown as MockWindow).__mock.mcpReplies, id);
  return page.evaluate((i) => (window as unknown as MockWindow).__mock.mcpReplies[i], id) as Promise<Wire>;
}

async function call(page: Page, tool: string, args: Record<string, unknown> = {}): Promise<Wire> {
  return reply(page, await send(page, tool, args));
}

function canvas(page: Page, mode: 'pixel' | 'image'): Locator {
  return mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);
}

function tabs(page: Page): Locator {
  return page.getByRole('tablist', { name: 'Open documents' }).getByRole('tab');
}

/** Opens the app (past the first-launch note) with `mode`'s 16×16 document. */
async function start(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  await page.goto('./');
  await page.getByRole('button', { name: 'Got it' }).click();
  const name = mode === 'pixel' ? 'New pixel sprite' : 'New image';
  await page.getByRole('button', { name }).click();
  const card = page.getByRole('form', { name });
  for (const label of ['Width', 'Height']) await card.getByLabel(label).fill('16');
  await card.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, mode);
}

const RECT = { x0: 2, y0: 2, x1: 12, y1: 12, color: '#ff0000', filled: true };
const STROKE = { points: [[2, 2], [13, 13]], size: 4, color: '#ff0000' };

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
});

for (const mode of ['pixel', 'image'] as const) {
  test(`AC-07 a ${mode} tool call changes the open document and Undo reverts exactly it`, async ({
    page,
  }) => {
    await start(page, mode);
    const before = await canvas(page, mode).screenshot();
    const r = await call(page, mode === 'pixel' ? 'draw.rect' : 'draw.stroke', mode === 'pixel' ? RECT : STROKE);
    expect(r.ok, r.error).toBe(true);
    await expect.poll(async () => (await canvas(page, mode).screenshot()).equals(before)).toBe(false);
    const info = await call(page, 'doc.info');
    expect(info.json?.undo_depth).toBe(1);
    await page.keyboard.press('Control+z');
    await expect.poll(async () => (await canvas(page, mode).screenshot()).equals(before)).toBe(true);
    expect((await call(page, 'doc.info')).json?.undo_depth).toBe(0);
  });
}

test('AC-09 a document tool without an open document fails', async ({ page }) => {
  await page.goto('./');
  await page.getByRole('button', { name: 'Got it' }).click();
  const r = await call(page, 'draw.rect', RECT);
  expect(r.ok).toBe(false);
  expect(r.error).toContain('no document is open');
});

test('AC-15 doc.new opens a new tab; doc.close of a dirty tab asks and a cancel keeps it', async ({
  page,
}) => {
  await start(page, 'pixel');
  const created = await call(page, 'doc.new', { mode: 'image', width: 20, height: 10 });
  expect(created.ok, created.error).toBe(true);
  expect(created.json).toMatchObject({ mode: 'image', width: 20, height: 10 });
  await expect(tabs(page)).toHaveCount(2);
  await expect(tabs(page).nth(1)).toHaveAttribute('aria-selected', 'true');
  await waitForEditor(page, 'image');
  // Without `doc` a call acts on the active tab.
  expect((await call(page, 'draw.stroke', STROKE)).ok).toBe(true);
  const closing = await send(page, 'doc.close');
  const prompt = page.getByRole('alertdialog', { name: 'Unsaved changes' });
  await expect(prompt).toBeVisible();
  await prompt.getByRole('button', { name: 'Cancel' }).click();
  const r = await reply(page, closing);
  expect(r.ok).toBe(false);
  expect(r.error).toContain('user cancelled');
  await expect(tabs(page)).toHaveCount(2);
  // A clean tab closes without a question.
  const first = (await call(page, 'doc.list')).json?.documents as { doc: string }[];
  const closed = await call(page, 'doc.close', { doc: first[0]?.doc });
  expect(closed.ok, closed.error).toBe(true);
  await expect(tabs(page)).toHaveCount(1);
});

test('AC-10 AC-11 doc.save: Save As asks first, a cancel writes nothing, Save reuses the file', async ({
  page,
}) => {
  await start(page, 'pixel');
  await call(page, 'draw.rect', RECT);
  await setMock(page, { saveQueue: [null] });
  const cancelled = await call(page, 'doc.save', { path: 'art/hero.aseprite' });
  expect(cancelled.ok).toBe(false);
  expect(cancelled.error).toContain('user cancelled');
  expect(await diskPaths(page)).toEqual([]);
  await setMock(page, { saveQueue: ['/docs/hero.aseprite'] });
  const saved = await call(page, 'doc.save', { path: 'art/hero.aseprite' });
  expect(saved.ok, saved.error).toBe(true);
  expect(saved.json).toMatchObject({ dirty: false, path: '/docs/hero.aseprite' });
  expect(await diskPaths(page)).toEqual(['/docs/hero.aseprite']);
  const dialogs = await page.evaluate(() => (window as unknown as MockWindow).__mock.saveDialogs.length);
  await call(page, 'draw.rect', { ...RECT, color: '#00ff00' });
  const again = await call(page, 'doc.save');
  expect(again.ok, again.error).toBe(true);
  expect(await page.evaluate(() => (window as unknown as MockWindow).__mock.saveDialogs.length)).toBe(dialogs);
  // AC-19: the call's clock paused while its dialogs waited.
  const waiting = await page.evaluate(() => (window as unknown as MockWindow).__mock.mcpWaiting);
  expect(waiting.filter(([, on]) => on).length).toBeGreaterThanOrEqual(2);
  expect(waiting.filter(([, on]) => !on).length).toBe(waiting.filter(([, on]) => on).length);
});

test('AC-16 exports go through the save dialog, imports through the open dialog', async ({
  page,
}) => {
  await start(page, 'pixel');
  await setMock(page, { saveQueue: ['/exports/final.png'] });
  const r = await call(page, 'export.png', { path: 'out/sprite.png' });
  expect(r.ok, r.error).toBe(true);
  expect(r.json?.saved_to).toEqual(['/exports/final.png']);
  const lastSave = await page.evaluate(() => (window as unknown as MockWindow).__mock.saveDialogs.at(-1));
  expect(lastSave?.defaultPath).toBe('sprite.png');
  const png = await page.evaluate(() => (window as unknown as MockWindow).__mock.disk['/exports/final.png']);
  expect(png?.slice(0, 4)).toEqual([0x89, 0x50, 0x4e, 0x47]);
  await setMock(page, { saveQueue: [null] });
  const cancelled = await call(page, 'export.png', { path: 'out/other.png' });
  expect(cancelled.error).toContain('user cancelled');
  expect(await diskPaths(page)).toEqual(['/exports/final.png']);

  await putFile(page, '/pal/warm.gpl', Buffer.from('GIMP Palette\n10 20 30 Dark\n200 100 0\n'));
  await setMock(page, { openPick: '/pal/warm.gpl' });
  const imported = await call(page, 'palette.import', { path: 'palettes/warm.gpl' });
  expect(imported.ok, imported.error).toBe(true);
  const lastOpen = await page.evaluate(() => (window as unknown as MockWindow).__mock.openDialogs.at(-1));
  expect(lastOpen?.defaultPath).toBe('palettes/warm.gpl');
  expect((await call(page, 'palette.get')).json?.entries).toHaveLength(2);
  await setMock(page, { openPick: null });
  const refused = await call(page, 'palette.import', { path: 'palettes/warm.gpl' });
  expect(refused.error).toContain('user cancelled');
});

test('AC-17 a call during a stroke waits for pointerup', async ({ page }) => {
  await start(page, 'pixel');
  await page.locator('[data-editor]:not([inert])').getByTitle('Pencil (B)').click();
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('no canvas');
  await page.mouse.move(box.x + box.width * 0.3, box.y + box.height * 0.3);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.6, box.y + box.height * 0.6, { steps: 4 });
  const id = await send(page, 'draw.rect', RECT);
  await page.waitForTimeout(400);
  expect(await page.evaluate((i) => i in (window as unknown as MockWindow).__mock.mcpReplies, id)).toBe(false);
  await page.mouse.up();
  const r = await reply(page, id);
  expect(r.ok, r.error).toBe(true);
  // The stroke and the call are two undo steps.
  expect((await call(page, 'doc.info')).json?.undo_depth).toBe(2);
});

test('AC-18 a call while a modal dialog is open fails at once and changes nothing', async ({
  page,
}) => {
  await start(page, 'image');
  await menu(page, 'Help', 'About Pincel');
  const about = page.getByRole('dialog', { name: 'About Pincel' });
  await expect(about).toBeVisible();
  const r = await call(page, 'draw.stroke', STROKE);
  expect(r.ok).toBe(false);
  expect(r.error).toBe('editor busy: a dialog is open');
  await about.getByRole('button', { name: 'Close' }).click();
  await expect(about).toBeHidden();
  expect((await call(page, 'doc.info')).json?.undo_depth).toBe(0);
});

test('doc.list reads background tabs without switching the shown document', async ({ page }) => {
  await start(page, 'pixel');
  for (let i = 0; i < 2; i += 1) {
    expect((await call(page, 'doc.new', { mode: 'pixel', width: 16, height: 16 })).ok).toBe(true);
  }
  await tabs(page).nth(1).click();
  await expect(tabs(page).nth(1)).toHaveAttribute('aria-selected', 'true');
  expect((await call(page, 'draw.rect', RECT)).ok).toBe(true);
  const shown = await pixelCanvas(page).screenshot();
  const listed = await call(page, 'doc.list');
  expect((listed.json?.documents as unknown[]).length).toBe(3);
  expect(listed.json?.active).toBe((await call(page, 'doc.info')).json?.doc);
  await page.waitForTimeout(200);
  expect((await pixelCanvas(page).screenshot()).equals(shown)).toBe(true);
});
