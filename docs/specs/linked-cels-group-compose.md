---
status: todo
created: 2026-10-29
size: L
mode: Pixel
design: [pincel.md §3.2, pincel.md §4.1, pincel.md §4.2, pincel.md §15]
---

# Compose linked cels and fold group layers

## Context

Two gaps in `compose()` (spec §4.1) that real Aseprite files hit all the time:

1. **Linked cels** are rejected with `RenderError::LinkedCelUnsupported`. A linked cel reuses another frame's cel on the same layer; every hand-drawn animation that holds a pose across frames uses them. Pincel already preserves links on read and write (§3.2), so such a file opens and saves — but no frame containing a link can be shown, thumbnailed, onion-skinned or exported.
2. **Group layers** are skipped: a group's visibility already hides its whole subtree (Decision Log 2026-07-09), but its **opacity and blend mode are ignored**, so a group set to 50 % or Multiply renders as if it were 100 % Normal. The design says how groups should render: "Group layers fold their children into a temporary buffer, then blend that buffer into the parent" (§4.2). The 2026-07-09 Decision Log entry deferred exactly this fold to Phase 2.

How the `.aseprite` format encodes groups (format reference <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>, Layer Chunk `0x2004`, NOTE.1): each layer carries a "child level" relative to the previously read layer; a layer at level *d* + 1 directly after a group at level *d* is that group's child, and nesting goes arbitrarily deep. Pincel's reader turns this into `Layer::parent`, and `Sprite.layers` stays flat and bottom-first with each group's subtree as one contiguous run directly after the group header (§3.2, Decision Log 2026-10-12). Group layers carry their own opacity (valid when header flag bit 0 is set, which the writer sets) and blend mode. Linked cels are Cel Type 1 with a "frame position to link with".

Already decided: blend modes are computed in u8 sRGB, and Aseprite pixel-exact parity is out of scope (Decision Log 2026-07-09, 2026-10-27).

## Scope

### In scope

- `compose()` renders linked cels by following the link to the target frame's cel on the same layer
- `compose()` folds group opacity and blend mode into the rendering of the group's subtree, at any nesting depth
- Both apply to the main frame, onion-skin neighbours, every `LayerFilter`, zoom and viewport / dirty rect
- Everything built on `compose()` — layer thumbnails, PNG frame and atlas export — renders such documents without further change

### Out of scope

- Creating links on write (link-on-write) — because §3.2 / §13.6 put it in Phase 2; the writer keeps writing links it read
- Editing linked cels (painting into a link, unlinking, linking) — because those are commands with their own undo semantics, not rendering
- Cel z-index — because `compose()` doesn't apply it for any layer kind yet (§7.1); it is a separate change
- Group layers in Image mode — because they are a separate feature (`image-layer-groups.md`) in a different core
- Bit-exact parity with Aseprite's renderer — because no Aseprite-generated reference fixture exists to cross-validate against; tests assert hand-derived values from our own u8 formulas
- Decoration overlays (`RenderError::OverlaysUnsupported`) — because they are independent of layer structure and come separately

## Acceptance criteria

### Linked cels

- [ ] AC-01 (event): When a frame contains a cel linked to another frame's image or tilemap cel on the same layer, then `compose()` renders that frame exactly as if the link were replaced by a copy of the target cel — the target's pixel or tile data, at the target's position, with the target's cel opacity — under the linking layer's opacity and blend mode.
- [ ] AC-02 (event): When onion skin includes a neighbour frame that contains linked cels, then the neighbour renders through the link like any other cel.
- [ ] AC-03 (event): When a frame containing linked cels is exported as PNG or as part of an atlas, then the export succeeds and its pixels equal `compose()`'s output for that frame.
- [ ] AC-04 (event): When a sprite with linked cels is written to `.aseprite` and read back, then every frame composes to the same pixels as before writing.
- [ ] AC-05 (error): If a linked cel's target frame does not exist in the sprite, or the target frame has no cel on that layer, then `compose()` fails with a structured `RenderError` naming the layer, the linking frame and the target frame.
- [ ] AC-06 (error): If a linked cel's target is itself a linked cel (including a cel linked to its own frame), then `compose()` fails with a structured `RenderError` naming the layer, the linking frame and the target frame, and does not loop.
- [ ] AC-07 (error): If the link target's data kind doesn't match the layer kind (tile data on an image layer or image data on a tilemap layer), then `compose()` fails with `RenderError::CelTypeMismatch`, as for an unlinked cel.

### Group layers

- [ ] AC-08 (always): A layer inside a hidden group, at any depth, does not render under the visible-layers filter.
- [ ] AC-09 (event): When a group's opacity is below 255, then its children are first composited together and the result is blended onto the layers below the group at the group's opacity — e.g. an opaque red child over an opaque blue child in a group at opacity 128, over an opaque white background, gives a pink overlap pixel with no blue showing through.
- [ ] AC-10 (event): When a group's blend mode is not Normal, then the composite of its children is blended onto the layers below the group with the group's blend mode.
- [ ] AC-11 (event): When a group has opacity below 255 or a blend mode other than Normal, then each child blends with its blend mode against the group's own content below it (its lower siblings), not against the layers below the group — e.g. a Multiply red child as the only child of a group at opacity 254 over a mid-grey background renders the red at the group's opacity, not the multiplied dark red.
- [ ] AC-12 (always): A group at opacity 255 with blend mode Normal whose descendants all use blend mode Normal renders, per channel within 1, the same pixels as the same layers without the group.
- [ ] AC-13 (event): When groups are nested, then each level applies its own opacity and blend mode to the composite of its subtree — e.g. an opaque child inside three nested groups at opacity 128 each renders with alpha 32 ± 2, and nesting at least eight levels deep renders without error.
- [ ] AC-14 (event): When a group's opacity is 0, then its subtree contributes nothing to the output.
- [ ] AC-15 (option): Where the layer filter is "all layers", groups apply their opacity and blend mode while visibility is ignored.
- [ ] AC-16 (option): Where the layer filter selects explicit layers, a group's opacity and blend mode apply only if that group is itself selected; a selected child whose group is not selected renders as if it had no parent group.
- [ ] AC-17 (always): Composing a sub-viewport or a dirty hint returns the same pixels as the matching region of a full-frame compose, for documents with groups and linked cels.
- [ ] AC-18 (event): When a sprite with nested groups that use non-default opacity and blend modes is written to `.aseprite` and read back, then every frame composes to the same pixels as before writing.
- [ ] AC-19 (error): If the layer hierarchy is inconsistent — a parent cycle, or a layer whose parent id names no layer — then `compose()` returns without panicking or hanging; a layer with a parent id that names no layer renders as a top-level layer.

## Technical notes

Hints from a 2026-10-29 code check; line numbers drift — grep for the names.

- **Linked rejection**: `crates/pincel-core/src/render/compose.rs` ≈L227 (`(_, CelData::Linked(_))` → `LinkedCelUnsupported`); test `rejects_linked_cel` (≈L678). The variant's doc in `render/error.rs` still says "the loader (M4) is the layer that resolves linkage" — outdated, the reader keeps `CelData::Linked`. Once links render the variant is dead; removing it is a public API change of `pincel-core`.
- **Group skip**: `compose.rs` ≈L163 skips `LayerKind::Group` with a comment that opacity / blend are not folded; `effectively_visible` (≈L301) walks parents for visibility, capped at `sprite.layers.len()` hops; a dangling parent ends the walk and the layer renders. Tests `group_layer_is_skipped_and_children_render`, `hidden_group_hides_child_layers`, `hidden_grandparent_hides_nested_child`, `layer_filter_all_ignores_hidden_group`.
- **Flat layer order**: `Sprite.layers` bottom-first; a group's subtree is a contiguous run directly after the group (§3.2). The writer checks it (`validate_layer_order`, `codec/aseprite_write.rs` ≈L292); the reader rebuilds `parent` from `child_level` with a `parent_stack` (`codec/aseprite_read.rs` ≈L79–96). In-memory sprites built without the writer may violate contiguity.
- **Link semantics already in the code**: `RemoveFrame` (`command/remove_frame.rs`) copies the target's `position`, `opacity` and `data` into a cel whose link target is removed — the same "a link shares the target's position and opacity" rule as AC-01. The writer (`aseprite_write.rs` ≈L584) refuses links whose target is missing (`LinkedCelTargetMissing`) or is not an image cel (`LinkedCelTargetNotImage`), because `aseprite-loader` rejects such files.
- **Callers**: onion skin `render/onion.rs` reuses `composite_visible_layers`; thumbnails use `LayerFilter::Only` with the layer plus its subtree (Decision Log 2026-10-12); PNG export `codec/png.rs` documents linked cels as a limitation to drop.
- **Scratch buffers**: §4.1 asks `compose()` not to allocate per pixel; a group fold needs a viewport-sized temporary per nesting level.
- **Tests**: inline in `compose.rs`, helpers in `render/test_support.rs`, integration in `crates/pincel-core/tests/render_compose.rs`, codec round-trips in `tests/aseprite_codec.rs` (written through `write_aseprite`, no binary fixtures). AC-12 suits a `proptest` (CLAUDE.md §7.3).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Linked cel's own position / opacity differ from its target's | The target's values are used (AC-01) |
| Linked cel on a hidden layer | Not rendered under the visible filter, like any cel of a hidden layer |
| Link target lies in a frame outside the onion-skin range | Still resolved — links reach any frame of the sprite |
| Empty group, or a group whose children have no cel in this frame | Contributes nothing |
| A cel stored on a group layer (corrupt document) | Ignored, as today |
| Group at opacity 255 / Normal containing a Multiply child | See Open questions (isolated vs pass-through) |
| Hidden child inside a visible group at opacity 128 | Child contributes nothing; siblings render at the group's opacity |
| Group subtree not contiguous in an in-memory sprite | `compose()` returns without panicking (AC-19); the writer refuses to save it anyway |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| A linked cel shares its target's position and cel opacity, not only its pixels | In Aseprite's document model links share one cel-data object (image, bounds, opacity); `RemoveFrame` already follows that rule | Low: Aseprite writes identical header values for both cels anyway |
| A group composites its children in isolation (children blend against the group's content, not the backdrop) whenever the group is not 100 % Normal | §4.2 prescribes a temporary buffer per group | Medium: Aseprite's own group rendering is not cross-validated; a reference fixture would settle it |

## Open questions

- Group opacity / blend-mode folding is deferred to Phase 2 by the 2026-07-09 Decision Log entry ("compose() skips group layers and propagates group visibility"). Doing it now needs a new Decision Log entry that supersedes that part (as 2026-10-27 superseded the HSL fallback), otherwise it mixes Phase 1 and Phase 2 (CLAUDE.md §9). Write that entry?
- Does a group at opacity 255 with blend mode Normal also composite its children in isolation (§4.2 read literally — a Multiply child then no longer multiplies with the layers below the group, which changes how existing files look), or does it pass its children through to the backdrop as today?

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

## Decisions

- One spec of size L for both parts, as requested; linked cels (AC-01 … AC-07) are independent of the group open questions, so splitting off `linked-cels-compose.md` would let them start before those are answered.
- Missing link targets and chained links are errors, not silently empty cels — consistent with how `compose()` treats other corrupt references (`TilesetNotFound`, `TileIdOutOfRange`), and the writer refuses both cases too, so they only arise from a bug that should surface.
- Under "all layers" groups still fold — "all" overrides visibility only, so a thumbnail or export of everything still looks like the document.
- Under an explicit layer selection a group folds only when it is selected itself — the selection is an explicit override, as it already is for visibility; the layer thumbnail selects the layer plus its subtree, so a group's thumbnail shows its opacity and a child's thumbnail shows the child alone.
- AC-12 allows a difference of 1 per channel because isolated u8 compositing may round differently from compositing straight onto the backdrop.
