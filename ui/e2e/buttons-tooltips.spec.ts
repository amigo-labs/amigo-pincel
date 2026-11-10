// The shared button looks and the tooltip layer (spec §9.2): the themed
// tooltip with its keycap, the parked `title`, the danger variant, the
// accent "on" state and the pressed feedback.
import { expect, test } from '@playwright/test';
import { createDocument } from './helpers';

test.describe('buttons and tooltips', () => {
  test('a rail tool shows a themed tooltip with keycap and variants hint', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const marquee = page.getByRole('button', { name: /^Marquee/ });
    await marquee.hover();
    const tip = page.locator('[data-tooltip]');
    await expect(tip).toBeVisible();
    await expect(tip.locator('kbd')).toHaveText('M');
    await expect(tip).toContainText('Marquee');
    await expect(tip).toContainText('Hold or right-click: variants');
    // While it shows, the OS tooltip is off; the name stays.
    await expect(marquee).not.toHaveAttribute('title');
    await expect(page.getByTitle('Marquee (M)')).toHaveCount(0);
    // The tooltip sits beside the vertical rail, not over the next tool.
    const tipBox = await tip.boundingBox();
    const toolBox = await marquee.boundingBox();
    expect(tipBox && toolBox && tipBox.x >= toolBox.x + toolBox.width).toBe(true);
  });

  test('a press dismisses the tooltip and restores the title', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const pencil = page.getByTitle('Pencil (B)');
    await pencil.hover();
    await expect(page.locator('[data-tooltip]')).toBeVisible();
    await page.mouse.down();
    await expect(page.locator('[data-tooltip]')).toHaveCount(0);
    await page.mouse.up();
    await expect(pencil).toHaveAttribute('title', 'Pencil (B)');
    // Dismissed stays dismissed until the pointer has been elsewhere.
    await page.waitForTimeout(600);
    await expect(page.locator('[data-tooltip]')).toHaveCount(0);
  });

  test('a key dismisses the tooltip of a focused button', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const pencil = page.getByTitle('Pencil (B)');
    await pencil.focus();
    await page.keyboard.press('Shift+Tab');
    await page.keyboard.press('Tab');
    await expect(page.locator('[data-tooltip]')).toBeVisible();
    await page.keyboard.press('Shift');
    await expect(page.locator('[data-tooltip]')).toHaveCount(0);
  });

  test('the active tool and a pressed button use the shared states', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const pencil = page.getByTitle('Pencil (B)');
    const eraser = page.getByTitle('Eraser (E)');
    await pencil.click();
    await expect(pencil).toHaveClass(/btn-on/);
    await expect(eraser).not.toHaveClass(/btn-on/);
    const box = await eraser.boundingBox();
    if (!box) throw new Error('no eraser button');
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await expect(eraser).toHaveCSS('transform', 'matrix(1, 0, 0, 1, 0, 1)');
    await page.mouse.up();
    await expect(eraser).toHaveCSS('transform', 'none');
  });

  test('a tooltip keeps the name of a control that has one and describes it', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await page.evaluate(() => {
      const b = document.createElement('button');
      b.id = 'probe';
      b.textContent = 'Probe';
      b.title = 'More about the probe';
      b.style.cssText = 'position:fixed;left:300px;top:300px;z-index:99';
      const inner = document.createElement('span');
      inner.id = 'inner';
      inner.title = 'Inner warning';
      inner.textContent = ' !';
      b.append(inner);
      document.body.append(b);
    });
    const probe = page.locator('#probe');
    const box = await probe.boundingBox();
    if (!box) throw new Error('no probe');
    await page.mouse.move(box.x + 6, box.y + box.height / 2);
    const tip = page.locator('[data-tooltip]');
    await expect(tip).toContainText('More about the probe');
    await expect(probe).not.toHaveAttribute('aria-label');
    await expect(probe).toHaveAttribute('aria-description', 'More about the probe');
    // A titled element inside the host gets its own tooltip.
    await page.locator('#inner').hover();
    await expect(tip).toContainText('Inner warning');
    await expect(probe).toHaveAttribute('title', 'More about the probe');
    await expect(probe).not.toHaveAttribute('aria-description');
  });

  test('focus by a press shows no tooltip', async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await page.evaluate(() => {
      const i = document.createElement('input');
      i.id = 'field';
      i.title = 'Field hint';
      i.style.cssText = 'position:fixed;left:300px;top:300px;z-index:99';
      document.body.append(i);
    });
    // Touch: no pointerover tooltip, and the focus it gives must not show one either.
    await page.locator('#field').dispatchEvent('pointerdown', { pointerType: 'touch' });
    await page.locator('#field').focus();
    await page.waitForTimeout(700);
    await expect(page.locator('[data-tooltip]')).toHaveCount(0);
  });

  test("Don't Save is a danger button, Cancel shows its key", async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await page.getByTitle('Pencil (B)').click();
    const canvas = page.locator('canvas[data-ready="true"]').first();
    const box = await canvas.boundingBox();
    if (!box) throw new Error('no canvas');
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 + 20, box.y + box.height / 2);
    await page.mouse.up();
    await page.keyboard.press('Alt+w');
    const prompt = page.getByRole('alertdialog');
    await expect(prompt).toBeVisible();
    await expect(prompt.getByRole('button', { name: "Don't Save" })).toHaveClass(
      /panel-btn-danger/,
    );
    await expect(prompt.getByRole('button', { name: 'Save', exact: true })).toHaveClass(
      /panel-btn-primary/,
    );
    // The keycap is not part of the name.
    const cancel = prompt.getByRole('button', { name: 'Cancel', exact: true });
    await expect(cancel.locator('kbd')).toHaveText('Esc');
  });
});
