import { expect, test } from '@playwright/test';
import { createDocument } from './helpers';

// File ▸ Open Recent, in both modes (spec §9.2). Headless Chromium has
// the File System Access API but no way to drive its picker, so the list
// stays empty here: what this pins is that the entry exists and that an
// empty registry disables it rather than offering a dead submenu.

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: File lists Open Recent, disabled while empty`, async ({ page }) => {
    await createDocument(page, mode);
    await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
    const entry = page
      .getByRole('menu', { name: 'File', exact: true })
      .getByRole('menuitem', { name: 'Open Recent' });
    await expect(entry).toBeVisible();
    await expect(entry).toBeDisabled();
  });
}
