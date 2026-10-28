---
id: todo-copy-cut-paste
title: Copy, Cut and Paste in both modes
status: todo
size: L (three M sessions)
mode: Pixel + Image
depends_on: []
created: 2026-10-28
---

# Copy, Cut and Paste in both modes

## Goal

Edit ▸ Cut / Copy / Paste with Ctrl+X / C / V (⌘ on macOS) in **both** modes, going through the system clipboard as PNG. Pasting into the other mode works too, and so does pasting into another app such as a chat window or paint.net.

## Why

Every competitor has this. Today:
- **neither mode can Copy or Cut**;
- Image mode can Paste (PR #87, §5.4), but only at the top-left;
- Pixel mode can't Paste at all.

## Current state (read these first)

**Image mode**
- **Paste.** `crates/pincel-image-core/src/command/paste.rs`
  - `PasteLayer { pixels, expand, layer, prev: Option<DocSnapshot> }` inserts above the active layer at (0, 0), optionally after `ResizeCanvas`, in one undo step.
  - wasm `paste_layer(handle, data, expand)` (`crates/pincel-image-wasm/src/lib.rs` ≈L155).
  - UI: `beginPaste` / `pasteFromClipboard` / `onPaste` in `ui/src/modes/image/ImageEditor.svelte` (≈L123–160), and the Edit menu Paste item.
- **Clipboard helpers.** `ui/src/lib/clipboard.ts` (`clipboardImage`, `pasteEventImage`, `imageSize`) can only read; there is no write helper.
- **Selection.** `crates/pincel-image-core/src/selection/mod.rs`
  - `SelectionMask` holds coverage `u8` per pixel; `bounding_box` (≈L167).
  - `masked_region(before, after, mask)` (≈L25) crops to the bbox and blends feathered edges.
  - `Document.selection` (`document/mod.rs` ≈L74).
- **Buffers.** `ImageBuffer::copy_region` / `paste_region` (`document/buffer.rs` ≈L127/151) need the rect fully inside the buffer.
- **Encoding.** `codec::to_png_bytes(buf, compression)` (`codec/mod.rs` ≈L124).
- **Delete.** `CommandSpec::DeleteSelection` already exists. Cut is Copy plus this.

**Pixel mode**
- **Selection.**
  - `Sprite.selection: Option<Rect>` plus `selection_mask: Option<SelectionMask>` (`crates/pincel-core/src/document/mod.rs` ≈L93–97). This is transient and not undo-tracked.
  - `SelectionMask` lives in `crates/pincel-core/src/selection.rs` (`contains`, `bounds`).
- **Delete.** wasm `delete_selection` (`crates/pincel-wasm/src/lib.rs` ≈L869) runs `ClearRegion::new(layer, frame, sel).masked()`. Cut is Copy plus this.
- **Move.** There is **no floating selection**. `MoveSelectionContent` (`command/move_selection_content.rs` ≈L33) moves pixels on release, and the overlay previews the move during the drag.
- **Import PNG as layer** (`crates/pincel-wasm/src/import.rs` ≈L29–54) costs **two undo steps**: `add_layer`, then `ReplaceCelPixels`. It clips to the canvas.
- **Undo bus.** It has no compound command. `AnyCommand` (`command/mod.rs` ≈L138) merges only same-kind pairs.
- **PNG encoding.** `encode_rgba8(pixels, w, h)` in `crates/pincel-core/src/codec/png.rs` (≈L377) is private, so expose it. The workspace has `png` 0.18.
- **Menus and keys.** The Edit menu (`ui/src/modes/pixel/menus.ts` ≈L128–145) has no clipboard items. `runShortcut` is in `ui/src/modes/pixel/PixelEditor.svelte` (≈L336). Pixel mode has no `paste` handler.

**Shortcuts.** `ui/src/lib/shell/shortcuts.ts` has `paste` as a hint-only action (≈L55). There are no `copy` or `cut` actions.

## Design

### Clipboard format and API

- **Write.** `navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })])` from the menu click or the keydown. Both count as user activation.
  - Add `writeClipboardImage(png: Uint8Array): Promise<boolean>` to `ui/src/lib/clipboard.ts`.
- **Internal fallback.** Keep a module-level "last copy" (PNG bytes plus origin `{ x, y }` plus a source tag). It covers browsers or WebViews that refuse image writes (WebKitGTK is unverified; test it) and remembers the position.
  - Paste checks the system clipboard first.
  - If that image's bytes equal the internal copy, paste uses the internal **origin**.
- **Keys.**
  - Add `copy` and `cut` actions to `shortcuts.ts`.
  - Copy and Cut can be handled in keydown (`navigator.clipboard.write`), so there is no need to hijack the `copy` event.
  - Paste stays on the `paste` event, as today.
  - Ignore all three while the target is a text field (`isTypingTarget`).

### What is copied

- **Source.**
  - With a selection: the bbox of the selection on the **active layer** (Pixel: the active layer and frame's cel), with pixels outside the mask transparent. Image mode keeps feathered alpha through `masked_region`.
  - Without a selection: the whole active layer.
  - Optional **Copy Merged** (paint.net and Photoshop have it): copy from the composite instead. Recommendation: include it, it's cheap.
- **Pixel-mode indexed sprites** copy as RGBA through the palette.

### Paste position

Applies to both modes.
1. If the clipboard image is the internal copy, paste at its **origin**, so copy and paste in place works.
2. Otherwise paste at the **top-left of the visible viewport**, clamped into the canvas (paint.net does this).
3. A larger image asks whether to expand the canvas. Reuse the Image-mode dialog wording and i18n keys from PR #87 (`paste.*`). Pixel mode needs its own dialog in its `dialogs` store (`ui/src/modes/pixel/stores/editor.svelte.ts`).

### Core changes

- **Image.** `PasteLayer` gains a position: `PasteLayer::at(pixels, x, y, expand)`.
  - When expanding, the canvas grows to cover `(x, y) + size`, anchored top-left. Paste at negative offsets is clamped by the UI.
  - wasm `paste_layer(handle, data, x, y, expand)`. This changes the API added in PR #87; it isn't published yet.
  - New `copy_selection(handle, merged: bool) -> Vec<u8>` (PNG).
- **Pixel.** New `PasteLayer` command in `crates/pincel-core/src/command/paste_layer.rs` with an `AnyCommand` variant.
  - It adds an image layer above the active one, writes the pixels into a cel on the current frame at `(x, y)`, and optionally grows the canvas with `ReframeCanvas::resize` (`command/reframe_canvas.rs`, top-left anchor).
  - All of that is **one undo step**, done like Image mode's snapshot or by holding the sub-commands and reverting them in reverse.
  - Move `import_png_as_layer` onto it too. That fixes the two-step undo, and the test at `import.rs` ≈L88–101 changes to expect one step.
  - wasm: `copySelection(merged) -> Vec<u8>`, `pasteLayer(bytes, x, y, expand)`.
  - **Indexed sprites.** Paste maps each pixel to the nearest palette entry. Alternatively, refuse with a notice and ask the user which; recommendation is nearest-entry mapping.

## Tasks (one M session each)

1. **`docs(spec)` + clipboard write helper + Image mode Copy / Cut / Copy Merged.**
   - Decision Log entry for paste position, the internal fallback and Copy Merged.
   - `copy_selection` in wasm, with core tests for masked and feathered copy.
   - UI menu items, keys and i18n (`menu.copy`, `menu.cut`, `menu.copyMerged`).
2. **Image mode paste position.**
   - `PasteLayer::at`, the paste origin from the internal copy, and the viewport top-left.
   - Core tests:
     - `paste_at_offset_places_layer`
     - `paste_at_offset_with_expand_grows_to_cover`
   - e2e: copy a selection, paste, and the new layer lands at the same place. The ×-size text and layer count follow the patterns in `ui/e2e/paste.spec.ts`.
3. **Pixel mode Copy / Cut / Paste.**
   - Core `PasteLayer`; `import_png_as_layer` now one undo step; wasm; UI menu, keys, `paste` handler and expand dialog.
   - e2e `ui/e2e/pixel-clipboard.spec.ts`: select, copy, paste → 2 layers; undo → 1 layer; cut clears the source. Use the clipboard-permission pattern from `ui/e2e/clipboard-new.spec.ts`.

## Out of scope

- A floating, movable paste with transform handles (Phase 2 selection transforms, §13 and §17.3 #1264).
- Paste into New Image (Ctrl+Alt+V). This one is cheap: hand the shell an `OpenedFile` built from the PNG. A good follow-up.
- Copying text or SVG.
