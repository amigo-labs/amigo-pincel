import { expect, test, type Page } from '@playwright/test';
import { clearClipboard, copiedImage, menu, setColor, waitForEditor } from './helpers';
import {
  SERIF,
  fixtureBytes,
  fontControl,
  fontEntries,
  fontList,
  imageReference,
  installLocalFonts,
  lfaLog,
  openFontList,
  pickFamily,
  sameInk,
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

/** One face as the shell lists it. */
interface ShellFace {
  file: number;
  index: number;
  family: string;
  style: string;
  weight: number;
  italic: boolean;
  stretch: number;
  postscript: string;
}

const FACES: ShellFace[] = [
  {
    file: 0,
    index: 0,
    family: 'Pincel Test Serif',
    style: 'Regular',
    weight: 400,
    italic: false,
    stretch: 5,
    postscript: 'PincelTestSerif-Regular',
  },
  {
    file: 0,
    index: 1,
    family: 'Pincel Test Serif',
    style: 'Bold',
    weight: 700,
    italic: false,
    stretch: 5,
    postscript: 'PincelTestSerif-Bold',
  },
  {
    file: 1,
    index: 0,
    family: 'Pincel Test Serif',
    style: 'Italic',
    weight: 400,
    italic: true,
    stretch: 5,
    postscript: 'PincelTestSerif-Italic',
  },
  {
    file: 2,
    index: 0,
    family: 'Desk Sans',
    style: 'Regular',
    weight: 400,
    italic: false,
    stretch: 5,
    postscript: 'DeskSans',
  },
];
const FILES = ['PincelTestSerif.ttc', 'PincelTestSerif-Italic.ttf', 'LiberationSans-Regular.ttf'];

/** Answers the font commands of the mocked shell; counts the calls. */
async function installShellFonts(page: Page): Promise<void> {
  const files = await Promise.all(
    FILES.map(async (f) => Buffer.from(await fixtureBytes(f)).toString('base64')),
  );
  await page.addInitScript(
    ({ faces, files }) => {
      const internals = (
        window as unknown as {
          __TAURI_INTERNALS__: {
            invoke: (c: string, a: Record<string, unknown>) => Promise<unknown>;
          };
        }
      ).__TAURI_INTERNALS__;
      const log = { lists: 0, reads: [] as number[] };
      (window as unknown as { __shellFonts: typeof log }).__shellFonts = log;
      const inner = internals.invoke;
      internals.invoke = async (cmd, args) => {
        if (cmd === 'system_font_faces') {
          log.lists += 1;
          return faces;
        }
        if (cmd === 'system_font_file') {
          const file = Number(args.file);
          log.reads.push(file);
          const b64 = files[file];
          if (b64 === undefined) throw new Error('unknown font file');
          return Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)).buffer;
        }
        return inner(cmd, args);
      };
    },
    { faces: FACES, files },
  );
}

async function shellLog(page: Page): Promise<{ lists: number; reads: number[] }> {
  return page.evaluate(
    () => (window as unknown as { __shellFonts: { lists: number; reads: number[] } }).__shellFonts,
  );
}

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
