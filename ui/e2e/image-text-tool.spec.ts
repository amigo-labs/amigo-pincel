import { expect, test } from '@playwright/test';
import { createDocument } from './helpers';
import { H, W, at, ink, liveInput, textBox, undoAll, undoButton } from './text-helpers';

// Image mode's Text tool: a click opens a live text box at the point that
// stays open and keeps the keyboard focus (docs/specs/image-live-text.md
// AC-01); typing goes into it and Ctrl+Enter places the text as one undo
// step (AC-11).

test('image: a click with the Text tool opens a box that keeps focus and places the text', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: W, height: H });
  await page.keyboard.press('t');
  const [x, y] = await at(page, [20, 24]);
  await page.mouse.click(x, y);

  await expect(textBox(page)).toBeVisible();
  await expect(liveInput(page)).toBeFocused();
  await page.keyboard.type('Hi');
  await expect(textBox(page)).toBeVisible();
  await expect(liveInput(page)).toBeFocused();
  expect(await ink(page)).not.toBeNull();

  await page.keyboard.press('Control+Enter');
  await expect(textBox(page)).toHaveCount(0);
  await expect(undoButton(page)).toBeEnabled();
  expect(await undoAll(page)).toBe(1);
  expect(await ink(page)).toBeNull();
});

test('image: a Text click takes the focus from a field being typed in', async ({ page }) => {
  await createDocument(page, 'image', { width: W, height: H });
  await page.keyboard.press('t');
  // An open rename field holds the focus before the press.
  const rename = page.getByRole('button', { name: /^Rename / }).first();
  const label = (await rename.getAttribute('aria-label')) ?? '';
  await rename.click();
  const field = page.getByRole('textbox', { name: label, exact: true });
  await expect(field).toBeFocused();

  const [x, y] = await at(page, [20, 24]);
  await page.mouse.click(x, y);
  await expect(textBox(page)).toBeVisible();
  await expect(liveInput(page)).toBeFocused();
  await expect(field).toHaveCount(0);
  await page.keyboard.type('Hi');
  expect(await ink(page)).not.toBeNull();
  // The layer keeps its name: the typing went into the text.
  await expect(page.getByRole('button', { name: label, exact: true })).toBeVisible();
});
