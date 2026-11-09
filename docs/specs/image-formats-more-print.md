---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §9.2, pincel.md §9.4, pincel.md §11.3, pincel.md §17.1, pincel.md §15 (2026-05-07 document state in Rust only)]
---

# Print in Image mode

## Context

paint.net has File ▸ Print… (Ctrl+P). Pincel has no print command: Ctrl+P in the browser prints the whole app page — menus, panels and a canvas that may be zoomed or scrolled — and the desktop app does the same through its webview.

The one place Pincel prints today is the keyboard-shortcut overview: a `@media print` block in `ui/src/app.css` hides everything but `.shortcuts-print` while that dialog is open (`shortcut-overview`). Printing the image works the same way: a print-only element holding the flattened composite, shown only for print media, and the platform's print dialog (`window.print()`) for paper, orientation, margins and the preview. The composite comes from the Rust core as an encoded image, as Export ▸ PNG gets it; nothing reads pixels back from the canvas (pincel.md §17.1, AGENTS.md §9).

Tauri 2 documents that `window.print()` works in its webview on all platforms (doc comment of `Webview::print` in `tauri` 2.x), so the desktop app needs no own print path; AC-12 checks it by hand on each platform. On macOS the desktop app has no in-app menu bar (pincel.md §11.3), so File ▸ Print… needs an entry in the native menu there.

This is the third of three specs split from the paint.net parity topic "more file formats and printing" (`image-formats-more`, `image-formats-more-modern`).

## Scope

### In scope

- File ▸ Print… and Ctrl+P / ⌘P in Image mode, opening a small Print dialog
- Two placements: *Fit to page* (default) and *Actual size*
- A print that holds only the flattened composite of the visible layers
- The native macOS menu entry in the desktop app
- Translated strings (English, German)

### Out of scope

- Pixel mode — because the parity target is paint.net; printing sprites needs its own choices (which frame, nearest-neighbour upscale, sprite sheet); owner's choice 2026-11-05
- Paper size, orientation, margins, copies, printer choice, print preview — because the system print dialog provides them on every platform
- Printing at the file's DPI — because Image mode keeps no resolution metadata (`image-files` AC-02 drops it); *Actual size* uses the CSS reference of 96 pixels per inch
- Tiling a large image across several pages (poster printing) — because browsers do not split one image reliably across pages; a tiled layout is its own feature
- Printing the selection only, or a single layer — because paint.net prints the whole image; Copy Merged + Paste as new image covers it
- An MCP print tool — because printing changes no document, and Decision Log 2026-11-05 gives MCP tools to document-changing capabilities only
- Pincel's print layout for a print started from the browser's own menu — because the browser's `beforeprint` cannot wait for the composite to be encoded, so only Pincel's Print command can prepare it
- Remembering the placement choice across sessions — because the default covers the common case; can be added as an IndexedDB pref later

## Acceptance criteria

- [ ] AC-01 (event): When the user chooses File ▸ Print… or presses Ctrl+P (⌘P on macOS) in Image mode with a document open, then a Print dialog opens with the placements *Fit to page* (selected) and *Actual size*, and the buttons *Print* and *Cancel*.
- [ ] AC-02 (state): While the active editor is an Image editor with a document open and no dialog is open over it, Ctrl+P / ⌘P never starts the browser's print of the app page directly: the platform print dialog opens only after *Print* in Pincel's dialog.
- [ ] AC-03 (event): When the user clicks *Print*, then the Print dialog closes and the platform print dialog opens once; under print media the page shows only the image — no menu, tool rail, panel, status bar, tab bar, selection outline, grid or transparency checkerboard.
- [ ] AC-04 (always): The printed image holds the same pixels Export ▸ PNG would write at the moment *Print* is clicked: the composite of the visible layers at the canvas size, independent of zoom, scroll position and active tool; transparent areas show the white print background (on paper: blank, checked by AC-12).
- [ ] AC-05 (option): Where *Fit to page* is selected, the image is scaled up or down, keeping its aspect ratio, to the largest size that fits inside the printable area of one page, centred horizontally and placed at the top; the print is exactly one page.
- [ ] AC-06 (option): Where *Actual size* is selected, the image prints unscaled at 96 pixels per inch (one image pixel per CSS pixel), anchored at the top-left of the printable area.
- [ ] AC-07 (event): When the platform print dialog closes — printed or cancelled — then the print-only content is removed (at the latest when the next print is prepared or the tab closes, should the platform not report the dialog's end), the editor looks and responds as before, and the document, its undo history and its saved / unsaved state are unchanged.
- [ ] AC-08 (event): When the user cancels the Print dialog (*Cancel*, Escape or the close button), then nothing is printed and the editor is as before.
- [ ] AC-09 (state): While no Image document is open, or any dialog is open over the editor (the editor's own dialogs and prompts, effect dialogs, the shell's dialogs, the shortcut overview, the command palette, the Print dialog itself), File ▸ Print… is disabled, Ctrl+P / ⌘P does not open the Print dialog and is left to the browser, and the native Print command does nothing.
- [ ] AC-10 (event): When the desktop app's native menu sends its Print command (macOS: File ▸ Print… with ⌘P), then the active Image editor opens the Print dialog, as AC-01; while the active editor is a Pixel editor or the start screen shows, the command does nothing.
- [ ] AC-11 (error): If the image for the print cannot be prepared — encoding the composite fails, or the encoded image fails to load into the print page — then an error notice says the print could not be prepared, the platform print dialog does not open, and the editor is as before.
- [ ] AC-12 (state, manual: needs a real printer dialog on each desktop platform): While the desktop app runs on Windows, macOS and Linux, *Print* opens the operating system's print dialog, and its preview shows only the image, placed as chosen.
- [ ] AC-13 (always): Every string of the Print dialog, the menu entry and the error notice is translated in English and German.

## Technical notes

- Existing print precedent: `@media print` in `ui/src/app.css` (≈L97) with `#app:has(> .shortcuts-print) > :not(.shortcuts-print) { display: none }`; the print image can use the same pattern with its own class.
- The composite as an encoded image: `core.exportPng(handle, compression)` (`ui/src/modes/image/core/wasm.ts`), as `exportImage` uses it; low compression keeps a large print fast. The image element's object URL is revoked in AC-07.
- *Fit to page* in CSS: `max-width: 100%; max-height: 100vh` alone never enlarges; enlarging a small image needs the box sized to the page with `object-fit: contain`. *Actual size*: `width` / `height` equal to the canvas size in CSS `px` (1 CSS px = 1/96 in in print).
- After the platform dialog closes: `afterprint` (fires in Chromium, WebKit and Firefox; `window.print()` blocks until then in Chromium).
- Image-mode menus are data in `ui/src/modes/image/ImageEditor.svelte` (File menu, after Export); keys in `ui/src/lib/shell/shortcuts.ts` (Ctrl+P is free; the command palette uses Ctrl+K); native menu items in `src-tauri/src/main.rs` (`bound_item(app, "menu:…", label, accelerator)`, labels from the menu language table) and the event union in `ui/src/lib/menu/index.ts`.
- e2e: `page.emulateMedia({ media: 'print' })` shows the print layout, with the viewport as the printable area; a stub of `window.print` counts calls (AC-02, AC-03) and dispatches `afterprint` (AC-07). AC-10 runs against the mocked Tauri bridge (`installTauriMock`, AGENTS.md Gotchas). AC-11 can be provoked by making the PNG export throw through the same seam.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| 1 × 1 px document, *Fit to page* | Prints as one colour filling the page width or height |
| 32000 × 200 px panorama, *Fit to page* | Scaled to the page width, a thin strip; aspect ratio kept |
| 4000 × 3000 px image, *Actual size* | Prints unscaled; what lies beyond the page is cut off by the platform (manual check) |
| Fully transparent document | Prints a blank page |
| Hidden top layer | Not printed (AC-04) |
| Effect dialog with live preview open | Print is disabled while it is open (AC-09) |
| Shortcut overview open, Ctrl+P | Left to the browser, which prints the overview's list as today (AC-09) |
| Pixel and Image tab both open, Pixel tab active | Web, Windows, Linux: Ctrl+P is left to the browser (Pixel mode has no Print); macOS desktop: ⌘P reaches the native item, which does nothing (AC-10) |
| Platform never reports the end of its print dialog | Print-only content is removed with the next print or when the tab closes; it is invisible on screen meanwhile |
| Print started from the browser's own menu (not Pincel's) | Prints the app page as the browser does today |
| Start screen, no editor | Ctrl+P is left to the browser |
| Print twice in a row | Each print shows the current composite; no leftover image from the first |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| *Fit to page* also enlarges small images | The name promises filling the page; *Actual size* covers the unscaled case | Low: a small screenshot prints blurry when enlarged |
| Scaled prints are smoothed, not nearest-neighbour | Image mode is for photos and screenshots; pixel art lives in Pixel mode | Low |
| Ctrl+P is left to the browser outside an active Image editor and while a dialog is open | Taking the key without a Print command would make it do nothing at all, and the shortcut overview relies on the browser's print | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/image-print.spec.ts` "Ctrl+P and File ▸ Print… open the Print dialog, never the platform print directly (AC-01, AC-02, AC-08)" | built | |
| AC-02 | `ui/e2e/image-print.spec.ts` "Ctrl+P and File ▸ Print… open the Print dialog, never the platform print directly (AC-01, AC-02, AC-08)" | built | |
| AC-03 | `ui/e2e/image-print.spec.ts` "Print shows only the composite under print media, fitted to the page, then cleans up (AC-03 – AC-05, AC-07)" | built | |
| AC-04 | `ui/e2e/image-print.spec.ts` "Print shows only the composite under print media, fitted to the page, then cleans up (AC-03 – AC-05, AC-07)" | built | |
| AC-05 | `ui/e2e/image-print.spec.ts` "Print shows only the composite under print media, fitted to the page, then cleans up (AC-03 – AC-05, AC-07)" | built | |
| AC-06 | `ui/e2e/image-print.spec.ts` "Actual size prints one image pixel per CSS pixel (AC-06)" | built | |
| AC-07 | `ui/e2e/image-print.spec.ts` "Print shows only the composite under print media, fitted to the page, then cleans up (AC-03 – AC-05, AC-07)" | built | |
| AC-08 | `ui/e2e/image-print.spec.ts` "Ctrl+P and File ▸ Print… open the Print dialog, never the platform print directly (AC-01, AC-02, AC-08)" | built | |
| AC-09 | `ui/e2e/image-print.spec.ts` "without a document or with a dialog open Ctrl+P is left to the browser (AC-09)" | built | |
| AC-10 | `ui/e2e/image-print.spec.ts` "desktop › the native Print command opens the dialog in Image mode only (AC-10)" | built | |
| AC-11 | `ui/e2e/image-print.spec.ts` "an image that fails to load opens no platform dialog and says so (AC-11)" | built | |
| AC-12 |      | open   |        |
| AC-13 | `ui/e2e/image-print.spec.ts` "the Print dialog and menu entry are German in the German UI (AC-13)" | built | |

## Decisions

- 2026-11-05: Printing goes through the platform print dialog (`window.print()`) on a print-only rendering of the encoded composite, in the browser and the desktop app alike — no own print pipeline, and no canvas readback (pincel.md §17.1).
- 2026-11-05: Printing in Image mode only (owner's choice), because paint.net parity is the goal and Pixel printing needs its own choices (frame, sheet, scale); a follow-up can reuse this mechanism.
- 2026-11-05: The native macOS Print item stays enabled and ignores the command where Print does not apply, because the native menu has no per-item enable state today (only `set_menu_language` / `set_menu_accelerators`); adding one is beyond this spec.
- 2026-11-05: *Fit to page* places the image at the top and keeps the print to one page, because centring vertically depends on the paper size, which the page cannot know, and a spill-over blank page is the common failure of image prints.
- 2026-11-08 (spec-build) — The print image is a `div.pincel-print` appended to `<body>` (not inside `#app`, where the editor is nested deep), shown alone under print media by `body:has(> .pincel-print) > :not(.pincel-print)` and hidden on screen (`ui/src/app.css`); `ui/src/modes/image/core/print.ts` builds it, waits for the image to load, calls `window.print()` once and removes it on `afterprint` or before the next print.
- 2026-11-08 (spec-build) — "A dialog is open" (AC-09) is the editor's own `ui.modalOpen` (which now includes the Print dialog), the help dialogs, the save prompts, and any element with role `dialog` / `alertdialog` (the shell's dialogs); Ctrl+P then returns unhandled, so the browser keeps it.
- 2026-11-08 (spec-build) — The native item is `menu:print` with the rebindable accelerator `CmdOrCtrl+P` (`bound_item`), labelled "Print…" / "Drucken…" in the menu label table; `print` joins `NATIVE_MENU_ACTIONS` so macOS leaves ⌘P to it. `pincel-tauri` cannot be compiled in this container (no GTK); CI's Tauri job builds it.

