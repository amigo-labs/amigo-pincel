#!/usr/bin/env node
// Fails when a wasm module about to ship is not built with 128-bit wasm SIMD
// (docs/specs/performance-build-simd.md AC-02). rustc records the target
// features a module was compiled with in its `target_features` custom
// section; a build with `-C target-feature=+simd128` lists `+simd128`.
// Run it on wasm-pack's output, before `wasm-opt`: a SIMD instruction cannot
// be optimized away silently (wasm-opt fails to validate it unless SIMD is
// enabled), so a module that passes here ships with SIMD.
//
// Usage: node scripts/check-wasm-simd.mjs <file.wasm | directory>...
// A directory is searched for `*.wasm` files (not recursively). Exit code 1
// names every offending module; 2 when there is nothing to check.

import { readFileSync } from 'node:fs';
import { basename } from 'node:path';
import { customSections, leb, wasmFiles } from './wasm-sections.mjs';

/** The features listed in the module's `target_features` section, as
 *  `+name` / `-name` / `=name`; null without such a section. */
function targetFeatures(bytes) {
  const section = customSections(bytes).find((s) => s.name === 'target_features');
  if (!section) return null;
  const features = [];
  let [count, at] = leb(bytes, section.start);
  for (; count > 0; count--) {
    const prefix = String.fromCharCode(bytes[at++]);
    const [len, start] = leb(bytes, at);
    features.push(prefix + new TextDecoder().decode(bytes.subarray(start, start + len)));
    at = start + len;
  }
  return features;
}

/** Why `bytes` does not use wasm SIMD; empty when it does. */
function simdProblems(bytes) {
  const features = targetFeatures(bytes);
  if (features === null) return ['has no `target_features` section'];
  if (!features.includes('+simd128')) return ['simd128 is not among its target features'];
  return [];
}

const files = wasmFiles(process.argv.slice(2));
if (files.length === 0) {
  console.error('check-wasm-simd: no .wasm files to check');
  process.exit(2);
}
let failed = false;
for (const file of files) {
  let problems;
  try {
    problems = simdProblems(readFileSync(file));
  } catch (err) {
    problems = [err instanceof Error ? err.message : String(err)];
  }
  if (problems.length > 0) {
    failed = true;
    console.error(`check-wasm-simd: ${basename(file)} does not use wasm SIMD: ${problems.join('; ')}`);
  } else {
    console.log(`check-wasm-simd: ${basename(file)} ok`);
  }
}
process.exit(failed ? 1 : 0);
