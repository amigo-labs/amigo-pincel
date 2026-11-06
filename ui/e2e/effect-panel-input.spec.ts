import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  copiedImage,
  clearClipboard,
  createDocument,
  drag,
  menu,
  pixelOf,
  waitForEditor,
} from './helpers';
import { brightness, canvasOf, centre, fill } from './effect-panel-helpers';

// What the editor does while an effect panel is open (effect-panel AC-03,
// AC-16, AC-17): the regions around the canvas area and the top bar, all
// but the Effects and Adjustments menus, are dimmed and inert and the
// canvas only zooms and pans; a failing effect keeps the panel; an autosave
// never holds the preview. Saving, closing and opening while an effect is
// open go through the desktop app's native menu: effect-panel-desktop.spec.ts.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const invertPanel = (page: Page) => page.getByRole('dialog', { name: 'Invert Colors' });

async function openInvert(page: Page): Promise<Locator> {
  await menu(page, 'Adjustments', 'Invert Colors');
  const panel = invertPanel(page);
  await expect(panel).toBeVisible();
  return panel;
}

for (const mode of ['pixel', 'image'] as const) {
  test(`${mode}: other panels are inert and the canvas only zooms and pans (AC-03)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    // The zoom below reaches the pixel grid's zoom, and the canvas centre of
    // an even-sized sprite is a grid crossing: hide the grid, so the centre
    // shows only the image (canvas screenshots include overlays).
    if (mode === 'pixel') {
      await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
      await page.getByRole('menuitemcheckbox', { name: 'Show Pixel Grid' }).click();
    }
    const layers = page.getByRole('list', { name: 'Layer list' });
    await page.getByRole('button', { name: 'Add layer' }).click();
    await expect(layers.getByRole('listitem')).toHaveCount(2);
    await fill(page, mode);
    const filled = await centre(page, mode);
    const lower = layers
      .getByRole('listitem')
      .nth(1)
      .locator('button[aria-pressed]:not(.row-icon)');
    await expect(lower).toHaveAttribute('aria-pressed', 'false');
    // The Eraser is armed: a stroke would show on the canvas.
    const tools = page.getByRole('toolbar', { name: 'Tools' });
    await tools.getByRole('button', { name: 'Eraser' }).click();
    const regions = {
      tools,
      options: page.getByRole('toolbar', { name: 'Tool options' }),
      tabs: page.getByRole('tablist', { name: 'Open documents' }),
      sidebar: page.getByRole('complementary', { name: 'Panels' }),
      status: page.getByRole('contentinfo'),
    };
    const lit: Record<string, number> = {};
    for (const [name, region] of Object.entries(regions)) lit[name] = await brightness(page, region);
    // A strip of stage left of the canvas, clear of the panel.
    const area = await page.getByRole('main').boundingBox();
    const canvasBox = await canvasOf(page, mode).boundingBox();
    if (!area || !canvasBox) throw new Error('canvas area is not visible');
    const stageClip = {
      x: area.x + 2,
      y: area.y + area.height - 24,
      width: Math.max(4, Math.min(20, canvasBox.x - area.x - 4)),
      height: 20,
    };
    const stage = await page.screenshot({ clip: stageClip });

    await openInvert(page);
    await expect.poll(async () => (await centre(page, mode)).equals(filled)).toBe(false);
    const preview = await centre(page, mode);

    // Around the canvas area: dimmed; the canvas area itself is not.
    for (const [name, region] of Object.entries(regions)) {
      expect(await brightness(page, region), name).toBeLessThan((lit[name] ?? 0) * 0.8);
    }
    expect((await page.screenshot({ clip: stageClip })).equals(stage)).toBe(true);

    // Tool rail, options bar and tab bar take no focus.
    for (const control of [
      tools.getByRole('button', { name: 'Eraser' }),
      page.getByRole('tab', { selected: true }),
    ]) {
      await control.focus();
      await expect(control).not.toBeFocused();
    }

    // Other panels: visible, but no focus and no clicks.
    await expect(layers).toBeVisible();
    await lower.focus();
    await expect(lower).not.toBeFocused();
    await lower.click({ force: true });
    await expect(lower).toHaveAttribute('aria-pressed', 'false');
    const swap = page.getByRole('button', { name: 'Swap foreground and background colors' });
    await swap.focus();
    await expect(swap).not.toBeFocused();
    if (mode === 'pixel') {
      // The timeline docked under the canvas is inert too.
      const addFrame = page.getByRole('button', { name: 'Add frame' });
      await expect(addFrame).toBeVisible();
      await addFrame.focus();
      await expect(addFrame).not.toBeFocused();
    }

    // Drawing tools don't reach the canvas; Undo stays blocked.
    await drag(page, canvasOf(page, mode), [0.3, 0.5], [0.7, 0.5]);
    await page.keyboard.press('Control+z');

    // Zoom: wheel and the zoom control; the preview stays shown.
    const canvas = canvasOf(page, mode);
    const before = await canvas.boundingBox();
    if (!before) throw new Error('canvas is not visible');
    await page.mouse.move(before.x + before.width / 2, before.y + before.height / 2);
    await page.mouse.wheel(0, -300);
    await expect
      .poll(async () => (await canvas.boundingBox())?.width ?? 0)
      .toBeGreaterThan(before.width);
    const zoomed = (await canvas.boundingBox())?.width ?? 0;
    await page.getByRole('button', { name: 'Zoom out', exact: true }).click();
    await expect.poll(async () => (await canvas.boundingBox())?.width ?? 0).toBeLessThan(zoomed);
    expect((await centre(page, mode)).equals(preview)).toBe(true);

    // Pan: middle-button drag, then a drag with Space held.
    const panned = async (move: () => Promise<void>) => {
      const from = await canvas.boundingBox();
      if (!from) throw new Error('canvas is not visible');
      await move();
      await expect
        .poll(async () => Math.round((await canvas.boundingBox())?.x ?? 0))
        .toBe(Math.round(from.x + 40));
    };
    const box = await canvas.boundingBox();
    if (!box) throw new Error('canvas is not visible');
    const [cx, cy] = [box.x + box.width / 2, box.y + box.height / 2];
    await panned(async () => {
      await page.mouse.move(cx, cy);
      await page.mouse.down({ button: 'middle' });
      await page.mouse.move(cx + 20, cy, { steps: 3 });
      await page.mouse.move(cx + 40, cy, { steps: 3 });
      await page.mouse.up({ button: 'middle' });
    });
    await panned(async () => {
      await page.mouse.move(cx + 40, cy);
      await page.keyboard.down('Space');
      await page.mouse.down();
      await page.mouse.move(cx + 60, cy, { steps: 3 });
      await page.mouse.move(cx + 80, cy, { steps: 3 });
      await page.mouse.up();
      await page.keyboard.up('Space');
    });
    expect((await centre(page, mode)).equals(preview)).toBe(true);

    // Cancel: no erase stroke and no undo happened.
    await invertPanel(page).getByRole('button', { name: 'Cancel' }).click();
    await expect.poll(async () => (await centre(page, mode)).equals(filled)).toBe(true);
    await expect(lower).toHaveAttribute('aria-pressed', 'false');
    await lower.click();
    await expect(lower).toHaveAttribute('aria-pressed', 'true');
  });

  test(`${mode}: a failing effect keeps the panel and its values (AC-16)`, async ({ page }) => {
    await createDocument(page, mode, { width: 32, height: 32 });
    await fill(page, mode);
    const filled = await centre(page, mode);
    // A locked layer makes the effect fail.
    await page
      .getByRole('button', { name: /^Lock / })
      .first()
      .click();
    await expect(page.getByRole('button', { name: /^Unlock / }).first()).toBeVisible();

    await menu(page, 'Adjustments', 'Levels…');
    const panel = page.getByRole('dialog', { name: 'Levels' });
    const field = panel.getByLabel('Input black value', { exact: true });
    await field.fill('40');
    await panel.getByRole('button', { name: 'OK' }).click();
    await expect(page.getByText(/Levels failed: .*locked/)).toBeVisible();
    await expect(panel).toBeVisible();
    await expect(field).toHaveValue('40');

    await panel.getByRole('button', { name: 'Cancel' }).click();
    await expect.poll(async () => (await centre(page, mode)).equals(filled)).toBe(true);
    if (mode === 'pixel') {
      // Pixel mode also reports the failed preview.
      await menu(page, 'Adjustments', 'Levels…');
      await expect(page.getByText(/Preview failed: .*locked/)).toBeVisible();
    }
  });

  test(`${mode}: an autosave while an effect is open stores the document without the preview (AC-17)`, async ({
    page,
  }) => {
    await page.clock.install();
    await createDocument(page, mode, { width: 16, height: 16 });
    await fill(page, mode);
    // The fill colour as the document holds it, before any preview.
    await clearClipboard(page);
    await page.keyboard.press('Control+a');
    await page.keyboard.press('Control+c');
    const fillColor = pixelOf(await copiedImage(page), 8, 8);
    await page.keyboard.press('Control+d');
    await openInvert(page);
    await page.clock.fastForward(31_000);
    // Let the IndexedDB write settle before leaving the page.
    await page.waitForTimeout(500);

    await page.reload();
    await createDocument(page, mode, { width: 8, height: 8 });
    const dialog = page.getByRole('dialog', { name: 'Recover unsaved work?' });
    await dialog.getByRole('button', { name: 'Recover' }).first().click();
    await expect(dialog).toHaveCount(0);
    await waitForEditor(page, mode);
    await clearClipboard(page);
    await page.keyboard.press('Control+a');
    await page.keyboard.press('Control+c');
    const img = await copiedImage(page);
    expect(img.width).toBe(16);
    // The fill, not its inverted preview.
    expect(fillColor[3]).toBe(255);
    expect(pixelOf(img, 8, 8)).toEqual(fillColor);
  });
}
