// Default text face for the Text tool: Liberation Sans (SIL OFL 1.1),
// shipped under public/fonts/ and registered with the wasm module once.
// Core never touches the file system (spec §15, 2026-09-06), so the UI
// fetches the bytes and hands them over.

import { registerFont } from '../core';

let fontIdPromise: Promise<number> | null = null;

/** Resolve the registered id of the bundled default font (fetched once). */
export function loadDefaultFont(): Promise<number> {
  if (!fontIdPromise) {
    fontIdPromise = (async () => {
      const url = `${import.meta.env.BASE_URL}fonts/LiberationSans-Regular.ttf`;
      const res = await fetch(url);
      if (!res.ok) throw new Error(`font fetch failed: ${res.status} ${url}`);
      const bytes = new Uint8Array(await res.arrayBuffer());
      return registerFont(bytes);
    })().catch((err: unknown) => {
      // Let a later attempt retry instead of caching the failure.
      fontIdPromise = null;
      throw err;
    });
  }
  return fontIdPromise;
}
