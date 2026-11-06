import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { crc32, deflateSync } from 'node:zlib';
import { expect, test } from '@playwright/test';
import { defaultValues, EFFECTS, forMode, toParams, type EffectMode } from '../src/lib/effects/catalog';

// docs/specs/performance-build-simd.md AC-02: the wasm SIMD build and the
// scalar build of both cores compose and run every effect to the same bytes.
// Runs in Node (no page) against two sets of packages:
//   SIMD   — `PINCEL_SIMD_PKGS`   or `crates/<crate>/pkg` (what ships),
//   scalar — `PINCEL_SCALAR_PKGS` or `target/scalar-pkg/<crate>`
//            (`pnpm wasm:build:scalar`).
// Both should be release builds: without optimisation the compiler does not
// vectorise, and the SIMD build would barely use SIMD. `pnpm simd:check`
// runs it; CI runs it in the nightly perf and the release workflows.

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const SIMD_ROOT = process.env.PINCEL_SIMD_PKGS;
const SCALAR_ROOT = process.env.PINCEL_SCALAR_PKGS ?? join(ROOT, 'target', 'scalar-pkg');

const simdDir = (crate: string): string =>
  SIMD_ROOT ? join(SIMD_ROOT, crate) : join(ROOT, 'crates', crate, 'pkg');
const scalarDir = (crate: string): string => join(SCALAR_ROOT, crate);

// --- Input pixels -----------------------------------------------------------

const W = 160;
const H = 120;

/** Deterministic RGBA8 test pixels: gradients, noise and varied alpha. */
function pixels(seed: number): Uint8Array {
  const out = new Uint8Array(W * H * 4);
  let s = seed;
  const rand = (): number => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s >>> 24;
  };
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const i = (y * W + x) * 4;
      out[i] = (x * 255) / (W - 1);
      out[i + 1] = (y * 255) / (H - 1);
      out[i + 2] = rand();
      // Opaque, transparent and partial alpha bands.
      const band = (x + y + seed) % 40;
      out[i + 3] = band < 20 ? 255 : band < 25 ? 0 : rand();
    }
  }
  return out;
}

/** The pixels as an RGBA8 PNG. */
function png(rgba: Uint8Array): Uint8Array {
  const chunk = (type: string, data: Buffer): Buffer => {
    const head = Buffer.alloc(8);
    head.writeUInt32BE(data.length, 0);
    head.write(type, 4, 'latin1');
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])), 0);
    return Buffer.concat([head, data, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(W, 0);
  ihdr.writeUInt32BE(H, 4);
  ihdr.set([8, 6, 0, 0, 0], 8);
  const rows = Buffer.alloc(H * (W * 4 + 1));
  for (let y = 0; y < H; y++) rows.set(rgba.subarray(y * W * 4, (y + 1) * W * 4), y * (W * 4 + 1) + 1);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(rows)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

const LAYERS = [png(pixels(1)), png(pixels(7)), png(pixels(13)), png(pixels(21))];

/** Wire parameters of every catalog effect at `mode`'s defaults. */
function effects(mode: EffectMode): { name: string; params: Float64Array }[] {
  return EFFECTS.map((def) => {
    const shown = forMode(def, mode);
    return { name: def.id, params: Float64Array.from(toParams(shown, defaultValues(shown))) };
  });
}

// --- Running a battery on one build ------------------------------------------

/** One output of a battery: what produced it and its bytes. */
type Output = { readonly label: string; readonly bytes: Uint8Array };

interface ImageCore {
  initSync(input: { module: Uint8Array }): unknown;
  open_image(data: Uint8Array, mime: string): number;
  paste_layer(handle: number, data: Uint8Array, x: number, y: number, expand: boolean): void;
  apply_command(handle: number, command: string): void;
  composite(handle: number): Uint8ClampedArray;
  preview_effect(handle: number, layer: number, name: string, params: Float64Array): Uint8ClampedArray;
  close_document(handle: number): void;
}

interface ComposeFrame {
  readonly pixels: Uint8Array;
  free(): void;
}

interface PixelDocument {
  pasteLayer(bytes: Uint8Array, x: number, y: number, expand: boolean): number;
  setLayerBlendMode(layer: number, mode: string): void;
  setLayerOpacity(layer: number, opacity: number): void;
  compose(frame: number, zoom: number): ComposeFrame;
  previewEffect(name: string, params: Float64Array, zoom: number): ComposeFrame;
  free(): void;
}

interface PixelCore {
  initSync(input: { module: Uint8Array }): unknown;
  blendModeNames(): string[];
  Document: { openPng(bytes: Uint8Array): PixelDocument };
}

const IMAGE_BLENDS = ['multiply', 'screen', 'overlay', 'soft_light', 'color_dodge', 'difference'];

async function load<T>(dir: string, name: string): Promise<T> {
  const js = join(dir, `${name}.js`);
  if (!existsSync(js)) {
    throw new Error(`${js} is missing; build it first (pnpm wasm:build:release, pnpm wasm:build:scalar)`);
  }
  const mod = (await import(pathToFileURL(js).href)) as T & {
    initSync(input: { module: Uint8Array }): unknown;
  };
  mod.initSync({ module: readFileSync(join(dir, `${name}_bg.wasm`)) });
  return mod;
}

async function imageBattery(dir: string): Promise<Output[]> {
  const core = await load<ImageCore>(dir, 'pincel_image_wasm');
  const [base, ...rest] = LAYERS;
  const doc = core.open_image(base as Uint8Array, 'image/png');
  try {
    const blends: Output[] = [];
    rest.forEach((layer, i) => core.paste_layer(doc, layer, (i * 13) % 40, (i * 7) % 30, false));
    for (const [i, mode] of IMAGE_BLENDS.entries()) {
      const index = 1 + (i % rest.length);
      core.apply_command(doc, JSON.stringify({ type: 'set_layer_blend_mode', index, mode }));
      core.apply_command(doc, JSON.stringify({ type: 'set_layer_opacity', index, opacity: 0.4 + 0.1 * i }));
      blends.push({ label: `image compose (${mode})`, bytes: Uint8Array.from(core.composite(doc)) });
    }
    const fx = effects('image').map(({ name, params }) => ({
      label: `image effect ${name}`,
      bytes: Uint8Array.from(core.preview_effect(doc, 1, name, params)),
    }));
    return [...blends, ...fx];
  } finally {
    core.close_document(doc);
  }
}

async function pixelBattery(dir: string): Promise<Output[]> {
  const core = await load<PixelCore>(dir, 'pincel_wasm');
  const [base, ...rest] = LAYERS;
  const doc = core.Document.openPng(base as Uint8Array);
  const frame = (f: ComposeFrame): Uint8Array => {
    const bytes = Uint8Array.from(f.pixels);
    f.free();
    return bytes;
  };
  try {
    const ids = rest.map((layer, i) => doc.pasteLayer(layer, (i * 13) % 40, (i * 7) % 30, false));
    const modes = core.blendModeNames().filter((m) => m !== 'normal');
    const out: Output[] = [];
    for (const [i, mode] of modes.entries()) {
      const id = ids[i % ids.length] as number;
      doc.setLayerBlendMode(id, mode);
      doc.setLayerOpacity(id, 96 + ((i * 37) % 160));
      out.push({ label: `pixel compose (${mode})`, bytes: frame(doc.compose(0, 1)) });
    }
    for (const { name, params } of effects('pixel')) {
      out.push({ label: `pixel effect ${name}`, bytes: frame(doc.previewEffect(name, params, 1)) });
    }
    return out;
  } finally {
    doc.free();
  }
}

/** The labels of the outputs that differ between two batteries. */
function differing(a: Output[], b: Output[]): string[] {
  expect(b.map((o) => o.label)).toEqual(a.map((o) => o.label));
  return a.filter((o, i) => !Buffer.from(o.bytes).equals(Buffer.from(b[i]?.bytes ?? []))).map((o) => o.label);
}

function simdCheck(dir: string): number | null {
  return spawnSync(process.execPath, [join(ROOT, 'scripts', 'check-wasm-simd.mjs'), dir]).status;
}

for (const [crate, battery] of [
  ['pincel-image-wasm', imageBattery],
  ['pincel-wasm', pixelBattery],
] as const) {
  test(`${crate}: the SIMD build composes and runs every effect like the scalar build (AC-02)`, async () => {
    // Compare a SIMD build with a scalar one, not a build with itself.
    expect(simdCheck(simdDir(crate)), `${simdDir(crate)} is not a SIMD build`).toBe(0);
    expect(simdCheck(scalarDir(crate)), `${scalarDir(crate)} is not a scalar build`).toBe(1);
    const simd = await battery(simdDir(crate));
    const scalar = await battery(scalarDir(crate));
    expect(simd.length).toBeGreaterThan(EFFECTS.length);
    expect(differing(simd, scalar)).toEqual([]);
  });
}
