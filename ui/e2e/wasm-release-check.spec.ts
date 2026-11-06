import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';

// The deploy check that every shipped wasm module went through `wasm-opt`
// (docs/specs/performance-build-simd.md AC-03), run on hand-made modules.

const SCRIPT = join(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
  'scripts',
  'check-wasm-release.mjs',
);

/** An empty wasm module with one custom section per name. */
function wasmWith(...sections: string[]): Buffer {
  const parts = [Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00])];
  for (const name of sections) {
    const label = Buffer.from(name);
    const body = Buffer.concat([Buffer.from([label.length]), label, Buffer.from([1, 2, 3])]);
    parts.push(Buffer.from([0x00, body.length]), body);
  }
  return Buffer.concat(parts);
}

function check(files: Record<string, Buffer>) {
  const dir = test.info().outputPath('wasm');
  mkdirSync(dir, { recursive: true });
  for (const [name, bytes] of Object.entries(files)) writeFileSync(join(dir, name), bytes);
  const run = spawnSync(process.execPath, [SCRIPT, dir], { encoding: 'utf8' });
  return { status: run.status, out: `${run.stdout}${run.stderr}` };
}

test('an optimized release module passes (AC-03)', () => {
  const run = check({ 'core_bg.wasm': wasmWith('producers', 'target_features') });
  expect(run.status, run.out).toBe(0);
});

test('a module that skipped wasm-opt fails and is named (AC-03)', () => {
  const run = check({
    'good_bg.wasm': wasmWith('producers'),
    'raw_bg.wasm': wasmWith('name', 'producers'),
  });
  expect(run.status).toBe(1);
  expect(run.out).toContain('raw_bg.wasm is not an optimized release build');
  expect(run.out).not.toContain('good_bg.wasm is not');
});

test('a dev build fails (AC-03)', () => {
  const run = check({ 'dev_bg.wasm': wasmWith('.debug_info', 'name') });
  expect(run.status).toBe(1);
  expect(run.out).toContain('dev_bg.wasm');
  expect(run.out).toContain('DWARF');
});
