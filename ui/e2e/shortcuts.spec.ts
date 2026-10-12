import { expect, test, type Page } from '@playwright/test';
import { createDocument } from './helpers';

/** The tool-rail button that is currently pressed. */
function activeTool(page: Page) {
  return page.getByRole('toolbar', { name: 'Tools' }).locator('[aria-pressed="true"]');
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode} mode: the shared keymap selects tools and cycles variants`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await page.keyboard.press('q');
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Lasso: Lasso');
    await page.keyboard.press('q');
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Lasso: Polygonal Lasso');
    await page.keyboard.press('m');
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Marquee: Rectangle Select');
    // The options bar offers the group's variants.
    await page
      .getByRole('radiogroup', { name: 'Marquee variant' })
      .getByRole('radio', { name: 'Ellipse Select' })
      .click();
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Marquee: Ellipse Select');
    await page.keyboard.press('i');
    await expect(activeTool(page)).toHaveAttribute('aria-label', 'Eyedropper');

    // D resets to black / white, X swaps them.
    const fg = page.getByLabel('Foreground color', { exact: true });
    await page.keyboard.press('d');
    await expect(fg).toHaveValue('#000000');
    await page.keyboard.press('x');
    await expect(fg).toHaveValue('#ffffff');
  });
}
