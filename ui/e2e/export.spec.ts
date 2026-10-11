import { readFile } from 'node:fs/promises';
import { expect, test } from '@playwright/test';
import { createDocument } from './helpers';

// Force the download fallback: the File System Access picker would open a
// native dialog the test cannot drive.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    delete (window as { showSaveFilePicker?: unknown }).showSaveFilePicker;
  });
});

test('sprite sheet export writes the PNG and the Aseprite JSON', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  const timeline = page.getByRole('region', { name: 'Timeline' });
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Frame 1', exact: true }).click();
  await timeline.getByRole('button', { name: 'Add tag' }).click();
  await timeline.getByLabel('Tag editor').getByLabel('Tag to').fill('2');
  await timeline.getByLabel('Tag editor').getByLabel('Tag to').press('Enter');

  await page.getByRole('button', { name: 'Image…' }).click();
  await page.getByRole('menuitem', { name: 'Export Sprite Sheet…' }).click();
  const dialog = page.getByRole('dialog', { name: 'Export sprite sheet' });
  await dialog.getByLabel('Columns').fill('3');
  await expect(dialog.getByText('3 × 1 cells · 24×8 px')).toBeVisible();

  const downloads: string[] = [];
  let jsonText = '';
  const both = new Promise<void>((resolve) => {
    page.on('download', async (d) => {
      if (d.suggestedFilename().endsWith('.json'))
        jsonText = await readFile(await d.path(), 'utf8');
      downloads.push(d.suggestedFilename());
      if (downloads.length === 2) resolve();
    });
  });
  await dialog.getByRole('button', { name: 'Export' }).click();
  await both;
  const json = JSON.parse(jsonText) as {
    frames: { duration: number }[];
    meta: {
      image: string;
      size: { w: number; h: number };
      frameTags: { name: string; to: number }[];
    };
  };
  expect(json.frames).toHaveLength(3);
  expect(json.frames[0]?.duration).toBe(100);
  expect(json.meta.size).toEqual({ w: 24, h: 8 });
  expect(json.meta.image).toBe('pincel.png');
  expect(json.meta.frameTags[0]).toMatchObject({ name: 'tag1', to: 1 });
  expect(downloads.sort()).toEqual(['pincel.json', 'pincel.png']);
});
