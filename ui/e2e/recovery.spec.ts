import { expect, test } from '@playwright/test';
import { createDocument, drag, imageCanvas, pixelCanvas } from './helpers';

// Autosave and recovery (spec §10, M10.3), the same in both modes: an
// edit is snapshotted after 30 s, a reload offers it in the Recovery
// dialog, and the recovered document reads as unsaved.

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: an autosaved edit is offered and recovered after a reload`, async ({ page }) => {
    await page.clock.install();
    await createDocument(page, mode, { width: 32, height: 32 });
    if (mode === 'pixel') await page.getByTitle('Pencil (B)').click();
    await drag(
      page,
      mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page),
      [0.2, 0.2],
      [0.8, 0.6],
    );
    await page.clock.fastForward(31_000);
    // Let the IndexedDB write settle before leaving the page.
    await page.waitForTimeout(500);

    await page.reload();
    await createDocument(page, mode, { width: 16, height: 16 });
    const dialog = page.getByRole('dialog', { name: 'Recover unsaved work?' });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Recover' }).first().click();
    await expect(dialog).toHaveCount(0);
    await expect(page.getByTitle(/unsaved changes/)).toBeVisible();
  });
}

test('discarding the last snapshot closes the dialog', async ({ page }) => {
  await page.clock.install();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByTitle('Pencil (B)').click();
  await drag(page, pixelCanvas(page), [0.2, 0.2], [0.8, 0.8]);
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);

  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const dialog = page.getByRole('dialog', { name: 'Recover unsaved work?' });
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Discard' }).click();
  await expect(dialog).toHaveCount(0);
});
