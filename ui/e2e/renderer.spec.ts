import { expect, test, type Page } from '@playwright/test';
import { createDocument, pixelCanvas } from './helpers';

// Pixel-mode backend choice (docs/specs/pixel-compose.md AC-25, AC-26).
// Headless Chromium offers `navigator.gpu` but no adapter, so the WebGPU
// branch runs against a recording stand-in for the device: it proves which
// backend the editor picks and that frames reach it, not GPU output.

interface GpuLog {
  adapterRequests: number;
  writes: number;
  submits: number;
}

/** Replaces `navigator.gpu` before the app loads; `working: false` offers no adapter. */
function installFakeWebGpu(working: boolean): void {
  const log: GpuLog = { adapterRequests: 0, writes: 0, submits: 0 };
  Object.defineProperty(window, '__fakeGpu', { value: log });
  const noop = (): void => undefined;
  const texture = { createView: () => ({}), destroy: noop };
  const device = {
    lost: new Promise<never>(noop),
    queue: {
      writeTexture: () => void log.writes++,
      submit: () => void log.submits++,
    },
    createShaderModule: () => ({}),
    createRenderPipeline: () => ({ getBindGroupLayout: () => ({}) }),
    createSampler: () => ({}),
    createTexture: () => texture,
    createBindGroup: () => ({}),
    createCommandEncoder: () => ({
      beginRenderPass: () => ({ setPipeline: noop, setBindGroup: noop, draw: noop, end: noop }),
      finish: () => ({}),
    }),
    destroy: noop,
  };
  const gpu = {
    requestAdapter: () => {
      log.adapterRequests++;
      return Promise.resolve(working ? { requestDevice: () => Promise.resolve(device) } : null);
    },
    getPreferredCanvasFormat: () => 'bgra8unorm',
  };
  Object.defineProperty(navigator, 'gpu', { configurable: true, value: gpu });
  const globals = window as unknown as Record<string, unknown>;
  globals.GPUTextureUsage ??= { COPY_DST: 0x02, TEXTURE_BINDING: 0x04 };
  const getContext = HTMLCanvasElement.prototype.getContext;
  Object.defineProperty(HTMLCanvasElement.prototype, 'getContext', {
    configurable: true,
    value: function (this: HTMLCanvasElement, type: string, ...rest: unknown[]): unknown {
      if (type === 'webgpu') return { configure: noop, getCurrentTexture: () => texture };
      return Reflect.apply(getContext, this, [type, ...rest]);
    },
  });
}

function gpuLog(page: Page): Promise<GpuLog> {
  return page.evaluate(() => ({ ...(window as unknown as { __fakeGpu: GpuLog }).__fakeGpu }));
}

function backendLabel(page: Page, name: 'webgpu' | 'canvas2d') {
  return page.locator('footer').getByText(name, { exact: true });
}

test('pixel mode: a working WebGPU device draws the canvas through WebGPU', async ({ page }) => {
  await page.addInitScript(installFakeWebGpu, true);
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  await expect(backendLabel(page, 'webgpu')).toBeVisible();
  await expect(backendLabel(page, 'canvas2d')).toHaveCount(0);
  const first = await gpuLog(page);
  expect(first.writes).toBeGreaterThan(0);
  expect(first.submits).toBeGreaterThan(0);

  // A stroke reaches the canvas through the same device.
  await page.getByTitle('Pencil (B)').click();
  await pixelCanvas(page).click();
  await expect.poll(async () => (await gpuLog(page)).submits).toBeGreaterThan(first.submits);
});

test('pixel mode: without a WebGPU adapter the canvas falls back to Canvas2D', async ({ page }) => {
  await page.addInitScript(installFakeWebGpu, false);
  await createDocument(page, 'pixel', { width: 8, height: 8 });
  await expect(backendLabel(page, 'canvas2d')).toBeVisible();
  await expect(backendLabel(page, 'webgpu')).toHaveCount(0);
  expect((await gpuLog(page)).adapterRequests).toBeGreaterThan(0);
});

test('pixel mode: ?renderer=canvas2d forces Canvas2D although WebGPU works', async ({ page }) => {
  await page.addInitScript(installFakeWebGpu, true);
  await createDocument(page, 'pixel', { width: 8, height: 8 }, './?renderer=canvas2d');
  await expect(backendLabel(page, 'canvas2d')).toBeVisible();
  await expect(backendLabel(page, 'webgpu')).toHaveCount(0);
  const log = await gpuLog(page);
  expect(log.adapterRequests).toBe(0);
  expect(log.writes).toBe(0);
});
