import { expect, test } from '@playwright/test';
import { createDocument, imageCanvas } from './helpers';

// Image mode's Text tool: a click opens an entry box at the point; typing
// goes into it and Escape places the text as one undo step.
test('image: a click with the Text tool opens an entry that keeps focus and places the text', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 48, height: 48 });
  await page.keyboard.press('t');
  const box = await imageCanvas(page).boundingBox();
  if (!box) throw new Error('canvas is not visible');
  await page.mouse.click(box.x + box.width * 0.1, box.y + box.height * 0.2);

  const entry = page.locator('textarea');
  await expect(entry).toBeVisible();
  await expect(entry).toBeFocused();
  await page.keyboard.type('Hi');
  await expect(entry).toHaveValue('Hi');
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  await expect(undo).toBeDisabled();
  await page.keyboard.press('Escape');
  await expect(entry).toHaveCount(0);
  await expect(undo).toBeEnabled();
});
