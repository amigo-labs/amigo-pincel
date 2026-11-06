#!/usr/bin/env node
// Fails when a wasm module about to ship is not an optimized release build
// (docs/specs/performance-build-simd.md AC-03): `wasm-opt` drops the `name`
// section unless asked to keep debug info (`-g`), and a dev build carries
// DWARF `.debug_*` sections as well, so either section means the module
// skipped `wasm-opt` (or is a dev build).
//
// Usage: node scripts/check-wasm-release.mjs <file.wasm | directory>...
// A directory is searched for `*.wasm` files (not recursively). Exit code 1
// names every offending module; 2 when there is nothing to check.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { basename, join } from 'node:path';

/** Reads an unsigned LEB128 at `at`; returns [value, next offset]. */
function leb(bytes, at) {
  let value = 0;
  let shift = 0;
  let byte;
  do {
    if (at >= bytes.length) throw new Error('truncated LEB128');
    byte = bytes[at++];
    value += (byte & 0x7f) * 2 ** shift;
    shift += 7;
  } while (byte & 0x80);
  return [value, at];
}

/** The names of the custom sections in a wasm binary. */
function customSections(bytes) {
  const magic = [0x00, 0x61, 0x73, 0x6d];
  if (bytes.length < 8 || magic.some((b, i) => bytes[i] !== b)) {
    throw new Error('not a wasm module');
  }
  const names = [];
  let at = 8;
  while (at < bytes.length) {
    const id = bytes[at++];
    const [size, body] = leb(bytes, at);
    if (id === 0) {
      const [len, start] = leb(bytes, body);
      names.push(new TextDecoder().decode(bytes.subarray(start, start + len)));
    }
    at = body + size;
  }
  return names;
}

/** Why `bytes` is not an optimized release build; empty when it is. */
function releaseProblems(bytes) {
  const names = customSections(bytes);
  const problems = [];
  if (names.includes('name')) problems.push('has a `name` section (wasm-opt did not run)');
  if (names.some((n) => n.startsWith('.debug_'))) problems.push('has DWARF debug sections (dev build)');
  return problems;
}

function wasmFiles(paths) {
  return paths.flatMap((p) =>
    statSync(p).isDirectory()
      ? readdirSync(p)
          .filter((f) => f.endsWith('.wasm'))
          .map((f) => join(p, f))
      : [p],
  );
}

const files = wasmFiles(process.argv.slice(2));
if (files.length === 0) {
  console.error('check-wasm-release: no .wasm files to check');
  process.exit(2);
}
let failed = false;
for (const file of files) {
  let problems;
  try {
    problems = releaseProblems(readFileSync(file));
  } catch (err) {
    problems = [err instanceof Error ? err.message : String(err)];
  }
  if (problems.length > 0) {
    failed = true;
    console.error(`check-wasm-release: ${basename(file)} is not an optimized release build: ${problems.join('; ')}`);
  } else {
    console.log(`check-wasm-release: ${basename(file)} ok`);
  }
}
process.exit(failed ? 1 : 0);
