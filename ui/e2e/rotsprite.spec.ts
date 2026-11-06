import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, drag, menu, pixelCanvas } from './helpers';

// Layer ▸ Rotate… (docs/specs/rotsprite.md): the dialog, its live preview,
// the RotSprite cap and the refusals. The rotation maths is covered by
// the pincel-core / pincel-wasm tests; these specs drive the UI.

const dialog = (page: Page) => page.getByRole('dialog', { name: 'Rotate' });
const angleField = (page: Page) => dialog(page).getByRole('spinbutton', { name: 'Angle (°)' });
const angleSlider = (page: Page) => dialog(page).getByRole('slider', { name: 'Angle slider' });
const methodSelect = (page: Page) => dialog(page).getByRole('combobox', { name: 'Method' });
const applyButton = (page: Page) =>
  dialog(page).getByRole('button', { name: 'Apply', exact: true });
const undoButton = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });

/** A 16×16 sprite with a horizontal pencil bar across row 8. */
async function barSprite(page: Page, size = { width: 16, height: 16 }): Promise<Locator> {
  await createDocument(page, 'pixel', size);
  const canvas = pixelCanvas(page);
  await page.getByTitle('Pencil (B)').click();
  const row = 8.5 / size.height;
  await drag(page, canvas, [3.5 / size.width, row], [12.5 / size.width, row]);
  await expect(undoButton(page)).toBeEnabled();
  return canvas;
}

/** Canvas screenshot with the pointer parked off it. */
async function shot(page: Page, canvas: Locator): Promise<Buffer> {
  await page.mouse.move(1, 1);
  return canvas.screenshot();
}

async function openRotate(page: Page): Promise<void> {
  await menu(page, 'Layer', 'Rotate…');
  await expect(dialog(page)).toBeVisible();
}

test('Layer ▸ Rotate… opens a dialog with angle field, slider and method (AC-07)', async ({
  page,
}) => {
  await barSprite(page);
  await openRotate(page);
  await expect(angleField(page)).toHaveAttribute('min', '-180');
  await expect(angleField(page)).toHaveAttribute('max', '180');
  await expect(angleSlider(page)).toHaveAttribute('min', '-180');
  await expect(angleSlider(page)).toHaveAttribute('max', '180');
  await expect(methodSelect(page).locator('option')).toHaveText(['Nearest', 'RotSprite']);
  // Field and slider are one value.
  await angleField(page).fill('45');
  await expect(angleSlider(page)).toHaveValue('45');
  await angleSlider(page).fill('-30');
  await expect(angleField(page)).toHaveValue('-30');
  await methodSelect(page).selectOption('nearest');
  await expect(methodSelect(page)).toHaveValue('nearest');
});

test('the dialog previews live; Cancel leaves the document and history alone (AC-08)', async ({
  page,
}) => {
  const canvas = await barSprite(page);
  // A 1-px staircase too: there RotSprite and Nearest part ways.
  await drag(page, canvas, [2.5 / 16, 2.5 / 16], [7.5 / 16, 7.5 / 16]);
  const original = await shot(page, canvas);
  await openRotate(page);
  await angleField(page).fill('90');
  await expect.poll(async () => (await shot(page, canvas)).equals(original)).toBe(false);
  const rotsprite = await shot(page, canvas);
  // The preview follows the method too: at 45° the two differ.
  await angleField(page).fill('45');
  const atRotsprite45 = await shot(page, canvas);
  expect(atRotsprite45.equals(rotsprite)).toBe(false);
  await methodSelect(page).selectOption('nearest');
  await expect.poll(async () => (await shot(page, canvas)).equals(atRotsprite45)).toBe(false);

  // Cancel after moving the slider (Edge cases).
  await angleSlider(page).fill('60');
  await dialog(page).getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog(page)).toBeHidden();
  await expect.poll(async () => (await shot(page, canvas)).equals(original)).toBe(true);
  // The only undo steps are still the two pencil strokes.
  await undoButton(page).click();
  await undoButton(page).click();
  await expect(undoButton(page)).toBeDisabled();
});

test('Apply commits the previewed rotation as one undo step (AC-08)', async ({ page }) => {
  const canvas = await barSprite(page);
  const original = await shot(page, canvas);
  await openRotate(page);
  await angleField(page).fill('90');
  await expect.poll(async () => (await shot(page, canvas)).equals(original)).toBe(false);
  const preview = await shot(page, canvas);
  await applyButton(page).click();
  await expect(dialog(page)).toBeHidden();
  await expect(page.getByText('rotated by 90°')).toBeVisible();
  await expect.poll(async () => (await shot(page, canvas)).equals(preview)).toBe(true);
  await undoButton(page).click();
  await expect.poll(async () => (await shot(page, canvas)).equals(original)).toBe(true);
  await undoButton(page).click();
  await expect(undoButton(page)).toBeDisabled();
});

test('RotSprite over 512 px shows a notice and rotates nothing (AC-09)', async ({ page }) => {
  const canvas = await barSprite(page, { width: 600, height: 16 });
  // Shots without a selection: marching ants move between two shots.
  const original = await shot(page, canvas);
  await menu(page, 'Select', 'Select All');
  await openRotate(page);
  const notice = dialog(page).getByRole('alert');
  await expect(methodSelect(page)).toHaveValue('rotsprite');
  await expect(notice).toContainText('512×512');
  await expect(notice).toContainText('600×16');
  await expect(applyButton(page)).toBeDisabled();
  // Enter in the angle field doesn't get past the cap either.
  await angleField(page).fill('30');
  await angleField(page).press('Enter');
  await expect(dialog(page)).toBeVisible();
  // Nearest has no cap.
  await methodSelect(page).selectOption('nearest');
  await expect(notice).toBeHidden();
  await expect(applyButton(page)).toBeEnabled();
  await methodSelect(page).selectOption('rotsprite');
  await dialog(page).getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog(page)).toBeHidden();
  await menu(page, 'Select', 'Deselect');
  await expect.poll(async () => (await shot(page, canvas)).equals(original)).toBe(true);
});

test('a locked layer or an empty one rotates nothing (AC-10)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await menu(page, 'Layer', 'Rotate…');
  await expect(page.getByText('rotate: nothing to rotate')).toBeVisible();
  await expect(dialog(page)).toBeHidden();
  await expect(undoButton(page)).toBeDisabled();

  const canvas = await barSprite(page);
  const original = await shot(page, canvas);
  await page.getByRole('button', { name: 'Lock Layer 1', exact: true }).click();
  await menu(page, 'Layer', 'Rotate…');
  await expect(page.getByText('Layer “Layer 1” is locked')).toBeVisible();
  await expect(dialog(page)).toBeHidden();
  expect((await shot(page, canvas)).equals(original)).toBe(true);
  // Undo reverts the lock, then the stroke: no rotation step between.
  await undoButton(page).click();
  await expect(page.getByRole('button', { name: 'Lock Layer 1', exact: true })).toBeVisible();
  await undoButton(page).click();
  await expect(undoButton(page)).toBeDisabled();
});

test('a typed angle outside ±180° snaps to the nearest bound (AC-11)', async ({ page }) => {
  await barSprite(page);
  await openRotate(page);
  await angleField(page).fill('400');
  await expect(angleField(page)).toHaveValue('180');
  await expect(angleSlider(page)).toHaveValue('180');
  await angleField(page).fill('-999');
  await expect(angleField(page)).toHaveValue('-180');
  await expect(angleSlider(page)).toHaveValue('-180');
  await angleField(page).fill('181');
  await expect(angleField(page)).toHaveValue('180');
});
