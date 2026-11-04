import { expect, test, type Locator, type Page } from '@playwright/test';
import { drag, imageCanvas, menu, pixelCanvas, waitForEditor } from './helpers';
import {
  closeWindow,
  destroyed,
  emit,
  installTauriMock,
  putFile,
  releaseWrite,
  setMock,
  type MockWindow,
} from './tauri-mock';

// Desktop close guard and the OS hand-over of the unsaved-changes prompt
// (docs/specs/unsaved-changes-prompt.md AC-01, AC-08 – AC-11), against the
// mocked Tauri bridge of `tauri-mock.ts`. The real window behavior on each
// OS stays a manual check (AC-09, AC-10).

test.beforeEach(async ({ page }) => {
  // A reload with unsaved work asks the browser's leave-page question.
  page.on('dialog', (d) => void d.accept());
  await installTauriMock(page);
});

/** A valid 1×1 PNG. */
const PNG_1X1 = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==',
  'base64',
);

function prompt(page: Page): Locator {
  return page.getByRole('alertdialog', { name: 'Unsaved changes' });
}

/** Opens the app (past the first-launch file-association note). */
async function start(page: Page): Promise<void> {
  await page.goto('./');
  await page.getByRole('button', { name: 'Got it' }).click();
  await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
}

/** Creates a 16×16 document of `mode` from the start screen. */
async function create(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  const name = mode === 'pixel' ? 'New pixel sprite' : 'New image';
  await page.getByRole('button', { name }).click();
  const card = page.getByRole('form', { name });
  for (const label of ['Width', 'Height']) await card.getByLabel(label).fill('16');
  await card.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, mode);
}

async function paint(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  if (mode === 'pixel') {
    await page.getByTitle('Pencil (B)').click();
    await drag(page, pixelCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  } else {
    await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  }
}

test('closing with unsaved changes asks; Cancel keeps the app running (AC-09, AC-06)', async ({ page }) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  await closeWindow(page);
  await expect(prompt(page)).toBeVisible();
  await expect(prompt(page)).toContainText('Save changes to "pincel.aseprite"?');
  // A second close while the prompt is open is ignored (AC-08).
  await closeWindow(page);
  await expect(prompt(page)).toHaveCount(1);
  await prompt(page).getByRole('button', { name: 'Cancel' }).click();
  await expect(prompt(page)).toHaveCount(0);
  expect(await destroyed(page)).toBe(0);
  await expect(page.getByTitle('pincel.aseprite — unsaved changes')).toBeVisible();
});

test("Don't Save closes the app and recovery never offers the work (AC-09, AC-05)", async ({ page }) => {
  await page.clock.install();
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  // Let autosave write a snapshot first.
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);
  await closeWindow(page);
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  await expect.poll(() => destroyed(page)).toBe(1);

  // The next start offers nothing to recover.
  await start(page);
  await create(page, 'pixel');
  await page.waitForTimeout(500);
  await expect(page.getByRole('dialog', { name: 'Recover unsaved work?' })).toHaveCount(0);
});

test('Save closes the app once the save succeeded (AC-09, AC-03)', async ({ page }) => {
  await start(page);
  await create(page, 'image');
  await paint(page, 'image');
  await setMock(page, { savePick: '/docs/picture.ora' });
  await closeWindow(page);
  await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
  await expect.poll(() => destroyed(page)).toBe(1);
  expect(await page.evaluate(() => Object.keys((window as unknown as MockWindow).__mock.disk))).toEqual([
    '/docs/picture.ora',
  ]);
});

test('a dismissed save picker keeps the app open (AC-09, AC-04)', async ({ page }) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  await setMock(page, { savePick: null });
  await closeWindow(page);
  await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
  await expect(prompt(page)).toHaveCount(0);
  await page.waitForTimeout(200);
  expect(await destroyed(page)).toBe(0);
  await expect(page.getByTitle('pincel.aseprite — unsaved changes')).toBeVisible();
});

test('without unsaved changes, or on the start screen, the app closes at once (AC-09, AC-07)', async ({
  page,
}) => {
  await start(page);
  await closeWindow(page);
  await expect.poll(() => destroyed(page)).toBe(1);
  await create(page, 'pixel');
  await closeWindow(page);
  await expect.poll(() => destroyed(page)).toBe(2);
  await expect(prompt(page)).toHaveCount(0);
});

test('macOS Quit (menu:quit) asks like a window close (AC-09)', async ({ page }) => {
  await start(page);
  await create(page, 'image');
  await paint(page, 'image');
  await emit(page, 'menu', 'menu:quit');
  await expect(prompt(page)).toBeVisible();
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  await expect.poll(() => destroyed(page)).toBe(1);
});

test('a close during a running save waits for it, then closes (AC-10)', async ({ page }) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  await setMock(page, { writeMode: 'hold' });
  await page.keyboard.press('Control+s');
  await page.waitForFunction(() => (window as unknown as MockWindow).__mock.release !== null);
  await closeWindow(page);
  await page.waitForTimeout(200);
  await expect(prompt(page)).toHaveCount(0);
  expect(await destroyed(page)).toBe(0);
  await releaseWrite(page);
  await expect.poll(() => destroyed(page)).toBe(1);
  await expect(prompt(page)).toHaveCount(0);
});

test('a close during a save that fails asks after it (AC-10)', async ({ page }) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  await setMock(page, { writeMode: 'hold' });
  await page.keyboard.press('Control+s');
  await page.waitForFunction(() => (window as unknown as MockWindow).__mock.release !== null);
  await closeWindow(page);
  await setMock(page, { writeMode: 'fail' });
  await releaseWrite(page);
  await expect(prompt(page)).toBeVisible();
  expect(await destroyed(page)).toBe(0);
});

test('a file handed over by the OS asks first (AC-01)', async ({ page }) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  await putFile(page, '/docs/dot.png', PNG_1X1);
  await emit(page, 'open-file', '/docs/dot.png');
  await expect(prompt(page)).toBeVisible();
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  await waitForEditor(page, 'image');
  await expect(page.getByTitle('dot.png', { exact: true })).toBeVisible();
});

test('a handed-over file that cannot be read shows its error and no prompt (AC-11)', async ({ page }) => {
  await start(page);
  await create(page, 'image');
  await paint(page, 'image');
  await emit(page, 'open-file', '/docs/missing.png');
  await expect(page.getByText(/Open failed: .*No such file/)).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
  await expect(page.getByTitle('image.ora — unsaved changes')).toBeVisible();
});

/** Clicks the first File ▸ Open Recent entry named `name` (a recent
 *  re-opened through the shell is listed again under a new id). */
async function openRecentEntry(page: Page, name: string): Promise<void> {
  await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
  await page.getByRole('menu', { name: 'File', exact: true }).getByRole('menuitem', { name: 'Open Recent' }).click();
  await page
    .getByRole('menu', { name: 'Open Recent', exact: true })
    .getByRole('menuitem', { name, exact: true })
    .first()
    .click();
}

/** File ▸ New… of the other mode: Create replaces the session. */
async function newOfOtherMode(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await dialog.getByRole('radio', { name: mode === 'image' ? /^Image/ : /^Pixel sprite/ }).click();
  await dialog.getByRole('button', { name: 'Create' }).click();
}

test('Open Recent asks first, from both editors; a missing recent shows its error (AC-01, AC-11)', async ({
  page,
}) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  // Saving to a path records the recent.
  await page.keyboard.press('Control+s');
  await expect(page.getByTitle('sprite.aseprite', { exact: true })).toBeVisible();
  await paint(page, 'pixel');

  // Pixel mode's own File ▸ Open Recent of the file in the open tab: that
  // tab stays active with its changes, no prompt (document-tabs AC-03).
  await menu(page, 'File', 'Open Recent', 'sprite.aseprite');
  await expect(prompt(page)).toHaveCount(0);
  await expect(page.getByTitle('sprite.aseprite — unsaved changes')).toBeVisible();
  await expect(page.getByRole('tab')).toHaveCount(1);

  // Image mode routes Open Recent through the shell.
  await newOfOtherMode(page, 'image');
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  await waitForEditor(page, 'image');
  await paint(page, 'image');
  await openRecentEntry(page, 'sprite.aseprite');
  await expect(prompt(page)).toBeVisible();
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  await waitForEditor(page, 'pixel');
  await expect(page.getByTitle('sprite.aseprite', { exact: true })).toBeVisible();

  // A recent whose file is gone: error, no prompt.
  await newOfOtherMode(page, 'image');
  await waitForEditor(page, 'image');
  await paint(page, 'image');
  await putFile(page, '/docs/sprite.aseprite', null);
  await openRecentEntry(page, 'sprite.aseprite');
  await expect(page.getByText(/sprite\.aseprite.*No such file/)).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
  await expect(page.getByTitle('image.ora — unsaved changes')).toBeVisible();
});

test('a clean document opens a handed-over file and a recent at once (AC-07)', async ({ page }) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  await page.keyboard.press('Control+s');
  await expect(page.getByTitle('sprite.aseprite', { exact: true })).toBeVisible();

  // Pixel mode's own Open Recent of the open file activates its tab.
  await menu(page, 'File', 'Open Recent', 'sprite.aseprite');
  await expect(prompt(page)).toHaveCount(0);
  await expect(page.getByRole('tab')).toHaveCount(1);

  // The OS hands over a file.
  await putFile(page, '/docs/dot.png', PNG_1X1);
  await emit(page, 'open-file', '/docs/dot.png');
  await waitForEditor(page, 'image');
  await expect(page.getByTitle('dot.png', { exact: true })).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);

  // Image mode's Open Recent, through the shell.
  await openRecentEntry(page, 'sprite.aseprite');
  await waitForEditor(page, 'pixel');
  await expect(page.getByTitle('sprite.aseprite', { exact: true })).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
});

test('native-menu items reach no editor while the prompt is open (AC-02)', async ({ page }) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  await closeWindow(page);
  await expect(prompt(page)).toBeVisible();
  // macOS menu accelerators (Cmd+Z, Cmd+S, …) arrive as `menu` events.
  for (const id of ['menu:undo', 'menu:save', 'menu:saveAs', 'menu:exportPng', 'menu:zoomIn']) {
    await emit(page, 'menu', id);
  }
  await page.waitForTimeout(300);
  await expect(prompt(page)).toHaveCount(1);
  expect(await page.evaluate(() => Object.keys((window as unknown as MockWindow).__mock.disk))).toEqual([]);
  await prompt(page).getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByTitle('pincel.aseprite — unsaved changes')).toBeVisible();
  // The history is intact: one undo reaches the saved state.
  await emit(page, 'menu', 'menu:undo');
  await expect(page.getByTitle('pincel.aseprite', { exact: true })).toBeVisible();
});
