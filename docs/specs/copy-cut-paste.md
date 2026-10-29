---
status: todo
created: 2026-10-28
size: L
mode: both
design: [pincel.md §5.4, pincel.md §9.4, pincel.md §13, pincel.md §17.3, pincel.md §15 (2026-10-28 Image-mode paste as new layer)]
---

# Copy, Cut and Paste in both modes

## Context

Every competitor has clipboard editing. Today neither mode can Copy or Cut; Image mode can Paste (§5.4, Decision Log 2026-10-28), but only at the top-left corner; Pixel mode cannot Paste at all. This spec adds Edit ▸ Cut / Copy / Copy Merged / Paste with Ctrl+X / C / V (⌘ on macOS) to **both** modes, going through the system clipboard as PNG, so a copy can be pasted into the other mode and into other apps (a chat window, paint.net).

Already decided (Decision Log 2026-10-28): Image mode pastes a clipboard image as a new layer above the active one, in one undo step together with an optional canvas expansion; Ctrl+V is served by the browser's `paste` event, Edit ▸ Paste by the async Clipboard API; the wasm layer takes encoded bytes, not JSON.

## Scope

### In scope

- Copy, Cut, Copy Merged and Paste in Pixel and Image mode: Edit menu items, shortcuts, en + de strings
- Writing PNG to the system clipboard, plus an in-app record of the last copy (image and origin) as a fallback and for paste-in-place
- Paste position: at the copy origin for the app's own copy pasted into the document it came from, otherwise at the top-left of the visible viewport
- Pixel mode: paste as a new layer, the expand-canvas dialog, and one undo step for paste and for Import PNG as layer
- Indexed Pixel sprites copy as RGBA

### Out of scope

- A floating, movable paste with transform handles — because it needs selection transforms, which are Phase 2 (§13, §17.3)
- Paste into New Image (Ctrl+Alt+V) — because it is a separate, cheap follow-up (hand the shell an opened file built from the PNG)
- Copying text or SVG — because both editors hold raster content and PNG is the one format both modes and other apps share
- Nearest-palette mapping on paste into an indexed Pixel sprite — Phase 2 with indexed painting (§3.7, AGENTS.md §9)

## Acceptance criteria

- [ ] AC-01 (always): In both modes the Edit menu contains Cut, Copy, Copy Merged and Paste, showing the shortcuts Ctrl+X, Ctrl+C and Ctrl+V (⌘ on macOS) for Cut, Copy and Paste.
- [ ] AC-02 (event): When the user copies (Edit ▸ Copy or the shortcut) while a selection exists, then the system clipboard holds an `image/png` the size of the selection's bounding box with the active layer's pixels (Pixel mode: the active layer's cel on the current frame), pixels outside the selection transparent, and the document is unchanged.
- [ ] AC-03 (event): When the user copies in Image mode with a feathered selection, then each copied pixel's alpha is the layer's alpha scaled by the selection's coverage at that pixel.
- [ ] AC-04 (event): When the user copies with no selection, then the clipboard holds the whole active layer at canvas size.
- [ ] AC-05 (event): When the user chooses Copy Merged, then the clipboard holds the same region as Copy would, taken from the composite of all visible layers instead of the active layer.
- [ ] AC-06 (event): When the user cuts while a selection exists, then the clipboard holds what Copy would, the selected pixels of the active layer are cleared, and one Undo restores them.
- [ ] AC-07 (event): When the user copies from an indexed Pixel sprite, then the PNG holds the RGBA colors the palette gives those pixels. (Depends on `indexed-grayscale-compose`.)
- [ ] AC-08 (event): When the user pastes, into the document it was copied from, the image Pincel last copied, then it lands on a new layer above the active layer at the position it was copied from, as one undo step.
- [ ] AC-09 (event): When the user pastes any other image, then it lands on a new layer above the active layer with its top-left corner at the canvas position shown at the top-left of the visible viewport, never at a negative canvas coordinate and, when the image fits the canvas, shifted so it lies fully inside, as one undo step.
- [ ] AC-10 (event): When the pasted image is larger than the canvas, then, in both modes, a dialog offers *Expand canvas*, *Keep canvas size* and *Cancel* with Image mode's existing wording: Expand grows the canvas (anchored top-left) to cover the pasted image at its position, Keep crops the image to the canvas, Cancel leaves the document unchanged; Expand plus paste is one undo step.
- [ ] AC-11 (event): When the user pastes in Pixel mode, then a new image layer appears above the active layer holding the pasted pixels on the current frame, and one Undo removes the layer, its pixels and any canvas expansion.
- [ ] AC-12 (event): When the user imports a PNG as a layer in Pixel mode, then one Undo removes the imported layer and its pixels.
- [ ] AC-13 (state): While keyboard focus is in a text field, Ctrl/⌘+C, X and V act on the text field only and leave the document and the image clipboard unchanged.
- [ ] AC-14 (event): When the user copies in one mode and pastes in a document of the other mode, then the image arrives as a new layer there, at the viewport top-left (AC-09).
- [ ] AC-15 (error): If the system clipboard refuses the image write, then the copy is still kept inside Pincel and a following Paste in Pincel pastes it at its copy origin.
- [ ] AC-16 (event, manual: needs a third-party application): When the user pastes into another application (a chat window, paint.net) after copying in Pincel, then that application receives the copied image.
- [ ] AC-17 (event, manual: needs the Linux desktop build with WebKitGTK): When the user copies and then pastes in the Linux desktop app, then the paste delivers the copied image, through the system clipboard or the in-app fallback.
- [ ] AC-18 (error): If the user pastes into an indexed Pixel sprite, then the paste is refused with a notice and the document is unchanged.
- [ ] AC-19 (event): When the user copies in Pixel mode while the active layer is a group or a tilemap layer, then the clipboard holds that layer's rendered pixels (a group's composite, a tilemap's rendered tiles).
- [ ] AC-20 (error): If the user cuts in Pixel mode while the active layer is a group or a tilemap layer, then the cut is refused with a notice and the document and the clipboard are unchanged.
- [ ] AC-21 (event): When the user cuts with no selection, then the clipboard holds the whole active layer and the layer is cleared; one Undo restores it.

## Technical notes

Hints from the legacy plan (2026-10-28); paths verified 2026-10-29, line numbers drift.

**Image mode**

- Paste: `crates/pincel-image-core/src/command/paste.rs` — `PasteLayer { pixels, expand, layer, prev: Option<DocSnapshot> }` inserts above the active layer at (0, 0), optionally after `ResizeCanvas`, in one undo step. Planned: `PasteLayer::at(pixels, x, y, expand)`; expanding grows the canvas to cover `(x, y) + size`, anchored top-left.
- wasm `paste_layer(handle, data, expand)` (`crates/pincel-image-wasm/src/lib.rs` ≈L155) becomes `paste_layer(handle, data, x, y, expand)` (unpublished, so the change is free). New `copy_selection(handle, merged: bool) -> Vec<u8>` (PNG).
- UI: `beginPaste` / `pasteFromClipboard` / `onPaste` in `ui/src/modes/image/ImageEditor.svelte` (≈L127–178) and the Edit menu Paste item.
- Selection: `crates/pincel-image-core/src/selection/mod.rs` — `SelectionMask` (coverage `u8` per pixel), `bounding_box` (≈L167), `masked_region(before, after, mask)` (≈L25) crops to the bbox and blends feathered edges. `Document.selection` (`document/mod.rs` ≈L74).
- Buffers: `ImageBuffer::copy_region` / `paste_region` (`document/buffer.rs` ≈L127 / ≈L151) need the rect fully inside the buffer.
- Encoding: `codec::to_png_bytes(buf, compression)` (`codec/mod.rs` ≈L124).
- Delete: `CommandSpec::DeleteSelection` exists (`pincel-image-wasm` ≈L618). Cut = Copy + this.

**Pixel mode**

- Selection: `Sprite.selection: Option<Rect>` plus `selection_mask: Option<SelectionMask>` (`crates/pincel-core/src/document/mod.rs` ≈L93–98), transient, not undo-tracked; `SelectionMask` in `crates/pincel-core/src/selection.rs` (`contains`, `bounds`).
- Delete: wasm `delete_selection` (`crates/pincel-wasm/src/lib.rs` ≈L869) runs `ClearRegion::new(layer, frame, sel).masked()`. Cut = Copy + this.
- No floating selection: `MoveSelectionContent` (`command/move_selection_content.rs`) moves pixels on release.
- Import PNG as layer (`crates/pincel-wasm/src/import.rs` ≈L32) costs two undo steps (`add_layer`, then `ReplaceCelPixels`) and clips to the canvas; its test (`import_png_as_layer_adds_a_clipped_layer_on_top`, ≈L85) changes to expect one step.
- Undo bus has no compound command; `AnyCommand` (`command/mod.rs` ≈L138) merges only same-kind pairs. Planned: `PasteLayer` in `crates/pincel-core/src/command/paste_layer.rs` with an `AnyCommand` variant — add an image layer above the active one, write a cel on the current frame at `(x, y)`, optionally grow the canvas with `ReframeCanvas::resize` (`command/reframe_canvas.rs`, top-left anchor); one undo step via a snapshot or by holding the sub-commands and reverting them in reverse. `import_png_as_layer` moves onto it.
- PNG encoding: `encode_rgba8(pixels, w, h)` in `crates/pincel-core/src/codec/png.rs` (≈L377) is private — expose it. Workspace `png` 0.18.
- wasm planned: `copySelection(merged) -> Vec<u8>`, `pasteLayer(bytes, x, y, expand)`.
- Menus / keys: Edit menu in `ui/src/modes/pixel/menus.ts` (≈L129) has no clipboard items; `runShortcut` in `ui/src/modes/pixel/PixelEditor.svelte` (≈L336); no `paste` handler. The expand dialog goes into the Pixel `dialogs` store (`ui/src/modes/pixel/stores/editor.svelte.ts`).

**Shared**

- `ui/src/lib/clipboard.ts` (`clipboardImage`, `pasteEventImage`, `imageSize`) only reads; add `writeClipboardImage(png: Uint8Array): Promise<boolean>` using `navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })])` from the menu click or keydown (both are user activation).
- In-app last copy: module-level PNG bytes + origin `{ x, y }` + source tag (the document id). Paste uses the in-app copy when (a) the last Pincel copy's system write failed or (b) the system clipboard holds no image; otherwise the system image wins, and it counts as Pincel's own copy if its decoded size and RGBA hash equal the internal copy (never compare bytes: Chromium re-encodes clipboard PNGs). The copy origin applies only when pasting into the source document.
- Pixel Copy on a group or tilemap layer: `compose()` with `LayerFilter::Only` (the layer plus its subtree), the path thumbnails use.
- `ui/src/lib/shell/shortcuts.ts`: `paste` is hint-only (≈L57); add `copy` and `cut` actions handled in keydown (no `copy` event hijack); Paste stays on the `paste` event. `isTypingTarget` (≈L172) guards text fields.
- i18n: reuse `paste.*` keys (`paste.largerTitle`, `paste.larger`, `paste.tooLarge`, `paste.expand`, `paste.keep`); new `menu.copy`, `menu.cut`, `menu.copyMerged`.
- e2e: Image-mode patterns in `ui/e2e/paste.spec.ts` (×-size text, layer count); clipboard permissions as in `ui/e2e/clipboard-new.spec.ts`; Pixel spec `ui/e2e/pixel-clipboard.spec.ts` (select, copy, paste → 2 layers; undo → 1 layer; cut clears the source). Tests read the clipboard; app code never reads canvas pixels.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Paste with no image on the clipboard (text only, or empty) | Nothing happens; document unchanged |
| Clipboard read permission denied for Edit ▸ Paste | Nothing is pasted, no crash; Ctrl+V through the `paste` event still works |
| Selection that covers only transparent pixels | Copy yields a fully transparent PNG of the bounding-box size |
| Pixel mode: no cel on the active layer at the current frame | Copy yields a transparent PNG of the region; Cut leaves the layer unchanged |
| Pasted image too large to expand the canvas to (beyond the mode's size limits) | Image mode's existing `paste.tooLarge` behavior: the image is cropped to the canvas, with the notice |
| Paste at the layer limit | Refused with the mode's existing layer-limit behavior; document unchanged |
| Copy twice, paste once | The second copy wins, both on the system clipboard and in the in-app record |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |

## Decisions

- The clipboard format is PNG through the async Clipboard API — because it is the one image format browsers, WebViews and other apps all exchange.
- Copy and Cut are handled in keydown / the menu click; Paste stays on the browser's `paste` event — because both are user activation for `clipboard.write`, and the `paste` event needs no read permission (Decision Log 2026-10-28).
- Copy Merged is included — because paint.net and Photoshop have it and it costs only a different pixel source.
- Indexed Pixel sprites copy as RGBA through the palette — because the clipboard PNG must look the same in any target.
- Pixel mode pastes as a new layer in one undo step, and Import PNG as layer moves onto the same command — because it matches Image mode (Decision Log 2026-10-28) and fixes the two-step undo of the import.
- The Image-mode wasm `paste_layer` gains `x, y` — because the API from PR #87 is not published yet, so changing it breaks nobody.
- 2026-10-29: A Decision Log entry records Copy / Cut / Copy Merged / Paste in both modes through the async Clipboard API as PNG, the in-app record (pixels + origin + source document) for failed writes and paste-in-place, the paste position rules, and the single Pixel `PasteLayer` command that Import PNG as layer moves onto; the `pincel.md` §5.4 Paste paragraph is updated in the same commit — AGENTS.md §2.1, and §5.4 still says Pixel mode has no paste (owner accepted the recommendation).
- 2026-10-29: Pasting into an indexed Pixel sprite is refused with a notice (AC-18); nearest-palette mapping comes with indexed painting, and AC-07 depends on `indexed-grayscale-compose` — writing indexed cels is Phase 2 (§3.7, AGENTS.md §9), as every Pixel paint command already rejects indexed cels (owner accepted the recommendation).
- 2026-10-29: Cut without a selection copies the whole active layer and clears it in one undo step (AC-21) — consistent with Copy's no-selection-means-everything rule, and paint.net treats no selection as select-all (owner accepted the recommendation).
- 2026-10-29: Paste uses the in-app copy only when the last system write failed or the system clipboard holds no image; a system image counts as Pincel's own copy when its decoded size and RGBA hash match the in-app record, never by byte comparison — Chromium re-encodes clipboard PNGs, and accepting a small stale-paste risk keeps copy / paste working on WebKitGTK (owner accepted the recommendation).
- 2026-10-29: The copy origin is used only when pasting into the same document (the source tag holds the document id); other documents and the other mode paste at the viewport top-left (AC-08, AC-14) — an origin from another canvas has no meaning in the target and can fall outside it (owner accepted the recommendation).
- 2026-10-29: The viewport paste position is clamped to non-negative coordinates and, when the image fits the canvas, shifted left / up so it lies fully inside; the expand dialog triggers only when the image is wider or taller than the canvas (AC-09, AC-10) — one size test drives the dialog and nothing is silently cropped at the right or bottom edge (owner accepted the recommendation).
- 2026-10-29: Pixel Copy on a group or tilemap layer copies its rendered pixels through `compose()` with `LayerFilter::Only` (AC-19); Cut on them is refused with a notice (AC-20) — one code path for every layer kind, and clearing a group or a tilemap's tiles is not a pixel clear (owner accepted the recommendation).
