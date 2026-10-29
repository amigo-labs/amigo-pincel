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
| AC-01 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: the Edit menu has Cut, Copy, Copy Merged and Paste with their shortcuts; `ui/e2e/image-clipboard.spec.ts` › image: the Edit menu has …; `ui/e2e/image-clipboard.spec.ts` › on macOS › both modes show ⌘X, ⌘C and ⌘V | built |        |
| AC-02 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: Copy puts the selected box of the active layer on the clipboard and changes nothing; `ui/e2e/image-clipboard.spec.ts` › image: Copy puts the selected box on the clipboard and changes nothing; image: Copy keeps only the selected pixels of a shaped selection; `crates/pincel-core/src/clipboard.rs` copy_pixels_with_selection_crops_to_the_bounding_box, copy_pixels_shaped_selection_clears_unselected_pixels, copy_pixels_selection_partly_off_canvas_is_clipped; `crates/pincel-wasm/src/clipboard.rs` copy_selection_takes_the_selected_box_of_the_active_layer_unchanged_doc, copy_selection_shaped_selection_clears_outside_pixels; `crates/pincel-image-core/src/tools/copy.rs` copy_selection_crops_to_the_bounding_box_of_the_active_layer; `crates/pincel-image-wasm/src/lib.rs` copy_selection_is_the_selected_box_and_leaves_the_document_alone | built |        |
| AC-03 | `ui/e2e/image-clipboard.spec.ts` › image: a feathered selection scales the layer alpha by its coverage; `crates/pincel-image-core/src/tools/copy.rs` copy_selection_scales_alpha_by_feathered_coverage, copy_selection_partial_alpha_times_coverage | built |        |
| AC-04 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: Copy without a selection takes the whole active layer at canvas size; `ui/e2e/image-clipboard.spec.ts` › image: Copy without a selection takes the whole layer; Copy Merged the composite; `crates/pincel-core/src/clipboard.rs` copy_pixels_without_selection_takes_the_whole_layer; `crates/pincel-wasm/src/clipboard.rs` copy_selection_without_selection_is_the_whole_layer; `crates/pincel-image-core/src/tools/copy.rs` copy_selection_without_selection_is_the_whole_layer | built |        |
| AC-05 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: Copy Merged takes the same region from the visible composite; `ui/e2e/image-clipboard.spec.ts` › image: Copy without a selection takes the whole layer; Copy Merged the composite; `crates/pincel-core/src/clipboard.rs` copy_pixels_merged_takes_the_visible_composite; `crates/pincel-wasm/src/clipboard.rs` copy_merged_takes_the_visible_composite_of_the_same_region; `crates/pincel-image-core/src/tools/copy.rs` copy_merged_takes_the_composite_of_the_same_region; `crates/pincel-image-wasm/src/lib.rs` copy_selection_merged_and_whole_layer_without_selection | built |        |
| AC-06 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: Cut copies the selection, clears it, and one Undo restores it; `ui/e2e/image-clipboard.spec.ts` › image: Cut copies and clears the selection, without one the whole layer; Undo restores; `crates/pincel-wasm/src/clipboard.rs` cut_selection_copies_then_clears_and_one_undo_restores; `crates/pincel-image-wasm/src/lib.rs` cut_selection_copies_clears_and_one_undo_restores | built |        |
| AC-07 | `crates/pincel-wasm/src/clipboard.rs` copy_selection_from_indexed_sprite_writes_palette_colors; `crates/pincel-core/src/clipboard.rs` copy_pixels_indexed_expands_through_the_palette (no UI path opens an indexed sprite yet: the codec rejects them) | built |        |
| AC-08 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: Pincel’s own copy pastes in place on a new layer above the active one, one undo step; `ui/e2e/image-clipboard.spec.ts` › image: Pincel’s own copy pastes in place on a new layer, one undo step; `crates/pincel-wasm/src/clipboard.rs` paste_layer_lands_above_the_active_layer_at_the_position_in_one_undo_step | built |        |
| AC-09 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: another image lands at the viewport top-left, clamped into the canvas; `ui/e2e/image-clipboard.spec.ts` › image: another image lands at the viewport top-left; Cancel on a larger one changes nothing; `crates/pincel-image-core/src/command/paste.rs` paste_at_places_the_image_at_its_position; `crates/pincel-image-wasm/src/lib.rs` paste_layer_lands_at_the_position_and_expands_to_cover_it | built |        |
| AC-10 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: a larger image asks — Cancel, Keep canvas size, Expand canvas in one undo step; `ui/e2e/image-clipboard.spec.ts` › image: another image lands at the viewport top-left; Cancel on a larger one changes nothing; `ui/e2e/paste.spec.ts` › a larger image expands the canvas in one undo step, Keep canvas size crops the pasted image; `crates/pincel-core/src/command/paste_layer.rs` paste_layer_expand_grows_the_canvas_to_cover_the_image_at_its_position, paste_layer_without_expand_crops_to_the_canvas; `crates/pincel-image-core/src/command/paste.rs` paste_at_with_expand_covers_the_image_at_its_position, paste_at_without_expand_crops_at_the_position | built |        |
| AC-11 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: a larger image asks — …; pixel: Pincel’s own copy pastes in place …; `crates/pincel-core/src/command/paste_layer.rs` paste_layer_inserts_above_the_anchor_on_the_frame_at_the_position, paste_layer_one_undo_removes_layer_pixels_and_expansion; `crates/pincel-wasm/src/clipboard.rs` paste_layer_expand_grows_the_canvas_and_one_undo_shrinks_it | built |        |
| AC-12 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: Import PNG as layer is one undo step; `crates/pincel-wasm/src/import.rs` import_png_as_layer_adds_a_clipped_layer_on_top_in_one_undo_step | built |        |
| AC-13 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: Ctrl+C / X / V in a text field act on the field only; `ui/e2e/image-clipboard.spec.ts` › image: Ctrl+C / X / V in a text field act on the field only | built |        |
| AC-14 | `ui/e2e/image-clipboard.spec.ts` › a copy pastes into the other mode as a new layer at the viewport top-left | built |        |
| AC-15 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: a refused clipboard write keeps the copy in Pincel for Paste, at its origin | built |        |
| AC-16 | manual: needs a third-party application | open |        |
| AC-17 | manual: needs the Linux desktop build with WebKitGTK | open |        |
| AC-18 | `crates/pincel-wasm/src/clipboard.rs` paste_layer_into_indexed_sprite_is_refused_unchanged; `crates/pincel-core/src/command/paste_layer.rs` paste_layer_into_indexed_sprite_is_refused_unchanged; `ui/e2e/pixel-clipboard.spec.ts` › pixel: a paste the core cannot use shows a notice and changes nothing (the notice path; no UI path opens an indexed sprite yet) | built |        |
| AC-19 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: Copy on a group takes its rendered pixels; Cut on it is refused; `crates/pincel-core/src/clipboard.rs` copy_pixels_group_takes_its_rendered_subtree, copy_pixels_tilemap_takes_its_rendered_tiles; `crates/pincel-wasm/src/clipboard.rs` copy_selection_on_a_group_takes_its_rendered_children, copy_selection_on_a_tilemap_takes_its_rendered_tiles | built |        |
| AC-20 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: Copy on a group takes its rendered pixels; Cut on it is refused; `crates/pincel-wasm/src/clipboard.rs` cut_selection_on_group_or_tilemap_is_refused_unchanged | built |        |
| AC-21 | `ui/e2e/pixel-clipboard.spec.ts` › pixel: Cut without a selection takes and clears the whole layer; one Undo restores it; `ui/e2e/image-clipboard.spec.ts` › image: Cut copies and clears the selection, without one the whole layer; Undo restores; `crates/pincel-wasm/src/clipboard.rs` cut_selection_without_selection_clears_the_whole_layer; `crates/pincel-image-wasm/src/lib.rs` cut_selection_without_selection_clears_the_layer | built |        |

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
- 2026-10-29 (spec-build): A Pixel image layer copies its raw cel pixels on the current frame — without layer / cel opacity and whether visible or not, a linked cel followed one hop — while groups and tilemaps go through `compose()` (`LayerFilter::Only`, so hidden children stay out) — because "the active layer's pixels" are what the layer holds, as in Aseprite and paint.net.
- 2026-10-29 (spec-build): AC-07 is built ahead of `indexed-grayscale-compose`: the raw cel read expands indexed (transparent index unless the layer is a background, past-the-palette indices transparent) and grayscale pixels itself; Copy Merged and group / tilemap copies of indexed sprites wait for that spec — because plain Copy never needs `compose()`.
- 2026-10-29 (spec-build): A Pixel selection that runs past the canvas copies only its on-canvas part, and a selection wholly off the canvas copies nothing — because there are no pixels outside the canvas.
- 2026-10-29 (spec-build): The "same image" key is `{w}x{h}` plus a 64-bit FNV-1a hash of the decoded pixels in premultiplied RGBA, exposed as `Document.imageKey` (pincel-wasm) and `image_key` (pincel-image-wasm) — because Chromium round-trips clipboard PNGs through premultiplied bitmaps, which can change the colour of translucent pixels but not their premultiplied value.
- 2026-10-29 (spec-build): The viewport clamp and the fit-shift apply per axis, and Expand is offered when the canvas can grow to cover the image at its position within the mode's limits (Pixel 4096 per edge, Image `clampCanvasSize`); otherwise the dialog offers only Keep with `paste.tooLarge` — because one image can fit on one axis and not the other.
- 2026-10-29 (spec-build): Import PNG as layer keeps its place (top of the stack, at the origin) and only moves onto `PasteLayer`; Paste names its layer `Layer {n}` like Image mode — because the import's placement is not in this spec.
- 2026-10-29 (spec-build): Copy Merged has no shortcut, and Pixel mode's notices go to the footer status line (Image mode: its notice bar) — because AC-01 lists none and each editor already reports there.
- 2026-10-29 (spec-build): New public API — `pincel-core`: `copy_pixels`, `CopySource`, `CopiedPixels`, `CopyError` (module `clipboard`), `PasteLayer`, `PastedImage`, `encode_png`; `pincel-wasm`: `CopiedImage`, `Document.copySelection`, `cutSelection`, `pasteLayer`, `imageKey`; `pincel-image-core`: `copy_selection`, `PasteLayer::at`; `pincel-image-wasm`: `copy_selection`, `cut_selection`, `image_key`, and `paste_layer(handle, data, x, y, expand)` — because the ACs need copy, cut, positioned paste and the clipboard identity on both sides of the boundary.
