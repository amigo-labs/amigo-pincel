// Reading the custom sections of a wasm binary, for the build checks
// `check-wasm-release.mjs` and `check-wasm-simd.mjs`.

import { readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

/** Reads an unsigned LEB128 at `at`; returns [value, next offset]. */
export function leb(bytes, at) {
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

/** The custom sections of a wasm binary: each one's name and the offsets
 *  of its payload (after the name) in `bytes`. */
export function customSections(bytes) {
  const magic = [0x00, 0x61, 0x73, 0x6d];
  if (bytes.length < 8 || magic.some((b, i) => bytes[i] !== b)) {
    throw new Error('not a wasm module');
  }
  const sections = [];
  let at = 8;
  while (at < bytes.length) {
    const id = bytes[at++];
    const [size, body] = leb(bytes, at);
    if (id === 0) {
      const [len, start] = leb(bytes, body);
      const name = new TextDecoder().decode(bytes.subarray(start, start + len));
      sections.push({ name, start: start + len, end: body + size });
    }
    at = body + size;
  }
  return sections;
}

/** The `*.wasm` files among `paths`; a directory is searched for them
 *  (not recursively). */
export function wasmFiles(paths) {
  return paths.flatMap((p) =>
    statSync(p).isDirectory()
      ? readdirSync(p)
          .filter((f) => f.endsWith('.wasm'))
          .map((f) => join(p, f))
      : [p],
  );
}
