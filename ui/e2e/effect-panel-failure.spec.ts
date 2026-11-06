import { expect, test, type Page } from '@playwright/test';
import { createDocument, imageCanvas, menu, pixelCanvas } from './helpers';

// A preview that fails keeps the effect panel open with its values, shows
// the error where the mode shows errors and leaves the document unchanged
// (effect-panel AC-16). The OK path is in effect-panel-input.spec.ts.

type Mode = 'pixel' | 'image';

const canvasOf = (page: Page, mode: Mode) =>
  mode === 'pixel' ? pixelCanvas(page) : imageCanvas(page);

/** A 6 × 6 px screenshot around the canvas centre. */
async function centre(page: Page, mode: Mode): Promise<Buffer> {
  const box = await canvasOf(page, mode).boundingBox();
  if (!box) throw new Error('canvas is not visible');
  return page.screenshot({
    clip: { x: box.x + box.width / 2 - 3, y: box.y + box.height / 2 - 3, width: 6, height: 6 },
  });
}

/** Fills the active layer with the foreground colour (opaque black). */
async function fill(page: Page, mode: Mode): Promise<void> {
  await page.keyboard.press('g');
  const box = await canvasOf(page, mode).boundingBox();
  if (!box) throw new Error('canvas is not visible');
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
}

test('pixel: a failing preview keeps the panel open with its values (AC-16)', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 32, height: 32 });
  await fill(page, 'pixel');
  const filled = await centre(page, 'pixel');
  // Pixel mode previews on the edit target, which a lock refuses.
  await page
    .getByRole('button', { name: /^Lock / })
    .first()
    .click();

  await menu(page, 'Adjustments', 'Levels…');
  const panel = page.getByRole('dialog', { name: 'Levels' });
  const status = page.getByText(/Preview failed: .*locked/);
  await expect(status).toBeVisible();
  const field = panel.getByLabel('Input black value', { exact: true });
  await field.fill('40');
  // Every re-run of the preview fails the same way; the panel stays.
  await expect(status).toBeVisible();
  await page.waitForTimeout(200);
  await expect(panel).toBeVisible();
  await expect(field).toHaveValue('40');
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();

  await panel.getByRole('button', { name: 'Cancel' }).click();
  await expect.poll(async () => (await centre(page, 'pixel')).equals(filled)).toBe(true);
});

test.describe('image mode', () => {
  // Image mode previews even a locked layer, and every parameter vector the
  // panel sends is valid; a failing preview is forced at the core boundary:
  // the page's wasm export `preview_effect` throws while `__failPreview` is
  // set. Previews run in the job worker, which an init script cannot reach,
  // so the worker is kept from starting: a preview then falls back to the
  // page (performance-background-jobs AC-09), and fails there.
  test.beforeEach(async ({ page }) => {
    await page.route(/\/assets\/worker-[^/]*\.js$/, (route) => route.abort());
    await page.addInitScript(() => {
      const w = window as unknown as { __failPreview?: boolean };
      // `instantiate` answers a compiled module with an Instance, bytes with
      // `{ instance, module }`; the page's core is compiled ahead.
      const patch = <T>(result: T): T => {
        const isInstance = result instanceof WebAssembly.Instance;
        const instance = isInstance
          ? (result as WebAssembly.Instance)
          : (result as { instance: WebAssembly.Instance }).instance;
        const exports = instance.exports;
        if (typeof exports.preview_effect !== 'function') return result;
        const original = exports.preview_effect as (...args: unknown[]) => unknown;
        const patched: Record<string, unknown> = { ...exports };
        patched.preview_effect = (...args: unknown[]) => {
          if (w.__failPreview) throw new Error('forced preview failure');
          return original(...args);
        };
        // Still an Instance to wasm-bindgen's loader, with the patched exports.
        const fake = Object.create(WebAssembly.Instance.prototype) as WebAssembly.Instance;
        Object.defineProperty(fake, 'exports', { value: patched });
        return (isInstance ? fake : { ...result, instance: fake }) as T;
      };
      const streaming = WebAssembly.instantiateStreaming.bind(WebAssembly);
      WebAssembly.instantiateStreaming = async (source, imports) =>
        patch(await streaming(source, imports));
      const instantiate = WebAssembly.instantiate.bind(WebAssembly) as (
        bytes: BufferSource,
        imports?: WebAssembly.Imports,
      ) => Promise<WebAssembly.WebAssemblyInstantiatedSource>;
      (WebAssembly as { instantiate: unknown }).instantiate = async (
        bytes: BufferSource,
        imports?: WebAssembly.Imports,
      ) => patch(await instantiate(bytes, imports));
    });
  });

  const failPreview = (page: Page, on: boolean) =>
    page.evaluate(
      (v) => ((window as unknown as { __failPreview?: boolean }).__failPreview = v),
      on,
    );

  test('a failing preview keeps the panel open with its values (AC-16)', async ({ page }) => {
    await createDocument(page, 'image', { width: 32, height: 32 });
    await fill(page, 'image');
    const filled = await centre(page, 'image');

    await failPreview(page, true);
    await menu(page, 'Effects', 'Blurs', 'Gaussian Blur…');
    const panel = page.getByRole('dialog', { name: 'Gaussian Blur' });
    const notice = page.getByText('Preview failed: forced preview failure');
    await expect(notice).toBeVisible();
    const field = panel.getByLabel('Radius value', { exact: true });
    await field.fill('7');
    await page.waitForTimeout(200);
    await expect(panel).toBeVisible();
    await expect(field).toHaveValue('7');
    await expect(notice).toBeVisible();
    // The document is untouched: the canvas shows the plain fill.
    expect((await centre(page, 'image')).equals(filled)).toBe(true);
    await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();

    // The next preview that works clears the notice.
    await failPreview(page, false);
    await field.fill('8');
    await expect(notice).toHaveCount(0);
    await panel.getByRole('button', { name: 'Cancel' }).click();
    await expect.poll(async () => (await centre(page, 'image')).equals(filled)).toBe(true);
  });
});
