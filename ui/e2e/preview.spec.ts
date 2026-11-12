import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, pixelCanvas, switchLanguage } from './helpers';

// docs/specs/animation-preview.md: the Pixel-mode Preview panel.

const previewOf = (page: Page) => page.getByRole('region', { name: 'Preview' });
const previewCanvas = (page: Page) => previewOf(page).getByLabel('Animation preview');
const timelineOf = (page: Page) => page.getByRole('region', { name: 'Timeline' });
const currentFrame = (page: Page) => timelineOf(page).locator('[aria-current="true"]');

/** Expands the collapsed Preview panel. */
async function openPreview(page: Page) {
  const header = previewOf(page).getByRole('button', { name: 'Preview', exact: true });
  await expect(header).toHaveAttribute('aria-expanded', 'false');
  await header.click();
  await expect(previewCanvas(page)).toBeVisible();
}

/** Adds `n` frames through the timeline. */
async function addFrames(page: Page, n: number) {
  for (let i = 0; i < n; i += 1) {
    await timelineOf(page).getByRole('button', { name: 'Add frame' }).click();
  }
}

/** Paints a horizontal pencil line across frame `n` (1-based). */
async function paintFrame(page: Page, n: number, y = 0.5) {
  await timelineOf(page)
    .getByRole('button', { name: `Frame ${n}`, exact: true })
    .click();
  await page.getByTitle('Pencil (B)').click();
  await drag(page, pixelCanvas(page), [0.1, y], [0.9, y]);
}

/** Starts recording every `data-frame` the preview shows, with timestamps. */
async function recordFrames(page: Page) {
  await page.evaluate(() => {
    const el = document.querySelector('canvas[aria-label="Animation preview"]');
    if (!el) throw new Error('no preview canvas');
    const w = window as { previewLog?: { frame: number; at: number }[] };
    w.previewLog = [{ frame: Number(el.getAttribute('data-frame')), at: performance.now() }];
    new MutationObserver(() => {
      w.previewLog?.push({ frame: Number(el.getAttribute('data-frame')), at: performance.now() });
    }).observe(el, { attributes: true, attributeFilter: ['data-frame'] });
  });
}

async function recorded(page: Page): Promise<{ frame: number; at: number }[]> {
  return page.evaluate(
    () => (window as { previewLog?: { frame: number; at: number }[] }).previewLog ?? [],
  );
}

/** Distinct `a→b` transitions in a recorded frame log. */
function transitions(log: { frame: number }[]): Set<string> {
  const out = new Set<string>();
  log.slice(1).forEach((next, i) => {
    const prev = log[i];
    if (prev && next.frame !== prev.frame) out.add(`${prev.frame}->${next.frame}`);
  });
  return out;
}

test('the Preview panel sits above Layers, collapsed, in Pixel mode only (AC-01)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const header = previewOf(page).getByRole('button', { name: 'Preview', exact: true });
  await expect(header).toHaveAttribute('aria-expanded', 'false');
  const preview = await previewOf(page).boundingBox();
  const layers = await page.getByRole('region', { name: 'Layers' }).boundingBox();
  if (!preview || !layers) throw new Error('panels not visible');
  expect(preview.y).toBeLessThan(layers.y);

  await createDocument(page, 'image', { width: 16, height: 16 });
  await expect(page.getByRole('region', { name: 'Layers' })).toBeVisible();
  await expect(previewOf(page)).toHaveCount(0);
});

test('paused, the preview follows the current frame (AC-02, AC-12)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await addFrames(page, 2);
  await openPreview(page);
  await expect(previewCanvas(page)).toHaveAttribute('data-frame', '2');
  await timelineOf(page).getByRole('button', { name: 'Frame 1', exact: true }).click();
  await expect(previewCanvas(page)).toHaveAttribute('data-frame', '0');
  // Main-canvas playback still drives the timeline, and the paused
  // preview follows it.
  await timelineOf(page).getByRole('button', { name: 'Play' }).click();
  await expect(currentFrame(page)).toHaveText('2');
  await expect(previewCanvas(page)).toHaveAttribute('data-frame', '1');
  await timelineOf(page).getByRole('button', { name: 'Pause' }).click();
  // Enter toggles main playback too.
  const shown = await currentFrame(page).textContent();
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.keyboard.press('Enter');
  await expect(currentFrame(page)).not.toHaveText(shown ?? '');
  await page.keyboard.press('Enter');
  await expect(timelineOf(page).getByRole('button', { name: 'Play' })).toBeVisible();
  const stopped = await currentFrame(page).textContent();
  await expect(previewCanvas(page)).toHaveAttribute('data-frame', String(Number(stopped) - 1));
});

test('playing, the preview advances while the timeline stays (AC-03, AC-04)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await addFrames(page, 2);
  await timelineOf(page).getByRole('button', { name: 'Frame 2', exact: true }).click();
  await openPreview(page);
  await recordFrames(page);
  await previewOf(page).getByRole('button', { name: 'Play preview' }).click();
  await expect(previewOf(page).getByRole('button', { name: 'Pause preview' })).toBeVisible();
  await expect.poll(async () => new Set((await recorded(page)).map((e) => e.frame)).size).toBe(3);
  await expect(currentFrame(page)).toHaveText('2');

  // A stroke while the preview plays lands on frame 2, as one undo step
  // (after the two Add frame steps).
  await expect(page.getByText('undo 2 / redo 0', { exact: true })).toBeVisible();
  await page.getByTitle('Pencil (B)').click();
  await drag(page, pixelCanvas(page), [0.1, 0.5], [0.9, 0.5]);
  await expect(page.getByText('undo 3 / redo 0', { exact: true })).toBeVisible();
  await expect(currentFrame(page)).toHaveText('2');
  await previewOf(page).getByRole('button', { name: 'Pause preview' }).click();
  await expect(previewCanvas(page)).toHaveAttribute('data-frame', '1');
  const painted = await previewCanvas(page).screenshot();
  await timelineOf(page).getByRole('button', { name: 'Frame 1', exact: true }).click();
  await expect(previewCanvas(page)).toHaveAttribute('data-frame', '0');
  await expect
    .poll(async () => (await previewCanvas(page).screenshot()).equals(painted))
    .toBe(false);
});

test('the paused preview shows document changes at once (AC-05)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await openPreview(page);
  const before = await previewCanvas(page).screenshot();
  await page.getByTitle('Pencil (B)').click();
  await drag(page, pixelCanvas(page), [0.1, 0.5], [0.9, 0.5]);
  await expect
    .poll(async () => (await previewCanvas(page).screenshot()).equals(before))
    .toBe(false);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect.poll(async () => (await previewCanvas(page).screenshot()).equals(before)).toBe(true);
});

test('preview playback follows the loop tag and its direction (AC-06)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await addFrames(page, 3);
  const timeline = timelineOf(page);
  await openPreview(page);
  const play = previewOf(page).getByRole('button', { name: 'Play preview' });
  const pause = previewOf(page).getByRole('button', { name: 'Pause preview' });

  // Without a loop tag: every frame, forward.
  await recordFrames(page);
  await play.click();
  await expect.poll(async () => transitions(await recorded(page)).size).toBeGreaterThanOrEqual(4);
  // Read the log before pausing: pausing returns to the current frame.
  const all = await recorded(page);
  await pause.click();
  expect([...transitions(all)].sort()).toEqual(['0->1', '1->2', '2->3', '3->0']);

  // A tag over frames 2–4 (indices 1–3).
  await timeline.getByRole('button', { name: 'Frame 2', exact: true }).click();
  await timeline.getByRole('button', { name: 'Add tag' }).click();
  const editor = timeline.getByLabel('Tag editor');
  await editor.getByLabel('Tag name').fill('walk');
  await editor.getByLabel('Tag name').press('Enter');
  await editor.getByLabel('Tag to').fill('4');
  await editor.getByLabel('Tag to').press('Enter');
  await timeline.getByLabel('Loop range').selectOption({ label: 'walk' });

  for (const [direction, expected] of [
    ['forward', ['1->2', '2->3', '3->1']],
    ['reverse', ['1->3', '2->1', '3->2']],
    ['pingpong', ['1->2', '2->1', '2->3', '3->2']],
  ] as const) {
    await editor.getByLabel('Tag direction').selectOption(direction);
    await timeline.getByRole('button', { name: 'Frame 3', exact: true }).click();
    await recordFrames(page);
    await play.click();
    await expect
      .poll(async () => transitions(await recorded(page)).size)
      .toBeGreaterThanOrEqual(expected.length);
    const log = await recorded(page);
    await pause.click();
    expect(
      log.every((e) => e.frame >= 1 && e.frame <= 3),
      direction,
    ).toBe(true);
    expect([...transitions(log)].sort(), direction).toEqual(expected);
  }
});

test('each frame stays shown for its duration (AC-07)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await addFrames(page, 1);
  const timeline = timelineOf(page);
  for (const [n, ms] of [
    [1, '100'],
    [2, '1000'],
  ] as const) {
    const field = timeline.getByLabel(`Frame ${n} duration (ms)`);
    await field.fill(ms);
    await field.press('Enter');
    await expect(field).toHaveValue(ms);
  }
  await openPreview(page);
  await recordFrames(page);
  await previewOf(page).getByRole('button', { name: 'Play preview' }).click();
  await expect
    .poll(async () => (await recorded(page)).length, { timeout: 10_000 })
    .toBeGreaterThan(5);
  const log = await recorded(page);
  await previewOf(page).getByRole('button', { name: 'Pause preview' }).click();
  // Time spent on each frame between complete changes.
  // The first entry is the paused frame, not a complete span.
  const spans = new Map<number, number[]>([
    [0, []],
    [1, []],
  ]);
  log.slice(1, -1).forEach((entry, i) => {
    const next = log[i + 2];
    if (next) spans.get(entry.frame)?.push(next.at - entry.at);
  });
  const avg = (xs: number[] = []) => xs.reduce((a, b) => a + b, 0) / Math.max(1, xs.length);
  expect(spans.get(0)?.length).toBeGreaterThan(0);
  expect(spans.get(1)?.length).toBeGreaterThan(0);
  expect(avg(spans.get(1))).toBeGreaterThan(avg(spans.get(0)) * 4);
});

test('the preview never shows onion-skin ghosts (AC-08)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await addFrames(page, 1);
  await paintFrame(page, 1, 0.3);
  await timelineOf(page).getByRole('button', { name: 'Frame 2', exact: true }).click();
  await openPreview(page);
  await expect(previewCanvas(page)).toHaveAttribute('data-frame', '1');
  await page.mouse.move(0, 0);
  const canvas = pixelCanvas(page);
  const mainOff = await canvas.screenshot();
  const off = await previewCanvas(page).screenshot();
  await timelineOf(page).getByLabel('Onion skin').check();
  // The main canvas shows the ghost of frame 1 …
  await expect.poll(async () => (await canvas.screenshot()).equals(mainOff)).toBe(false);
  // … the preview does not.
  expect((await previewCanvas(page).screenshot()).equals(off)).toBe(true);
});

test('preview zoom: 1×, 2×, 4× and Fit (AC-10)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 8 });
  await openPreview(page);
  const zoom = previewOf(page).getByRole('group', { name: 'Preview zoom' });
  for (const factor of [1, 2, 4]) {
    await zoom.getByRole('button', { name: `${factor}×`, exact: true }).click();
    await expect(zoom.getByRole('button', { name: `${factor}×`, exact: true })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await expect
      .poll(async () => {
        const box = await previewCanvas(page).boundingBox();
        return box ? [Math.round(box.width), Math.round(box.height)] : null;
      })
      .toEqual([16 * factor, 8 * factor]);
  }
  await zoom.getByRole('button', { name: 'Fit', exact: true }).click();
  const stage = previewOf(page).getByTestId('preview-stage');
  await expect
    .poll(async () => {
      const box = await previewCanvas(page).boundingBox();
      const area = await stage.boundingBox();
      if (!box || !area) return false;
      const ratio = Math.abs(box.width / box.height - 2) < 0.05;
      const fitsW = box.width <= area.width + 0.5;
      const fitsH = box.height <= area.height + 0.5;
      const touches =
        Math.abs(box.width - area.width) < 1.5 || Math.abs(box.height - area.height) < 1.5;
      return ratio && fitsW && fitsH && touches && box.width > 64;
    })
    .toBe(true);
});

test('collapsing the panel stops preview playback (AC-13)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await addFrames(page, 2);
  await openPreview(page);
  await previewOf(page).getByRole('button', { name: 'Play preview' }).click();
  const header = previewOf(page).getByRole('button', { name: 'Preview', exact: true });
  await header.click();
  await expect(header).toHaveAttribute('aria-expanded', 'false');
  await header.click();
  await expect(previewOf(page).getByRole('button', { name: 'Play preview' })).toBeVisible();
  await expect(previewCanvas(page)).toHaveAttribute('data-frame', '2');
  await recordFrames(page);
  await page.waitForTimeout(400);
  expect(transitions(await recorded(page)).size).toBe(0);
});

test('deleting the shown frame or the loop tag keeps the preview going (AC-15)', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await addFrames(page, 3);
  const timeline = timelineOf(page);
  await timeline.getByRole('button', { name: 'Frame 3', exact: true }).click();
  await timeline.getByRole('button', { name: 'Add tag' }).click();
  const editor = timeline.getByLabel('Tag editor');
  await editor.getByLabel('Tag name').fill('end');
  await editor.getByLabel('Tag name').press('Enter');
  await editor.getByLabel('Tag to').fill('4');
  await editor.getByLabel('Tag to').press('Enter');
  await timeline.getByLabel('Loop range').selectOption({ label: 'end' });
  await openPreview(page);
  await previewOf(page).getByRole('button', { name: 'Play preview' }).click();
  // The preview loops over frames 3–4; remove frame 4, then frame 3.
  for (const n of [4, 3]) {
    await timeline.getByRole('button', { name: `Frame ${n}`, exact: true }).click();
    await timeline.getByRole('button', { name: 'Remove frame' }).click();
    await expect(timeline.getByRole('button', { name: `Frame ${n}`, exact: true })).toHaveCount(0);
  }
  // Delete the loop tag (if it survived) while playing.
  const tag = timeline.getByRole('button', { name: /^Tag / });
  if ((await tag.count()) > 0) {
    await tag.first().click();
    await timeline.getByLabel('Tag editor').getByRole('button', { name: 'Delete tag' }).click();
    await expect(tag).toHaveCount(0);
  }
  await recordFrames(page);
  await page.waitForTimeout(500);
  const log = await recorded(page);
  expect(log.every((e) => e.frame >= 0 && e.frame <= 1)).toBe(true);
  await expect(previewOf(page).getByRole('button', { name: 'Pause preview' })).toBeVisible();
  expect(errors).toEqual([]);
  await expect(page.getByText(/failed/)).toHaveCount(0);
});

test('German labels (AC-16)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await switchLanguage(page, 'Deutsch');
  const panel = page.getByRole('region', { name: 'Vorschau' });
  await panel.getByRole('button', { name: 'Vorschau', exact: true }).click();
  await expect(panel.getByRole('button', { name: 'Vorschau abspielen' })).toBeVisible();
  const zoom = panel.getByRole('group', { name: 'Vorschau-Zoom' });
  await expect(zoom.getByRole('button', { name: 'Einpassen' })).toBeVisible();
  await panel.getByRole('button', { name: 'Vorschau abspielen' }).click();
  await expect(panel.getByRole('button', { name: 'Vorschau anhalten' })).toBeVisible();
});

test.describe('phone', () => {
  test.use({ viewport: { width: 390, height: 844 } });

  test('the Preview panel is a sheet tab (AC-14)', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await page.getByRole('button', { name: 'Panels' }).click();
    const sheet = page.getByRole('dialog', { name: 'Panels' });
    await sheet.getByRole('tab', { name: 'Preview' }).click();
    await expect(sheet.getByLabel('Animation preview')).toBeVisible();
    await expect(sheet.getByRole('button', { name: 'Play preview' })).toBeVisible();
  });
});
