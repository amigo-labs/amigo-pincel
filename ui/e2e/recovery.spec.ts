import { expect, test } from '@playwright/test';
import { createDocument, drag, imageCanvas, menu, pixelCanvas } from './helpers';

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

test('a discarded document is not offered again, on New or after a reload', async ({ page }) => {
  await page.clock.install();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByTitle('Pencil (B)').click();
  await drag(page, pixelCanvas(page), [0.2, 0.2], [0.8, 0.8]);
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);

  await menu(page, 'File', 'New…');
  await page.getByRole('button', { name: 'Discard changes' }).click();
  const newDialog = page.getByRole('dialog', { name: 'New document' });
  await newDialog.getByRole('button', { name: 'Create' }).click();
  await expect(newDialog).toBeHidden();
  const recovery = page.getByRole('dialog', { name: 'Recover unsaved work?' });
  // The probe is async; give it time to (not) open the dialog.
  await page.waitForTimeout(500);
  await expect(recovery).toHaveCount(0);

  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.waitForTimeout(500);
  await expect(recovery).toHaveCount(0);
});

test('"Not now" keeps the dialog away for the rest of the page load', async ({ page }) => {
  await page.clock.install();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await page.getByTitle('Pencil (B)').click();
  await drag(page, pixelCanvas(page), [0.2, 0.2], [0.8, 0.8]);
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);

  await page.reload();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const recovery = page.getByRole('dialog', { name: 'Recover unsaved work?' });
  await expect(recovery).toBeVisible();
  await recovery.getByRole('button', { name: 'Not now' }).click();
  await expect(recovery).toHaveCount(0);

  await menu(page, 'File', 'New…');
  const newDialog = page.getByRole('dialog', { name: 'New document' });
  await newDialog.getByRole('button', { name: 'Create' }).click();
  await expect(newDialog).toBeHidden();
  await page.waitForTimeout(500);
  await expect(recovery).toHaveCount(0);
});
