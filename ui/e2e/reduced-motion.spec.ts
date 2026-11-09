import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, gotoStart, imageCanvas, pixelCanvas, storedPref } from './helpers';

// "Reduce motion" (docs/specs/reduced-motion.md): the selection outline and
// the other marching outlines stand still while the system or View ▸ Reduce
// Motion asks for it, UI transitions finish instantly, playback keeps
// running. The canvas is compared through series of element screenshots
// (AGENTS.md §7.4); the app never reads canvas pixels.

const MODES = ['pixel', 'image'] as const;
type Mode = (typeof MODES)[number];

const SIZE = 32;
const LABEL = 'Reduce Motion';
const SYSTEM_LABEL = 'Reduce Motion (on in system settings)';

const canvasOf = (page: Page, mode: Mode): Locator =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);

/** Screen point of the document position `(x, y)`. */
async function point(page: Page, mode: Mode, x: number, y: number): Promise<[number, number]> {
  const box = await canvasOf(page, mode).boundingBox();
  if (!box) throw new Error('canvas not visible');
  return [box.x + (x * box.width) / SIZE, box.y + (y * box.height) / SIZE];
}

/** Starts a Rectangle Select drag from `a` to `b` (document units); `release` ends it. */
async function selectionDrag(
  page: Page,
  mode: Mode,
  a: [number, number],
  b: [number, number],
  release = true,
): Promise<void> {
  await page.keyboard.press('m');
  await page.mouse.move(...(await point(page, mode, ...a)));
  await page.mouse.down();
  await page.mouse.move(...(await point(page, mode, ...b)), { steps: 4 });
  if (release) {
    await page.mouse.up();
    await page.mouse.move(1, 1);
  }
}

/** An element screenshot of the canvas (overlays included). */
const shot = (page: Page, mode: Mode) => canvasOf(page, mode).screenshot();

/** Whether a series of canvas screenshots over about a second are identical. */
async function looksStill(page: Page, mode: Mode): Promise<boolean> {
  // The ants cycle: Pixel mode steps every 7 frames (~117 ms) through 4
  // phases (~467 ms), Image mode every frame through ~267 ms. Shots spaced
  // close to a whole cycle all land on one phase and a marching outline
  // looks still: 300 ms waits plus the CI runner's ~150 ms per screenshot
  // did exactly that. Short waits keep every gap (screenshot included) well
  // under three steps, and the series spans more than a cycle, so a march
  // always shows as a difference.
  const first = await shot(page, mode);
  for (const wait of [40, 90, 60, 110, 70, 130]) {
    await page.waitForTimeout(wait);
    if (!(await shot(page, mode)).equals(first)) return false;
  }
  return true;
}

/** Waits until the canvas stands still (`true`) or moves (`false`). */
async function expectStill(page: Page, mode: Mode, still: boolean): Promise<void> {
  await expect.poll(() => looksStill(page, mode), { timeout: 10_000 }).toBe(still);
}

/** A document of `mode` with a rectangular selection whose outline is drawn. */
async function withSelection(page: Page, mode: Mode): Promise<void> {
  await createDocument(page, mode, { width: SIZE, height: SIZE });
  const empty = await shot(page, mode);
  await selectionDrag(page, mode, [8, 8], [24, 24]);
  await expect.poll(async () => (await shot(page, mode)).equals(empty)).toBe(false);
}

/** The View ▸ Reduce Motion entry (whichever label it carries). */
async function openViewMenu(page: Page): Promise<Locator> {
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  return page
    .getByRole('menu', { name: 'View', exact: true })
    .getByRole('menuitemcheckbox', { name: /^Reduce Motion/ });
}

/** The start screen's chevron and its computed transition / animation durations (ms). */
async function chevronDurations(page: Page): Promise<number[]> {
  const chevron = page
    .getByRole('button', { name: 'New pixel sprite', exact: true })
    .locator('svg')
    .last();
  return chevron.evaluate((el) => {
    const s = getComputedStyle(el);
    const ms = (v: string) =>
      Math.max(
        ...v
          .split(',')
          .map((d) => (d.trim().endsWith('ms') ? parseFloat(d) : parseFloat(d) * 1000)),
      );
    return [ms(s.transitionDuration), ms(s.animationDuration)];
  });
}

for (const mode of MODES) {
  test(`${mode}: under reduced motion the selection outline stands still (AC-01)`, async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await withSelection(page, mode);
    expect(await looksStill(page, mode)).toBe(true);
  });

  test(`${mode}: without reduced motion the selection outline marches (AC-02, AC-11)`, async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await withSelection(page, mode);
    await expectStill(page, mode, false);
    // A new profile follows the system: the option is off.
    const entry = await openViewMenu(page);
    await expect(entry).toHaveAccessibleName(LABEL);
    await expect(entry).toHaveAttribute('aria-checked', 'false');
  });

  test(`${mode}: under reduced motion a changed selection shows at its new place (AC-04)`, async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await withSelection(page, mode);
    const before = await shot(page, mode);
    await selectionDrag(page, mode, [2, 2], [12, 20]);
    await expect.poll(async () => (await shot(page, mode)).equals(before)).toBe(false);
    expect(await looksStill(page, mode)).toBe(true);
  });

  test(`${mode}: a switch of the system setting stops and starts the march without a reload (AC-05)`, async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await withSelection(page, mode);
    await expectStill(page, mode, false);
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await expectStill(page, mode, true);
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await expectStill(page, mode, false);
  });

  test(`${mode}: View ▸ Reduce Motion stills the outline without the system setting (AC-09)`, async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await withSelection(page, mode);
    await expectStill(page, mode, false);
    await (await openViewMenu(page)).click();
    await expectStill(page, mode, true);
    // AC-04 under the option: a new selection is drawn, still static.
    const before = await shot(page, mode);
    await selectionDrag(page, mode, [2, 2], [12, 20]);
    await expect.poll(async () => (await shot(page, mode)).equals(before)).toBe(false);
    expect(await looksStill(page, mode)).toBe(true);
  });

  test(`${mode}: under a system reduce request the entry stays unchecked and names the system setting (AC-13)`, async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await createDocument(page, mode, { width: SIZE, height: SIZE });
    const entry = await openViewMenu(page);
    await expect(entry).toHaveAccessibleName(SYSTEM_LABEL);
    await expect(entry).toHaveAttribute('aria-checked', 'false');
    await page.keyboard.press('Escape');
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await expect(await openViewMenu(page)).toHaveAccessibleName(LABEL);
  });
}

test('pixel: under reduced motion a drag preview and the active slice outline stand still (AC-03, AC-09)', async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await createDocument(page, 'pixel', { width: SIZE, height: SIZE });
  // A Rectangle Select drag that is still in progress.
  await selectionDrag(page, 'pixel', [4, 4], [20, 20], false);
  expect(await looksStill(page, 'pixel')).toBe(true);
  await page.mouse.up();
  await page.keyboard.press('ControlOrMeta+d');
  // A new slice is the active one; its outline marches unless reduced.
  await page
    .getByRole('toolbar', { name: 'Tools' })
    .getByRole('button', { name: 'Slice', exact: true })
    .click();
  await page.mouse.move(...(await point(page, 'pixel', 6, 6)));
  await page.mouse.down();
  await page.mouse.move(...(await point(page, 'pixel', 22, 18)), { steps: 4 });
  await page.mouse.up();
  await page.mouse.move(1, 1);
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await expectStill(page, 'pixel', false);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await expectStill(page, 'pixel', true);
  // The in-app option does the same without the system setting (AC-09).
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await (await openViewMenu(page)).click();
  await expectStill(page, 'pixel', true);
});

test('pixel: frame playback still advances under reduced motion (AC-07)', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Frame 1', exact: true }).click();
  await expect(timeline.getByText('frame 1/3')).toBeVisible();
  await timeline.getByRole('button', { name: 'Play' }).click();
  await expect(timeline.getByText('frame 2/3')).toBeVisible();
  await expect(timeline.getByText('frame 3/3')).toBeVisible();
  await timeline.getByRole('button', { name: 'Pause' }).click();
});

test('UI transitions finish instantly under reduced motion, from the system or the option (AC-06, AC-09, AC-10)', async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await gotoStart(page);
  const [transition] = await chevronDurations(page);
  expect(transition, 'the chevron animates normally').toBeGreaterThan(0.01);

  await page.emulateMedia({ reducedMotion: 'reduce' });
  for (const d of await chevronDurations(page)) expect(d).toBeLessThanOrEqual(0.01);

  // The option, saved and restored on reload, does the same without the system.
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  await (await openViewMenu(page)).click();
  await expect.poll(() => storedPref(page, 'reduceMotion')).toBe(true);
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
  await expect.poll(async () => (await chevronDurations(page))[0]).toBeLessThanOrEqual(0.01);
  for (const d of await chevronDurations(page)) expect(d).toBeLessThanOrEqual(0.01);
  await createDocument(page, 'image', { width: 8, height: 8 });
  await expect(await openViewMenu(page)).toHaveAttribute('aria-checked', 'true');
  await page.keyboard.press('Escape');

  // Unchecking is kept too.
  await (await openViewMenu(page)).click();
  await expect.poll(() => storedPref(page, 'reduceMotion')).toBe(false);
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  await expect(await openViewMenu(page)).toHaveAttribute('aria-checked', 'false');
});

test('View ▸ Reduce Motion is German in the German UI (AC-12)', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  await page.getByRole('menubar').getByRole('menuitem', { name: 'Ansicht', exact: true }).click();
  const view = page.getByRole('menu', { name: 'Ansicht', exact: true });
  await expect(
    view.getByRole('menuitemcheckbox', { name: /^Bewegung reduzieren/ }),
  ).toHaveAccessibleName('Bewegung reduzieren (in den Systemeinstellungen an)');
  await page.keyboard.press('Escape');
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'Ansicht', exact: true }).click();
  await expect(
    view.getByRole('menuitemcheckbox', { name: /^Bewegung reduzieren/ }),
  ).toHaveAccessibleName('Bewegung reduzieren');
});
