import { expect, test, type Page } from '@playwright/test';
import {
  type ModelAnswer,
  ocrDialog,
  ocrStatus,
  openOcr,
  openPng,
  pagePng,
  recognizedText,
  recordSteps,
  recordedSteps,
  serveModels,
  storedModels,
  wordsPng,
} from './ocr-helpers';

// Stopping the Recognize Text dialog (docs/specs/ocr-dialog.md): Cancel,
// Escape and Close during the download and the recognition, and how fast
// Cancel ends a long recognition.

const words = (text: string): string[] => text.split(/\s+/).filter((w) => w !== '');

async function openThreeWords(page: Page): Promise<void> {
  const png = await wordsPng(page, 640, 200, [
    { text: 'apple', x: 30, y: 70 },
    { text: 'river', x: 250, y: 70 },
    { text: 'garden', x: 450, y: 70 },
  ]);
  await openPng(page, 'words.png', png);
}

/** Holds every model request until `release` is called. */
function held(): { answer: () => Promise<ModelAnswer>; release: () => void } {
  let release = () => {};
  const gate = new Promise<void>((resolve) => (release = resolve));
  return {
    answer: async () => {
      await gate;
      return 'serve';
    },
    release,
  };
}

/** Milliseconds from a click on the dialog's `button` until it is gone. */
async function closeTime(page: Page, button: string): Promise<number> {
  return page.evaluate(
    (label) =>
      new Promise<number>((resolve, reject) => {
        const dialog = document.querySelector('[role="dialog"][aria-label="Recognize Text"]');
        const target = [...(dialog?.querySelectorAll('button') ?? [])].find(
          (b) => b.textContent?.trim() === label,
        );
        if (!dialog || !target) {
          reject(new Error(`no ${label} button`));
          return;
        }
        const start = performance.now();
        const watch = new MutationObserver(() => {
          if (dialog.isConnected) return;
          watch.disconnect();
          resolve(performance.now() - start);
        });
        watch.observe(document.body, { childList: true, subtree: true });
        target.click();
      }),
    button,
  );
}

test('Cancel during the download closes the dialog and stores nothing (AC-12)', async ({
  page,
}) => {
  const hold = held();
  const requests = await serveModels(page, (_lang, n) => (n === 0 ? hold.answer() : 'serve'));
  await openThreeWords(page);
  await openOcr(page);
  await expect(ocrStatus(page)).toHaveText(/Downloading English model…/);
  await ocrDialog(page).getByRole('button', { name: 'Cancel' }).click();
  await expect(ocrDialog(page)).toBeHidden();
  hold.release();
  // The discarded download is downloaded again next time.
  await openOcr(page);
  expect(words(await recognizedText(page))).toEqual(['apple', 'river', 'garden']);
  expect(requests.count('eng')).toBe(2);
});

test('Escape during the download closes the dialog (AC-12)', async ({ page }) => {
  const hold = held();
  await serveModels(page, hold.answer);
  await openThreeWords(page);
  await openOcr(page);
  await expect(ocrStatus(page)).toHaveText(/Downloading English model…/);
  await page.keyboard.press('Escape');
  await expect(ocrDialog(page)).toBeHidden();
  hold.release();
  await page.waitForTimeout(300);
  expect(await storedModels(page)).toEqual([]);
});

test('another language during a download drops it and reads with the other model', async ({
  page,
}) => {
  const hold = held();
  const requests = await serveModels(page, (lang) => (lang === 'eng' ? hold.answer() : 'serve'));
  await openThreeWords(page);
  await openOcr(page);
  await expect(ocrStatus(page)).toHaveText(/Downloading English model…/);
  await ocrDialog(page).getByRole('combobox', { name: 'Language' }).selectOption('deu');
  expect(words(await recognizedText(page))).toEqual(['apple', 'river', 'garden']);
  hold.release();
  await page.waitForTimeout(300);
  expect((await storedModels(page)).map((m) => m.lang)).toEqual(['deu']);
  expect(requests.count('deu')).toBe(1);
});

test.describe('the benchmark screenshot', () => {
  test.setTimeout(180_000);

  test('progress changes per line; Cancel closes within 500 ms and keeps the stored model (AC-12, AC-14)', async ({
    page,
  }) => {
    await serveModels(page);
    await openPng(page, 'ui_light.png', pagePng('eng', 'ui_light'));
    await recordSteps(page);
    await openOcr(page);
    await recognizedText(page, 150_000);
    const lines = (await recordedSteps(page))
      .map((s) => /^Recognizing line (\d+) of (\d+)$/.exec(s))
      .filter((m) => m !== null);
    const total = Number(lines[0]?.[2]);
    expect(total).toBeGreaterThan(10);
    // The line changes at least once per recognized line: every k shows.
    expect([...new Set(lines.map((m) => Number(m[1])))]).toEqual(
      Array.from({ length: total }, (_, i) => i + 1),
    );
    await ocrDialog(page).getByRole('button', { name: 'Close' }).click();

    await openOcr(page);
    await expect(ocrStatus(page)).toHaveText(/^Recognizing line ([3-9]|\d\d) of/, {
      timeout: 60_000,
    });
    expect(await closeTime(page, 'Cancel')).toBeLessThan(500);
    await expect(ocrDialog(page)).toBeHidden();
    expect((await storedModels(page)).map((m) => m.lang)).toEqual(['eng']);
  });

  test('Escape during recognition closes the dialog (AC-12)', async ({ page }) => {
    await serveModels(page);
    await openPng(page, 'ui_light.png', pagePng('eng', 'ui_light'));
    await openOcr(page);
    await expect(ocrStatus(page)).toHaveText(/^Recognizing line/, { timeout: 60_000 });
    await page.keyboard.press('Escape');
    await expect(ocrDialog(page)).toBeHidden();
  });
});
