// Runtime detection of the host shell.
//
// The PWA and the Tauri desktop build share one Svelte UI; this
// module is the single point of branching. Adapters in lib/fs/,
// lib/idb/, etc. dispatch on `isTauri()` to swap their FSA / IDB
// implementations for native equivalents (commands wired in M11.2).
//
// See docs/specs/pincel.md §11.4.

// Tauri 2 sets `__TAURI_INTERNALS__` on `window` once the WebView
// has loaded the runtime bridge. Tauri 1 used `__TAURI__`; we accept
// both for safety, though only the v2 path ever fires in this build.
interface TauriWindow {
  __TAURI_INTERNALS__?: unknown;
  __TAURI__?: unknown;
}

/** True when running inside the Tauri desktop shell. False in any
 *  browser context (PWA included) and during SSR. */
export function isTauri(): boolean {
  if (typeof window === 'undefined') return false;
  const w = window as Window & TauriWindow;
  return (
    typeof w.__TAURI_INTERNALS__ !== 'undefined' ||
    typeof w.__TAURI__ !== 'undefined'
  );
}

/** True on macOS / iPadOS. Read from the user-agent platform token, which
 *  is what every engine keeps through UA reduction and what `App.svelte`
 *  already sniffs for the file-association prompt. */
export function isMac(): boolean {
  return typeof navigator !== 'undefined' && /Mac|iPhone|iPad/i.test(navigator.userAgent);
}

/** True when the OS draws a native menu bar for this window: the desktop
 *  app on macOS, where the menu bar belongs to the screen rather than the
 *  window. That menu renders File / Edit / View / Help itself and owns
 *  their accelerators (spec §9.4), so the in-app bar leaves both to it.
 *
 *  Everywhere else — the web build, and the desktop app on Windows and
 *  Linux — the in-app menu bar is the only one, and the editors bind the
 *  chords themselves. See docs/specs/pincel.md §11.3. */
export function hasNativeMenu(): boolean {
  return isTauri() && isMac();
}
