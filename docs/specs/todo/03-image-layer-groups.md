---
id: todo-image-layer-groups
title: Layer groups in Image mode
status: todo
size: XL (four M sessions; write the Decision Log entry first)
mode: Image
depends_on: []
created: 2026-10-28
---

# Layer groups in Image mode

## Goal

Image mode gets layer groups:
- create, nest, collapse and drag layers into and out of groups;
- group visibility, opacity, blend mode and lock that act on the whole subtree;
- groups that survive OpenRaster save and load.

## Why

The gap is listed in §18.5. Pixel mode, Paint.NET's planned 6.0, Photopea, Pixlr, Krita and Affinity all have groups.

There is also a **silent data-loss bug** today: opening an `.ora` with nested `<stack>`s flattens them and **drops the group's own opacity, visibility and blend mode**. A hidden or 50 % group's children then show fully visible. This task fixes that.

## Current state (read these first)

**Model.**
- `crates/pincel-image-core/src/document/layer.rs`: `LayerKind { Raster }` only (≈L8). `Layer` has no `parent` field.
- `document/mod.rs`:
  - `layers: Vec<Layer>`, flat, bottom first (≈L70)
  - `MAX_LAYERS = 999`
  - `insert_layer` / `remove_layer` / `move_layer` (≈L157–201)

**Compose.** `crates/pincel-image-core/src/render/mod.rs`
- `compose()` (≈L35), `compose_region` (≈L57) and `compose_over` (≈L82) each loop over a flat list.
- `composite_layer` (≈L116) multiplies opacity into source alpha and blends in linear light into one accumulator.
- 12 blend modes (`color/mod.rs` ≈L91).

**Commands that use flat indices.**
- `command/layers.rs`: `AddLayer`, `RemoveLayer`, `MoveLayer`, `DuplicateLayer`
- `merge.rs`: `MergeDown`, `MergeVisible`, `FlattenImage`
- `properties.rs`: rename, opacity, blend, visible, lock
- also `paste.rs`, `transform.rs`, `resize.rs` and most of `tools/*`

**wasm.** `crates/pincel-image-wasm/src/lib.rs`
- `CommandSpec` (≈L468, generated to `ui/src/modes/image/core/generated/CommandSpec.ts` by `cargo test -p pincel-image-wasm export_command_spec`; CI fails on drift).
- `LayerInfo` / `DocumentInfo` (≈L1588) are hand-mirrored in `ui/src/modes/image/core/wasm.ts` (≈L31–53).

**ORA.** `crates/pincel-image-core/src/codec/ora.rs`
- Writes one flat `<stack>` (≈L105).
- Reads every `<layer>` start tag in document order with the `tags()` scanner (≈L248). Nested stack attributes are discarded.
- Test: `from_ora_bytes_places_offset_layers_and_flattens_nested_stacks` (≈L384).
- Per the Decision Log 2026-10-11 there is **no XML crate**. Keep the attribute scanner, and track `<stack` / `</stack>` depth.

**The Pixel-mode model to mirror** (`crates/pincel-core`)
- `LayerKind::Group` and `Layer.parent: Option<LayerId>` (`document/layer.rs` ≈L99–139).
- Invariant: the flat Vec is kept, and a group's subtree is one contiguous run directly after the group header (`command/move_layer.rs` ≈L3–80: `is_descendant`, `subtree_end`).
- `MoveLayerTo` with `LayerPlacement { Above, Below, Into }` (`command/move_layer_to.rs`).
- `RemoveLayer` removes the subtree. `MergeDown` rejects groups and cross-parent merges.
- `effectively_visible` walks the parents (`render/compose.rs` ≈L301).
- Pixel mode **does not fold** group opacity or blend into its children. That folding was deferred in Decision Log 2026-07-09, while §4.2 says groups should fold.

**Shared layers panel.** `ui/src/lib/components/layers/`
- It already supports trees: `LayerRow.depth` / `isGroup` / `lockedByParent`, `DropPlace` `'into'`, `onMove`, `onAddGroup`, and Alt+←/→.
- The Pixel mapping in `ui/src/modes/pixel/components/LayersPanel.svelte` (≈L78–128) is the example.
- The Image mapping in `ui/src/modes/image/components/panels/LayersPanel.svelte` (≈L42–79) sets none of these and passes only `onReorder`.

## Design decisions (Decision Log entry before any code)

1. **Model.** Mirror pincel-core exactly:
   - `LayerKind::Group`, `parent: Option<Uuid>`, and the contiguous-subtree invariant in the flat Vec.
   - A group layer's `pixels` is an empty buffer. Don't allocate canvas-sized memory for it. That needs a small `ImageBuffer` change, or `pixels: Option<ImageBuffer>` behind accessors. Choose one and record it.
2. **Compositing.** Use **isolated groups**, as §4.2 intends and as ORA's `isolation="isolate"` describes:
   - Children are composited into a transparent temp buffer.
   - That buffer is then blended into the parent with the group's opacity and blend mode.
   - Pass-through groups (Photoshop's default) are out of scope.
   - Record that Image mode folds while Pixel mode still doesn't, and that the difference is deliberate (Decision Log 2026-07-09).
3. **Editing.**
   - Painting tools, effects and fill on a group are refused with a status-bar notice, the same as a locked layer.
   - Lock and hide inherit down the tree.
4. **Merge.**
   - `MergeDown` rejects groups and cross-parent merges, as in Pixel mode.
   - New **Merge Group**: rasterize a group into one layer with its composite.
   - `MergeVisible` and `FlattenImage` fold through groups.
5. **ORA.**
   - Write nested `<stack name opacity visibility composite-op isolation="isolate">`.
   - On read, honour the stack attributes and build the tree. This fixes the data-loss bug.

## Tasks (one M session each)

1. **`docs(spec)` + `feat(pincel-image-core)`: model and invariant.**
   - `LayerKind::Group`, `parent`, helpers (`subtree_end`, `is_descendant`, `depth`).
   - `AddGroup`, and `MoveLayerTo` with placement.
   - `RemoveLayer` and `DuplicateLayer` work on subtrees.
   - Unit tests mirror the pincel-core ones. Add proptest: random move / add / remove sequences keep the contiguous-subtree invariant.
2. **`feat(pincel-image-core)`: compose with isolated groups.**
   - Make `compose` / `compose_region` / `compose_over` recursive over subtrees.
   - Visibility inheritance.
   - Refuse to paint on groups (`tools/*`, `layer_effect_image`).
   - Merge Group.
   - Tests:
     - `hidden_group_hides_children`
     - `group_opacity_applies_once_to_composite_not_per_child`
     - `group_blend_mode_blends_composite`
     - `merge_group_matches_composite`
3. **`feat(pincel-image-core)`: ORA nested stacks, read and write.**
   - Update the flatten test into `from_ora_bytes_keeps_nested_stack_attributes`, plus a round-trip test (write → read → equal tree).
   - Old flat files still open.
4. **`feat(pincel-image-wasm)` + `feat(ui)`.**
   - `CommandSpec`: `AddGroup`, `MoveLayerTo { index, target, place }`, `MergeGroup`. Regenerate the TS mirror.
   - `LayerInfo` gains `kind`, `parent` / `depth` and `locked_by_parent`; update the hand mirror in `wasm.ts`.
   - Image `LayersPanel.svelte` maps depth / isGroup / lockedByParent and passes `onMove` and `onAddGroup`.
   - Menu: Layer ▸ New Group and Layer ▸ Merge Group. Reuse the Pixel i18n keys where the wording matches.
   - e2e, following `ui/e2e/layers.spec.ts`:
     - image mode: add a group, drag a layer into it, hide the group and the child disappears from the composite, undo.
     - save as `.ora`, reopen, the group is still there (pattern from `ui/e2e/ora.spec.ts`).

## Out of scope

- Pass-through groups, group masks and clipping masks (backlog).
- Folding groups in Pixel mode: that stays a separate decision.
