import { expect, test, type Page } from '@playwright/test';
import { createDocument } from './helpers';

// Pixel-mode playback (docs/specs/pixel-layers-frames.md AC-37, AC-41):
// tag directions and wall-clock timing. The page clock is faked and
// paused before Play, so every frame change happens at a known time.

const timelineOf = (page: Page) => page.getByRole('region', { name: 'Timeline' });

/** A paused clock and a 16 × 16 sprite of `frames` frames, frame 1 current. */
async function setup(page: Page, frames: number, durations: number[] = []): Promise<void> {
  await page.clock.install();
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const timeline = timelineOf(page);
  for (let i = 1; i < frames; i += 1) {
    await timeline.getByRole('button', { name: 'Add frame' }).click();
  }
  await expect(timeline.getByText(`frame ${frames}/${frames}`)).toBeVisible();
  for (const [i, ms] of durations.entries()) {
    const field = timeline.getByLabel(`Frame ${i + 1} duration (ms)`);
    await field.fill(String(ms));
    await field.press('Enter');
    await expect(field).toHaveValue(String(ms));
  }
}

/**
 * Pauses the clock, presses Play and records every change of the
 * timeline's `frame n/total` readout with the (fake) time it happened,
 * relative to the press.
 */
async function play(page: Page): Promise<void> {
  const now = await page.evaluate(() => Date.now());
  await page.clock.pauseAt(now + 1000);
  await page.evaluate(() => {
    const readout = [...document.querySelectorAll('[aria-live="polite"]')].find((el) =>
      /^frame \d+\/\d+$/.test(el.textContent ?? ''),
    );
    if (!readout) throw new Error('no frame readout');
    const w = window as unknown as { shown: [number, number][]; playedAt: number };
    w.shown = [];
    w.playedAt = performance.now();
    new MutationObserver(() => {
      const n = Number(/frame (\d+)/.exec(readout.textContent ?? '')?.[1]);
      const last = w.shown.at(-1);
      if (last?.[1] !== n) w.shown.push([performance.now() - w.playedAt, n]);
    }).observe(readout, { characterData: true, childList: true, subtree: true });
  });
  await timelineOf(page).getByRole('button', { name: 'Play' }).click();
}

/** The frames shown since Play, with the time each one appeared. */
async function shown(page: Page): Promise<[number, number][]> {
  return page.evaluate(() => (window as unknown as { shown: [number, number][] }).shown);
}

/** The frame shown right now. */
async function current(page: Page): Promise<number> {
  const text = await timelineOf(page).getByText(/^frame \d+\/\d+$/).textContent();
  return Number(/frame (\d+)/.exec(text ?? '')?.[1]);
}

const DIRECTIONS = [
  // Tag over frames 2–4 of 5; frames 1 and 5 must never show.
  { direction: 'forward', start: 2, frames: [2, 3, 4, 2, 3, 4, 2] },
  { direction: 'reverse', start: 4, frames: [4, 3, 2, 4, 3, 2, 4] },
  { direction: 'pingpong', start: 2, frames: [2, 3, 4, 3, 2, 3, 4] },
  { direction: 'pingpong_reverse', start: 4, frames: [4, 3, 2, 3, 4, 3, 2] },
] as const;

for (const { direction, start, frames } of DIRECTIONS) {
  test(`a ${direction} tag plays its frames in that order (AC-37)`, async ({ page }) => {
    await setup(page, 5);
    const timeline = timelineOf(page);
    await timeline.getByRole('button', { name: 'Frame 2', exact: true }).click();
    await timeline.getByRole('button', { name: 'Add tag' }).click();
    const editor = timeline.getByLabel('Tag editor');
    await editor.getByLabel('Tag to').fill('4');
    await editor.getByLabel('Tag to').press('Enter');
    await editor.getByLabel('Tag direction').selectOption(direction);
    await expect(timeline.getByRole('button', { name: 'Tag tag1' })).toHaveAttribute(
      'title',
      `tag1 · frames 2–4 · ${direction}`,
    );
    await timeline.getByLabel('Loop range').selectOption({ label: 'tag1' });
    await timeline.getByRole('button', { name: `Frame ${start}`, exact: true }).click();
    await expect(timeline.getByText(`frame ${start}/5`)).toBeVisible();

    await play(page);
    // Every frame lasts 100 ms: sample in the middle of each slot.
    await page.clock.runFor(50);
    const seen = [await current(page)];
    for (let i = 1; i < frames.length; i += 1) {
      await page.clock.runFor(100);
      seen.push(await current(page));
    }
    expect(seen).toEqual(frames);
    await timeline.getByRole('button', { name: 'Pause' }).click();
  });
}

test('each frame stays on screen for its own duration (AC-41)', async ({ page }) => {
  const durations = [100, 300, 50, 150];
  await setup(page, 4, durations);
  await timelineOf(page).getByRole('button', { name: 'Frame 1', exact: true }).click();
  await play(page);
  await page.clock.runFor(1500);
  const changes = await shown(page);
  // Frame 1, then 2, 3, 4, 1, 2, … each for its own duration (one
  // animation frame of slack, 16 ms).
  expect(changes.length).toBeGreaterThanOrEqual(6);
  const sequence = changes.map(([, n]) => n);
  expect(sequence.slice(0, 6)).toEqual([2, 3, 4, 1, 2, 3]);
  for (let i = 1; i < changes.length; i += 1) {
    const [from, frame] = changes[i - 1] ?? [0, 0];
    const [to] = changes[i] ?? [0];
    expect(Math.abs(to - from - (durations[frame - 1] ?? 0))).toBeLessThanOrEqual(17);
  }
});

test('a late frame skips ahead to keep wall-clock time (AC-41)', async ({ page }) => {
  await setup(page, 4);
  const timeline = timelineOf(page);
  await timeline.getByRole('button', { name: 'Frame 1', exact: true }).click();
  await play(page);
  // The page stalls for 250 ms: frame 2's slot (100–200 ms) has passed,
  // so the next frame shown is 3, and 4 still starts at 300 ms.
  await page.clock.fastForward(250);
  expect(await current(page)).toBe(3);
  expect((await shown(page)).map(([, n]) => n)).toEqual([3]);
  await page.clock.runFor(40);
  expect(await current(page)).toBe(3);
  await page.clock.runFor(40);
  expect(await current(page)).toBe(4);
  const [, last] = await shown(page);
  expect(last?.[1]).toBe(4);
  expect(last?.[0]).toBeLessThanOrEqual(317);
  await timeline.getByRole('button', { name: 'Pause' }).click();
});
