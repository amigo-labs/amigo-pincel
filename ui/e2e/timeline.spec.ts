import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, menu, pixelCanvas } from './helpers';

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

// docs/specs/pixel-layers-frames.md: frames, tags and onion skin.

/** The `undo n / redo m` readout of the Pixel status bar. */
const depths = (page: Page, undo: number, redo = 0) =>
  page.getByText(`undo ${undo} / redo ${redo}`, { exact: true });

const timelineOf = (page: Page) => page.getByRole('region', { name: 'Timeline' });

test('selecting a frame shows it, reads frame n/total and takes the next stroke (AC-27)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const timeline = timelineOf(page);
  const canvas = pixelCanvas(page);
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await expect(timeline.getByText('frame 3/3')).toBeVisible();
  await page.mouse.move(0, 0);
  const blank = await canvas.screenshot();

  // A stroke on frame 2 lands in frame 2 only.
  await timeline.getByRole('button', { name: 'Frame 2', exact: true }).click();
  await expect(timeline.getByText('frame 2/3')).toBeVisible();
  await page.getByTitle('Pencil (B)').click();
  await drag(page, canvas, [0.2, 0.5], [0.8, 0.5]);
  await page.mouse.move(0, 0);
  await expect.poll(async () => (await canvas.screenshot()).equals(blank)).toBe(false);
  const painted = await canvas.screenshot();

  /** The timeline reads `frame n/3` and the canvas shows `shot`. */
  const shows = async (n: number, shot: Buffer) => {
    await expect(timeline.getByText(`frame ${n}/3`)).toBeVisible();
    await expect.poll(async () => (await canvas.screenshot()).equals(shot)).toBe(true);
  };
  await timeline.getByRole('button', { name: 'Previous frame' }).click();
  await shows(1, blank);
  await timeline.getByRole('button', { name: 'Next frame' }).click();
  await shows(2, painted);
  await page.keyboard.press('.');
  await shows(3, blank);
  await page.keyboard.press(',');
  await shows(2, painted);
  await menu(page, 'Frame', 'Previous Frame');
  await shows(1, blank);
  await menu(page, 'Frame', 'Next Frame');
  await shows(2, painted);
  await timeline.getByRole('button', { name: 'Frame 3', exact: true }).click();
  await shows(3, blank);
});

test('a duration of 0, above 65535 or not a number keeps the previous one (AC-31)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const timeline = timelineOf(page);
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  const duration = timeline.getByLabel('Frame 1 duration (ms)');
  await duration.fill('250');
  await duration.press('Enter');
  await expect(duration).toHaveValue('250');
  await expect(depths(page, 2)).toBeVisible();

  for (const typed of ['0', '65536', '70000', '']) {
    await duration.fill(typed);
    await duration.press('Enter');
    // The field shows the frame's duration again, and no edit happened.
    await expect(duration).toHaveValue('250');
    await expect(depths(page, 2)).toBeVisible();
  }
  // The last valid edit is still the one an undo takes back.
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(duration).toHaveValue('100');
});

test('a new tag covers the current frame, plays forward, takes the next colour and opens its editor (AC-32)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const timeline = timelineOf(page);
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await timeline.getByRole('button', { name: 'Frame 2', exact: true }).click();

  await timeline.getByRole('button', { name: 'Add tag' }).click();
  const bar = timeline.getByRole('button', { name: 'Tag tag1' });
  await expect(bar).toHaveAttribute('title', 'tag1 · frames 2–2 · forward');
  const editor = timeline.getByLabel('Tag editor');
  await expect(editor.getByLabel('Tag name')).toHaveValue('tag1');
  await expect(editor.getByLabel('Tag from')).toHaveValue('2');
  await expect(editor.getByLabel('Tag to')).toHaveValue('2');
  await expect(editor.getByLabel('Tag direction')).toHaveValue('forward');

  // Each new tag takes the next of six colours, then the cycle repeats.
  for (let n = 2; n <= 7; n += 1) {
    await timeline.getByRole('button', { name: 'Add tag' }).click();
    await expect(timeline.getByRole('button', { name: `Tag tag${n}` })).toBeVisible();
  }
  const colours = await timeline
    .getByRole('button', { name: /^Tag tag\d$/ })
    .evaluateAll((els) => els.map((el) => getComputedStyle(el).borderTopColor));
  expect(colours).toEqual([
    'rgb(91, 141, 239)',
    'rgb(224, 168, 60)',
    'rgb(88, 179, 104)',
    'rgb(208, 96, 122)',
    'rgb(154, 106, 216)',
    'rgb(60, 183, 196)',
    'rgb(91, 141, 239)',
  ]);

  // One undo removes the last tag.
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(timeline.getByRole('button', { name: 'Tag tag7' })).toHaveCount(0);
  await expect(timeline.getByRole('button', { name: 'Tag tag6' })).toBeVisible();
});

test('a tag name that is not identifier-like warns but is kept (AC-34)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const timeline = timelineOf(page);
  await timeline.getByRole('button', { name: 'Add tag' }).click();
  const editor = timeline.getByLabel('Tag editor');
  const name = editor.getByLabel('Tag name');
  const hint = editor.getByText('Use letters, digits and _ only', { exact: false });

  for (const bad of ['walk cycle', '2walk', 'walk-left']) {
    await name.fill(bad);
    await name.press('Enter');
    const bar = timeline.getByRole('button', { name: `Tag ${bad}` });
    await expect(bar).toBeVisible();
    await expect(bar.getByTitle('Not identifier-like')).toHaveText('⚠');
    await expect(hint).toBeVisible();
    await expect(name).toHaveValue(bad);
  }
  for (const good of ['walk_2', '_idle', 'Run']) {
    await name.fill(good);
    await name.press('Enter');
    const bar = timeline.getByRole('button', { name: `Tag ${good}` });
    await expect(bar).toBeVisible();
    await expect(bar.getByTitle('Not identifier-like')).toHaveCount(0);
    await expect(hint).toHaveCount(0);
  }
});

test('Play is unavailable while the document has fewer than two frames (AC-38)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const timeline = timelineOf(page);
  const play = timeline.getByRole('button', { name: 'Play' });
  const menuPlay = async () => {
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Frame', exact: true }).click();
    const item = page
      .getByRole('menu', { name: 'Frame', exact: true })
      .getByRole('menuitem', { name: 'Play', exact: true });
    await expect(item).toBeVisible();
    return item;
  };

  await expect(play).toBeDisabled();
  await expect(await menuPlay()).toBeDisabled();
  await page.keyboard.press('Escape');
  await page.keyboard.press('Enter');
  await page.waitForTimeout(300);
  await expect(timeline.getByRole('button', { name: 'Pause' })).toHaveCount(0);

  await timeline.getByRole('button', { name: 'Add frame' }).click();
  await expect(play).toBeEnabled();
  await expect(await menuPlay()).toBeEnabled();
  await page.keyboard.press('Escape');
  await timeline.getByRole('button', { name: 'Remove frame' }).click();
  await expect(timeline.getByText('frame 1/1')).toBeVisible();
  await expect(play).toBeDisabled();
});

test('onion-skin settings add no undo step and leave the document unchanged (AC-40)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const timeline = timelineOf(page);
  const undo = page.getByRole('button', { name: 'Undo', exact: true });
  await expect(depths(page, 0)).toBeVisible();
  await expect(page.getByTitle(/unsaved changes/)).toHaveCount(0);

  await timeline.getByLabel('Onion skin').check();
  const back = timeline.getByLabel('Onion frames back');
  const forward = timeline.getByLabel('Onion frames forward');
  await back.fill('3');
  await back.press('Enter');
  await forward.fill('2');
  await forward.press('Enter');
  await timeline.getByLabel('Onion opacity').fill('200');
  await expect(back).toHaveValue('3');
  await expect(forward).toHaveValue('2');
  await timeline.getByLabel('Onion skin').uncheck();

  await expect(depths(page, 0)).toBeVisible();
  await expect(undo).toBeDisabled();
  await expect(page.getByTitle(/unsaved changes/)).toHaveCount(0);
});
