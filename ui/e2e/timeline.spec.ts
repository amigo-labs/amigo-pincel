import { expect, test } from '@playwright/test';
import { createDocument } from './helpers';

test('timeline: frames, durations, tags, playback and onion skin', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await expect(timeline).toBeVisible();

  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await expect(timeline.getByText('frame 3/3')).toBeVisible();

  const duration = timeline.getByLabel('Frame 2 duration (ms)');
  await duration.fill('40');
  await duration.press('Enter');
  await expect(duration).toHaveValue('40');
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(duration).toHaveValue('100');

  await timeline.getByRole('button', { name: 'Frame 1', exact: true }).click();
  await timeline.getByRole('button', { name: 'Add tag' }).click();
  const editor = timeline.getByLabel('Tag editor');
  await editor.getByLabel('Tag name').fill('walk cycle');
  await editor.getByLabel('Tag name').press('Enter');
  await expect(editor.getByText('Use letters, digits and _ only')).toBeVisible();
  await editor.getByLabel('Tag name').fill('walk');
  await editor.getByLabel('Tag name').press('Enter');
  await editor.getByLabel('Tag to').fill('3');
  await editor.getByLabel('Tag to').press('Enter');
  await editor.getByLabel('Tag direction').selectOption('pingpong');
  await expect(timeline.getByRole('button', { name: 'Tag walk' })).toBeVisible();

  await timeline.getByLabel('Loop range').selectOption({ label: 'walk' });
  await timeline.getByRole('button', { name: 'Play' }).click();
  await expect(timeline.getByText('frame 2/3')).toBeVisible();
  await expect(timeline.getByText('frame 3/3')).toBeVisible();
  await timeline.getByRole('button', { name: 'Pause' }).click();

  await timeline.getByLabel('Onion skin').check();
  await expect(timeline.getByLabel('Onion opacity')).toBeEnabled();

  await timeline.getByRole('button', { name: 'Frame 3', exact: true }).click();
  await timeline.getByRole('button', { name: 'Remove frame' }).click();
  await expect(timeline.getByText('frame 2/2')).toBeVisible();
});
