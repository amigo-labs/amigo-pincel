import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, imageCanvas } from './helpers';
import { expectRows, openHistory } from './history-helpers';

// Image mode's marching ants trace the selection mask, which the overlay
// reads from the core. Pixel edits never change the selection, so a brush
// stroke inside one must not re-read (and re-trace) the canvas-sized mask
// on every frame; a selection change still does.

type DiagnosticsWindow = Window & {
  __pincelDiagnostics?: { calls?: Record<string, number> };
};

/** How often the Image core's selection mask has been read. */
function maskReads(page: Page): Promise<number> {
  return page.evaluate(
    () => (window as DiagnosticsWindow).__pincelDiagnostics?.calls?.['image.selectionMask'] ?? 0,
  );
}

/** Resolves after two animation frames, once queued redraws have run. */
function frames(page: Page): Promise<void> {
  return page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
      }),
  );
}

test('image: a brush stroke inside a selection does not re-read the selection mask', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 256, height: 256 });
  await openHistory(page);
  const initial = await maskReads(page);

  await page.keyboard.press('ControlOrMeta+a');
  await expectRows(page, ['New', 'Select All']);
  await expect.poll(() => maskReads(page)).toBeGreaterThan(initial);
  await frames(page);
  const selected = await maskReads(page);

  await page.keyboard.press('b');
  await drag(page, imageCanvas(page), [0.2, 0.3], [0.8, 0.6]);
  await expectRows(page, ['New', 'Select All', 'Brush']);
  await frames(page);
  expect(await maskReads(page), 'mask reads during and after the stroke').toBe(selected);

  // Undoing the stroke keeps the selection too.
  await page.keyboard.press('ControlOrMeta+z');
  await frames(page);
  expect(await maskReads(page), 'mask reads after undoing the stroke').toBe(selected);

  // A selection change still rebuilds the outline.
  await page.keyboard.press('ControlOrMeta+d');
  await expect.poll(() => maskReads(page)).toBeGreaterThan(selected);
});
