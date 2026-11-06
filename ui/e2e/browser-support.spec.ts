import { expect, test } from '@playwright/test';
import { gotoStart } from './helpers';

// A browser without wasm SIMD gets a notice instead of an editor
// (docs/specs/performance-build-simd.md AC-07). The start screen detects
// SIMD with `WebAssembly.validate`; the spec makes it answer "no".

test('without wasm SIMD the start screen explains why and loads no editor (AC-07)', async ({
  page,
}) => {
  await page.addInitScript(() => {
    WebAssembly.validate = () => false;
  });
  const wasm: string[] = [];
  page.on('request', (r) => {
    if (r.url().endsWith('.wasm')) wasm.push(r.url());
  });
  await gotoStart(page);
  await expect(
    page.getByText(
      'This browser is too old for Pincel (needs Chrome/Edge 91+, Firefox 89+, Safari 16.4+)',
    ),
  ).toBeVisible();
  await expect(page.getByRole('button', { name: 'New pixel sprite' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'New image' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Open…' })).toBeDisabled();
  expect(wasm).toEqual([]);
});

test('with wasm SIMD the start screen shows no notice (AC-07)', async ({ page }) => {
  await gotoStart(page);
  await expect(page.getByText(/too old for Pincel/)).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'New image' })).toBeEnabled();
});
