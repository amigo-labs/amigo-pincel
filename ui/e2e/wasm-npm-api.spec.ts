import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { expect, test } from '@playwright/test';

// The `@amigo-labs/pincel-wasm` package contract (docs/specs/wasm-npm-api.md)
// exercised from Node through the package name the UI links, the way an
// embedder imports it. No browser page: these tests only need the package.

const PACKAGE = '@amigo-labs/pincel-wasm';

interface PackageJson {
  readonly name: string;
  readonly type?: string;
  readonly main?: string;
  readonly module?: string;
}

const require = createRequire(import.meta.url);
const packageJsonPath = require.resolve(`${PACKAGE}/package.json`);

async function readPackageJson(): Promise<PackageJson> {
  return JSON.parse(await readFile(packageJsonPath, 'utf8')) as PackageJson;
}

/** Imports the package and runs its default export with the wasm bytes (Node has no file `fetch`). */
async function loadPackage(): Promise<typeof import('@amigo-labs/pincel-wasm')> {
  const pkg = await import(PACKAGE);
  const wasm = await readFile(join(dirname(packageJsonPath), 'pincel_wasm_bg.wasm'));
  await pkg.default({ module_or_path: wasm });
  return pkg;
}

test.describe('@amigo-labs/pincel-wasm package', () => {
  test('is published under the scoped name as an ES module', async () => {
    const manifest = await readPackageJson();
    expect(manifest.name).toBe(PACKAGE);
    expect(manifest.type).toBe('module');
    const entry = manifest.module ?? manifest.main;
    expect(entry).toBeTruthy();
    const source = await readFile(join(dirname(packageJsonPath), entry ?? ''), 'utf8');
    expect(source).toMatch(/^export (class|function|\{)/m);
  });

  test('default export initializes the wasm module', async () => {
    const pkg = await import(PACKAGE);
    expect(typeof pkg.default).toBe('function');
    const wasm = await readFile(join(dirname(packageJsonPath), 'pincel_wasm_bg.wasm'));
    await pkg.default({ module_or_path: wasm });
    const doc = new pkg.Document(3, 2);
    expect([doc.width, doc.height]).toEqual([3, 2]);
    doc.free();
  });

  test('exports the documented classes and functions', async () => {
    const pkg = await loadPackage();
    for (const name of ['Document', 'ComposeFrame', 'AtlasExport', 'Event'] as const) {
      expect(typeof pkg[name], name).toBe('function');
    }
    for (const name of ['palettePresetNames', 'blendModeNames', 'effectNames', 'registerFont'] as const) {
      expect(typeof pkg[name], name).toBe('function');
    }
    expect(pkg.palettePresetNames()).toContain('db32');
    expect(pkg.blendModeNames()).toContain('normal');
    expect(pkg.effectNames().length).toBeGreaterThan(0);

    const doc = new pkg.Document(2, 2);
    doc.applyTool('pencil', 0, 0, 0xff0000ff);
    const events = doc.drainEvents();
    expect(events[0]).toBeInstanceOf(pkg.Event);
    expect(doc.compose(0, 1)).toBeInstanceOf(pkg.ComposeFrame);
    expect(doc.exportAtlas(1)).toBeInstanceOf(pkg.AtlasExport);
    doc.free();
  });

  test('byte reads are fresh copies that JS can change without effect', async () => {
    const pkg = await loadPackage();
    const doc = new pkg.Document(4, 4);
    doc.applyTool('pencil', 1, 2, 0xff0000ff);
    doc.endStroke();

    const frame = doc.compose(0, 1);
    const atlas = doc.exportAtlas(1);
    const composed = Array.from(frame.pixels);
    const reads: ReadonlyArray<readonly [string, () => Uint8Array]> = [
      ['ComposeFrame.pixels', () => frame.pixels],
      ['saveAseprite', () => doc.saveAseprite()],
      ['exportPng', () => doc.exportPng(0)],
      ['AtlasExport.png', () => atlas.png],
    ];

    for (const [label, read] of reads) {
      const original = Array.from(read());
      const first = read();
      expect(first, label).not.toBe(read());
      first.fill(0x5a);
      expect(Array.from(read()), label).toEqual(original);
    }
    // The document itself is untouched: a new compose matches the first one.
    expect(Array.from(doc.compose(0, 1).pixels)).toEqual(composed);
    doc.free();
  });
});
