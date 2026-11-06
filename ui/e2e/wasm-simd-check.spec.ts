import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';

// The build check that every shipped wasm module uses 128-bit wasm SIMD
// (docs/specs/performance-build-simd.md AC-02): on hand-made modules, and on
// the packages `pnpm wasm:build` produced for this run.

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const SCRIPT = join(ROOT, 'scripts', 'check-wasm-simd.mjs');

/** An unsigned LEB128 (the values here stay below 128). */
const u = (n: number): Buffer => Buffer.from([n]);

/** A wasm module whose `target_features` section lists `features`
 *  (`+name` / `-name`); without features, no such section at all. */
function wasmWith(features: string[] | null): Buffer {
  const parts = [Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00])];
  if (features) {
    const label = Buffer.from('target_features');
    const entries = features.map((f) => {
      const name = Buffer.from(f.slice(1));
      return Buffer.concat([Buffer.from(f.slice(0, 1)), u(name.length), name]);
    });
    const body = Buffer.concat([u(label.length), label, u(entries.length), ...entries]);
    parts.push(Buffer.from([0x00, body.length]), body);
  }
  return Buffer.concat(parts);
}

function check(args: string[]) {
  const run = spawnSync(process.execPath, [SCRIPT, ...args], { encoding: 'utf8' });
  return { status: run.status, out: `${run.stdout}${run.stderr}` };
}

function checkFiles(files: Record<string, Buffer>) {
  const dir = test.info().outputPath('wasm');
  mkdirSync(dir, { recursive: true });
  for (const [name, bytes] of Object.entries(files)) writeFileSync(join(dir, name), bytes);
  return check([dir]);
}

test('a module built with simd128 passes (AC-02)', () => {
  const run = checkFiles({ 'core_bg.wasm': wasmWith(['+bulk-memory', '+simd128', '+sign-ext']) });
  expect(run.status, run.out).toBe(0);
});

test('a module without simd128 fails and is named (AC-02)', () => {
  const run = checkFiles({
    'fast_bg.wasm': wasmWith(['+simd128']),
    'scalar_bg.wasm': wasmWith(['+bulk-memory', '+sign-ext']),
    'off_bg.wasm': wasmWith(['-simd128']),
    'bare_bg.wasm': wasmWith(null),
  });
  expect(run.status).toBe(1);
  expect(run.out).toContain('scalar_bg.wasm does not use wasm SIMD');
  expect(run.out).toContain('off_bg.wasm does not use wasm SIMD');
  expect(run.out).toContain('bare_bg.wasm does not use wasm SIMD');
  expect(run.out).not.toContain('fast_bg.wasm does not');
});

test('the built wasm packages use simd128 (AC-02)', () => {
  const run = check([
    join(ROOT, 'crates', 'pincel-wasm', 'pkg'),
    join(ROOT, 'crates', 'pincel-image-wasm', 'pkg'),
  ]);
  expect(run.status, run.out).toBe(0);
  expect(run.out).toContain('pincel_wasm_bg.wasm ok');
  expect(run.out).toContain('pincel_image_wasm_bg.wasm ok');
});
