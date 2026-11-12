import { expect, test, type Page } from '@playwright/test';
import { clearClipboard, copiedImage, menu, setColor, waitForEditor } from './helpers';
import {
  SERIF,
  fontControl,
  fontEntries,
  fontList,
  imageReference,
  installLocalFonts,
  installShellFonts,
  lfaLog,
  openFontList,
  pickFamily,
  sameInk,
  shellLog,
} from './font-helpers';
import { installTauriMock } from './tauri-mock';
import { H, W, at, chooseText, liveInput, setFontSize, textBox, undoAll } from './text-helpers';

// docs/specs/system-fonts.md AC-15, AC-16: the desktop app lists the system
// families its shell reports (`system_font_faces`, `fontdb`), without a
// permission prompt, and reads a face's file through `system_font_file`.
// The shell is the mocked Tauri bridge; `fontdb` itself is covered by the
// unit tests in src-tauri/src/fonts.rs.

test.use({
  permissions: ['clipboard-read', 'clipboard-write'],
  userAgent:
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
});

/** Opens the desktop app with a new image and the Text tool at size 32. */
async function start(page: Page): Promise<void> {
  await page.goto('./');
  await page.getByRole('button', { name: 'Got it' }).click();
  await page.getByRole('button', { name: 'New image' }).click();
  const card = page.getByRole('form', { name: 'New image' });
  await card.getByLabel('Width').fill(String(W));
  await card.getByLabel('Height').fill(String(H));
  await card.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, 'image');
  await setColor(page, 'fg', '#000000');
  await chooseText(page);
  await setFontSize(page, 32);
}

async function imagePlace(page: Page): Promise<number[]> {
  const [x, y] = await at(page, [20.5, 24.5]);
  await page.mouse.click(x, y);
  await expect(liveInput(page)).toBeFocused();
  await page.keyboard.type('Hg');
  await page.getByRole('button', { name: 'Finish', exact: true }).click();
  await expect(textBox(page)).toHaveCount(0);
  await clearClipboard(page);
  await menu(page, 'Edit', 'Copy');
  return (await copiedImage(page)).data;
}

const imageRef = (file: string, index = 0) =>
  imageReference({
    width: W,
    height: H,
    file,
    index,
    text: 'Hg',
    x: 20,
    y: 24,
    size: 32,
    color: 0x000000ff,
  });

test.beforeEach(async ({ page }) => {
  page.on('dialog', (d) => void d.accept());
  // A Local Font Access API the desktop app must not use.
  await installLocalFonts(page, { permission: 'prompt', answer: 'grant', fonts: SERIF });
  await installTauriMock(page);
  await installShellFonts(page);
});

test('AC-15 / AC-16: the font list shows the families the shell reports, without a prompt', async ({
  page,
}) => {
  await start(page);
  await openFontList(page);
  expect((await fontEntries(page).allTextContents()).map((s) => s.trim())).toEqual([
    'Desk Sans',
    'Liberation Sans',
    'Pincel Test Serif',
  ]);
  await expect(fontList(page).getByRole('button', { name: 'Add font file…' })).toHaveCount(0);
  await expect(fontList(page).locator('[data-font-note]')).toHaveCount(0);
  expect((await lfaLog(page)).queries).toBe(0);
  expect((await shellLog(page)).reads).toEqual([]);
});

test('AC-15: text placed with a system family equals the core output for its Regular face', async ({
  page,
}) => {
  await start(page);
  await pickFamily(page, 'Pincel Test Serif');
  expect(sameInk(await imagePlace(page), await imageRef('PincelTestSerif.ttc', 0))).toBe(true);
  // Bold is face 1 of the same file: the file is read once.
  expect(await undoAll(page)).toBe(1);
  await page.getByRole('checkbox', { name: 'Bold' }).check();
  expect(sameInk(await imagePlace(page), await imageRef('PincelTestSerif.ttc', 1))).toBe(true);
  expect((await shellLog(page)).reads).toEqual([0]);
  expect((await lfaLog(page)).queries).toBe(0);
  await expect(fontControl(page)).toHaveAttribute('aria-label', 'Font: Pincel Test Serif');
});
