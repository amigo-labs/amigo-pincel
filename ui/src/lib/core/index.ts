// Adapter between the UI and the `pincel-wasm` crate (published as
// `@amigo-labs/pincel-wasm`).
//
// The wasm module must be initialized exactly once before any
// `Document` / `ComposeFrame` / `Event` constructor is touched. This
// module owns that lifecycle behind `loadCore()` and re-exports the
// classes for callers; UI code should never import `@amigo-labs/pincel-wasm`
// directly (see CLAUDE.md §5.4).

import init, {
  Document,
  blendModeNames,
  effectNames,
  registerFont,
  type ComposeFrame,
  type Event as PincelEvent,
} from '@amigo-labs/pincel-wasm';
// Vite-native URL import. Using the `?url` query forces Vite to copy
// the binary into the bundle and hand back the asset URL, instead of
// relying on the default `new URL('…', import.meta.url)` pattern that
// fires inside the wasm-pack-generated entry. Both work in dev; the
// explicit form is unambiguous and survives bundling.
import wasmUrl from '@amigo-labs/pincel-wasm/pincel_wasm_bg.wasm?url';

export type { ComposeFrame, PincelEvent };
export { Document, blendModeNames, effectNames, registerFont };

let initPromise: Promise<void> | null = null;

/**
 * Initialize the wasm module. Idempotent and safe to await from
 * multiple call sites.
 */
export function loadCore(): Promise<void> {
  if (!initPromise) {
    initPromise = init({ module_or_path: wasmUrl }).then(() => undefined);
  }
  return initPromise;
}
