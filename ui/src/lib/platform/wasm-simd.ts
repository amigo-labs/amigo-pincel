// Both wasm cores are built with 128-bit wasm SIMD and no scalar build
// ships (docs/specs/performance-build-simd.md AC-07): the start screen
// checks for it before any editor loads.

// The smallest module that uses a SIMD instruction: one function that
// splats an i32 into a v128 (`i32.const 0; i8x16.splat; i8x16.popcnt`).
const SIMD_PROBE = new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123, 3, 2, 1, 0, 10, 10, 1, 8, 0, 65, 0, 253, 15,
  253, 98, 11,
]);

/** Whether this browser runs wasm with fixed-width SIMD. */
export function supportsWasmSimd(): boolean {
  try {
    return typeof WebAssembly === 'object' && WebAssembly.validate(SIMD_PROBE);
  } catch {
    return false;
  }
}
