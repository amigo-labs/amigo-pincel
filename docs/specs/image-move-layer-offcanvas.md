---
status: todo
created: 2026-11-18
size: M
mode: Image
design: [pincel.md §5.4, pincel.md §15 (2026-11-18 Layer overhang)]
depends_on: [image-move-layer-preview]
---

# Image Move Layer: keep pixels outside the canvas

## Context

In Image mode every raster layer is exactly as large as the canvas. Move Layer writes the moved pixels into that canvas-sized layer on release, so whatever is pushed past the canvas edge is lost: drag a photo half out to the left, release, drag it back — its left half is gone (`image-editing` AC-21: "pixels pushed past the canvas edge are lost").

Owner report and decision 2026-11-18: pixels moved past the edge are kept and come back when the layer is moved back. The layer stays canvas-sized for every tool, effect and the composite; the pixels outside the canvas are kept beside it as the layer's **overhang** (Decision Log 2026-11-18 "Layer overhang"). They persist through undo / redo and in OpenRaster files; autosave is not required to keep them (owner's choice).

## Scope

### In scope

- Move Layer, and Move Selected Pixels with no selection, keep the overhang of raster layers, also when a group is moved (each raster layer in it)
- Undo / redo of every command restores the overhang it changed
- OpenRaster save and open keep the overhang
- The commands listed in AC-05 (geometry, canvas size, merges) discard it; every other command keeps it

### Out of scope

- Move Selected Pixels with a selection and the transform box — because the selection lives in canvas coordinates; lifted content dragged off the canvas stays clipped as today
- Autosave and crash recovery — because the owner chose not to require it (2026-11-18); a recovered document may or may not have the overhang
- Carrying the overhang through Canvas Size, rotations and flips (e.g. a larger canvas revealing it) — because the owner chose "discard" for those commands (2026-11-18)
- Layers of arbitrary size and position (Photoshop model) — because the owner chose the overhang model, which leaves tools, effects and the composite unchanged
- Pixel mode — because its cels already have positions

## Acceptance criteria

- [ ] AC-01 (event): When a raster layer is moved with Move Layer by (dx, dy) so that some of its pixels leave the canvas, and then moved back by (−dx, −dy) in a separate drag, then the layer's pixels equal the pixels before the first move: every pixel with alpha > 0 has the same RGBA value, and every fully transparent pixel stays fully transparent. This holds for any sequence of moves whose offsets sum to zero, as long as no move of the sequence drops pixels by AC-10.
- [ ] AC-02 (always): Outside a Move Layer drag preview (AC-09), pixels in the overhang are never visible: the canvas, the composite, the layer thumbnail, exports to flat formats (PNG, JPEG, WebP, BMP, GIF, TIFF) and Copy show only the pixels inside the canvas.
- [ ] AC-03 (event): When any command other than those of AC-05 and Move Layer — every paint tool (Smudge included), a fill, every effect and adjustment except Rotate / Zoom (blurs and distortions included), Delete, committing a live object or text — changes the active layer's pixels inside the canvas while it has an overhang, then the overhang is unchanged, and a later move carries both the edited pixels and the overhang.
- [ ] AC-04 (event): When a move that changed the overhang is undone, then the layer's pixels and overhang equal their state before the move; redo restores the state after it.
- [ ] AC-05 (event): When a command that moves pixels to other positions (other than Move Layer), changes the canvas size, or merges layers runs on a document with overhang — exactly Image ▸ Canvas Size, Image ▸ Resize, Crop to Selection, image and layer rotations and flips, the Rotate / Zoom effect, a selection transform, Merge Down, Merge Visible and Flatten — then the layers it changes lose their overhang and their in-canvas pixels are what they are today; undo restores the overhang.
- [ ] AC-06 (event): When Duplicate Layer copies a layer with overhang, then the copy has the same overhang.
- [ ] AC-07 (event): When a document with overhang is saved as OpenRaster (.ora) and opened again in Pincel, then every layer's in-canvas pixels and overhang equal those before saving, so AC-01 still holds across the save.
- [ ] AC-08 (always): In an OpenRaster file Pincel writes for a layer with overhang, `stack.xml` keeps the canvas size as the image `w` / `h`, and the layer's PNG covers canvas and overhang, placed by integer `x` / `y` attributes (negative where the overhang lies left of or above the canvas); placing each layer PNG at its `x` / `y` on a canvas-sized transparent buffer and clipping gives exactly Pincel's in-canvas pixels of that layer.
- [ ] AC-09 (state): While a Move Layer drag is in progress (`image-move-layer-preview`), pixels of the overhang that the offset brings inside the canvas show in the preview.
- [ ] AC-10 (error): If a move would put pixels farther outside the canvas than one canvas width to the left or right, or one canvas height above or below, then those pixels are dropped and the rest of the move happens as one undo step; an OpenRaster file opened with layer pixels beyond that limit drops them the same way.
- [ ] AC-11 (always): With the §4.5 memory scenario (4K, 10 layers, 50 undo steps), where 10 of those steps move a fully opaque layer one canvas width off the canvas and back, the wasm module's `WebAssembly.Memory` `buffer.byteLength` stays within the §4.5 budget (≤ 1 GB).
- [ ] AC-12 (event): When an OpenRaster file written by another application has a layer image larger than the canvas or placed partly outside it, then Pincel opens the pixels outside the canvas as that layer's overhang (within AC-10's limit), so moving the layer brings them into view.

## Technical notes

- `pincel-image-core`'s `Layer` gets an optional overhang (an image buffer with its offset relative to the canvas, or an equivalent). Everything that reads `Layer::pixels` keeps seeing the canvas-sized buffer.
- Move Layer builds the layer's full content (overhang + canvas pixels), shifts it, and splits it again into canvas pixels and overhang; its undo record holds both. Storing only the bounding area of non-transparent overhang pixels keeps AC-11 within budget.
- OpenRaster: the layer PNG covers the union of canvas and overhang, `x` / `y` give its offset (negative values allowed by the format). The reader keeps what lies outside the canvas as overhang instead of clipping it.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Layer moved entirely off the canvas, then back in a later drag | Restored bit for bit (AC-01) |
| Paint on the canvas, then move the layer | The paint and the overhang move together (AC-03) |
| Erase everything on the canvas, then move back | The overhang comes back; the erased pixels stay erased |
| Move, then Canvas Size larger | Overhang dropped, in-canvas pixels as today (AC-05); undo of Canvas Size brings it back |
| Group moved half off the canvas and back | Every raster layer in the group is restored (AC-01) |
| Layer deleted and the deletion undone | The overhang comes back with the layer |
| .ora with overhang written by another app (layer image larger than the canvas) | Opens with that overhang (AC-12) |
| Move more than one canvas width past the edge | Pixels beyond the limit are dropped (AC-10) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Overhang is not shown anywhere in the UI except during a Move Layer drag (no "reveal all") | AC-02; the one command that would reveal it, a larger Canvas Size, discards it by the owner's choice | Low |
| Allocation of the overhang within AC-10's limit does not fail on the documents Image mode can open | AC-10 bounds it to 8 × the canvas area per layer, and Image mode already holds full-canvas layers and undo copies of that order (§4.5 memory budget); an out-of-memory abort is handled as for any other command today | Low |
| The overhang limit is one canvas size on each side (AC-10) | A layer one canvas size away is already entirely off the canvas, so dragging further gains nothing visible; the limit bounds the overhang to 8 × the canvas area, which keeps the §4.5 memory budget reachable | Medium: a user who drags a layer very far and back loses its far part |
| Flat exports and Copy clip at the canvas | Those formats and the clipboard have no layers to keep it in | Low |
| `image-editing` AC-21's "pixels pushed past the canvas edge are lost" stays true for Move Selected Pixels with a selection | Owner scoped the change to Move Layer | Low |

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

## Decisions

- 2026-11-18: Overhang per layer instead of layers with their own size (owner): tools, effects and the composite stay unchanged.
- 2026-11-18: Kept through undo / redo and OpenRaster; autosave not required (owner).
- 2026-11-18: Canvas-geometry commands and merges discard the overhang instead of carrying it (owner), to keep the scope small.
- 2026-11-18: `image-editing` AC-21 is narrowed by this spec for Move Layer and for Move Selected Pixels without a selection; its other cases are unchanged.
