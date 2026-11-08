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

import { readFileSync } from 'node:fs';
import { basename } from 'node:path';
import { customSections, wasmFiles } from './wasm-sections.mjs';

/** Why `bytes` is not an optimized release build; empty when it is. */
function releaseProblems(bytes) {
  const names = customSections(bytes).map((s) => s.name);
  const problems = [];
  if (names.includes('name')) problems.push('has a `name` section (wasm-opt did not run)');
  if (names.some((n) => n.startsWith('.debug_'))) problems.push('has DWARF debug sections (dev build)');
  return problems;
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
