import { expect, test, type Page } from '@playwright/test';
import { clearClipboard, copiedImage, menu, setColor, waitForEditor } from './helpers';
import {
  fontEntries,
  imageReference,
  installShellFonts,
  openFontList,
  openPixelText,
  pickFamily,
  pixelReference,
  sameInk,
  shellLog,
  statusBar,
} from './font-helpers';
import { emit, installTauriMock, type MockWindow } from './tauri-mock';
import { H, W, at, chooseText, liveInput, setFontSize, textBox } from './text-helpers';

// docs/specs/system-fonts-mcp.md AC-11: in live mode the desktop app hosts
// the agent tools; `font.list` reports the app's own font list and
// `draw.text` with `font` draws what the editor's Text tool draws with that
// family. The test plays the MCP endpoint (src-tauri/src/mcp) on the mocked
// Tauri bridge, whose font commands list and read the fixture faces.

test.use({
  permissions: ['clipboard-read', 'clipboard-write'],
  userAgent:
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
});

interface Wire {
  ok: boolean;
  json?: Record<string, unknown>;
  error?: string;
}

let nextId = 1;

/** Forwards the tool call `tool` as the endpoint does; waits for the reply. */
async function call(page: Page, tool: string, args: Record<string, unknown> = {}): Promise<Wire> {
  const id = nextId++;
  await emit(page, 'mcp-call', { id, tool, args, chat: null });
  await page.waitForFunction((i) => i in (window as unknown as MockWindow).__mock.mcpReplies, id);
  return page.evaluate(
    (i) => (window as unknown as MockWindow).__mock.mcpReplies[i],
    id,
  ) as Promise<Wire>;
}

/** The active layer, read through Copy. */
async function layerPixels(page: Page): Promise<number[]> {
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  return (await copiedImage(page)).data;
}

async function launch(page: Page): Promise<void> {
  await page.goto('./');
  await page.getByRole('button', { name: 'Got it' }).click();
}

/** A new `width`×`height` document from the start screen, black foreground. */
async function newDoc(page: Page, mode: 'pixel' | 'image', width: number, height: number) {
  const name = mode === 'pixel' ? 'New pixel sprite' : 'New image';
  await page.getByRole('button', { name }).click();
  const card = page.getByRole('form', { name });
  await card.getByLabel('Width').fill(String(width));
  await card.getByLabel('Height').fill(String(height));
  await card.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, mode);
  await setColor(page, 'fg', '#000000');
}

const BLACK = 0x000000ff;
const SPRITE = 48;

test.beforeEach(async ({ page }) => {
  page.on('dialog', (d) => void d.accept());
  await installTauriMock(page);
  await installShellFonts(page, { missing: [2] });
});

test('AC-11 font.list reports the families of the app font list, reading no face', async ({
  page,
}) => {
  await launch(page);
  // No document is needed, as for doc.list.
  const all = await call(page, 'font.list');
  expect(all.ok, all.error).toBe(true);
  expect(all.json).toEqual({
    families: [
      { name: 'Desk Sans', styles: ['Regular'] },
      { name: 'Liberation Sans', styles: ['Regular'] },
      { name: 'Pincel Test Serif', styles: ['Regular', 'Italic', 'Bold'] },
    ],
  });
  const some = await call(page, 'font.list', { query: 'SERIF' });
  expect(some.json).toEqual({
    families: [{ name: 'Pincel Test Serif', styles: ['Regular', 'Italic', 'Bold'] }],
  });
  // The same families as the Text tool's font list.
  const listed = all.json?.families as { name: string }[];
  await newDoc(page, 'image', W, H);
  await chooseText(page);
  await openFontList(page);
  const shown = (await fontEntries(page).allTextContents()).map((s) => s.trim());
  expect(listed.map((f) => f.name)).toEqual(shown);
  expect((await shellLog(page)).reads).toEqual([]);
});

test('AC-11 draw.text with font on an Image document draws what the editor draws', async ({
  page,
}) => {
  await launch(page);
  await newDoc(page, 'image', W, H);
  const r = await call(page, 'draw.text', {
    text: 'Hg',
    x: 20,
    y: 24,
    size: 32,
    color: '#000000',
    // Matched ignoring case (AC-06); Bold is face 1 of the collection.
    font: 'pincel test serif',
    bold: true,
  });
  expect(r.ok, r.error).toBe(true);
  const agent = await layerPixels(page);
  const reference = await imageReference({
    width: W,
    height: H,
    file: 'PincelTestSerif.ttc',
    index: 1,
    text: 'Hg',
    x: 20,
    y: 24,
    size: 32,
    color: BLACK,
  });
  expect(sameInk(agent, reference)).toBe(true);
  expect((await shellLog(page)).reads).toEqual([0]);
  // The editor's Text tool, same family, Bold, at the same place.
  await page.keyboard.press('Control+z');
  await chooseText(page);
  await setFontSize(page, 32);
  await pickFamily(page, 'Pincel Test Serif');
  await page.getByRole('checkbox', { name: 'Bold' }).check();
  const [x, y] = await at(page, [20.5, 24.5]);
  await page.mouse.click(x, y);
  await expect(liveInput(page)).toBeFocused();
  await page.keyboard.type('Hg');
  await page.getByRole('button', { name: 'Finish', exact: true }).click();
  await expect(textBox(page)).toHaveCount(0);
  expect(sameInk(await layerPixels(page), agent)).toBe(true);
  // The face was read once for both.
  expect((await shellLog(page)).reads).toEqual([0]);
});

test('AC-11 draw.text with font on a Pixel document draws what the editor draws', async ({
  page,
}) => {
  await launch(page);
  await newDoc(page, 'pixel', SPRITE, SPRITE);
  const r = await call(page, 'draw.text', {
    text: 'Hg',
    x: 2,
    y: 2,
    size: 16,
    color: '#000000',
    font: 'Pincel Test Serif',
    italic: true,
  });
  expect(r.ok, r.error).toBe(true);
  // Only the face the text uses was read.
  expect((await shellLog(page)).reads).toEqual([1]);
  const agent = await layerPixels(page);
  const reference = await pixelReference({
    width: SPRITE,
    height: SPRITE,
    file: 'PincelTestSerif-Italic.ttf',
    text: 'Hg',
    x: 2,
    y: 2,
    size: 16,
    color: BLACK,
  });
  expect(sameInk(agent, reference)).toBe(true);
  // The editor's Text dialog, same family, Italic, at the same place.
  await page.keyboard.press('Control+z');
  const dialog = await openPixelText(page, 2, 2, SPRITE);
  await pickFamily(page, 'Pincel Test Serif');
  await dialog.getByLabel('Font size').fill('16');
  await dialog.getByRole('checkbox', { name: 'Italic' }).check();
  await dialog.getByLabel('Text content').fill('Hg');
  await dialog.getByRole('button', { name: 'Place' }).click();
  await expect(statusBar(page)).toContainText('text placed');
  expect(sameInk(await layerPixels(page), agent)).toBe(true);
  // The Italic file was read once; the dialog's preview read the Regular.
  expect((await shellLog(page)).reads.filter((f) => f === 1)).toEqual([1]);
});

test('AC-11 draw.text with a face the shell cannot read fails and changes nothing', async ({
  page,
}) => {
  await launch(page);
  await newDoc(page, 'image', W, H);
  const r = await call(page, 'draw.text', { text: 'Hg', x: 20, y: 24, font: 'Desk Sans' });
  expect(r.ok).toBe(false);
  expect(r.error).toContain('Desk Sans');
  expect(r.error).toContain('the font file is gone');
  const unknown = await call(page, 'draw.text', { text: 'Hg', x: 20, y: 24, font: 'Nope' });
  expect(unknown.ok).toBe(false);
  expect(unknown.error).toContain('font.list');
  expect((await call(page, 'doc.info')).json?.undo_depth).toBe(0);
});
