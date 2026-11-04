---
status: implemented
created: 2026-10-28
size: L
mode: Image
design: [pincel.md §4.2, pincel.md §5.4, pincel.md §18.5, pincel.md §15 (2026-07-09 group compose), pincel.md §15 (2026-10-12 MoveLayerTo)]
---

# Layer groups in Image mode

## Context

Image mode has a flat layer stack only. Layer groups are a gap listed in §18.5: Pixel mode, Paint.NET's planned 6.0, Photopea, Pixlr, Krita and Affinity all have them. This spec gives Image mode groups in the document, the compositor and the layers panel: create, nest, collapse and drag layers into and out of groups, and group visibility, opacity, blend mode and lock that act on the whole subtree.

Reading and writing groups in OpenRaster files — which also fixes today's silent data loss when opening an `.ora` with nested stacks — is the separate spec `image-layer-groups-ora`, which depends on this one.

Already decided: Pixel mode has groups (`LayerKind::Group`, `parent`, contiguous subtrees, `MoveLayerTo` with `Above | Below | Into`, Decision Log 2026-10-12). Pixel mode does **not** fold group opacity / blend into a temp buffer (Decision Log 2026-07-09 deferred it), while §4.2 says groups fold their children into a temporary buffer. Image mode follows §4.2.

## Scope

### In scope

- Group layers in the Image-mode document: create, nest, move into / out of / between groups, delete and duplicate with their subtree
- Isolated group compositing: children composite into their own buffer, which is blended into the layers below with the group's opacity and blend mode
- Visibility and lock that inherit down the tree
- Refusing paint, fill and effects on a group layer
- Merge rules: Merge Down refuses groups and cross-parent merges; new Layer ▸ Merge Group; Merge Visible and Flatten Image fold through groups
- Layers panel: tree display, drag into / out of groups, Alt+←/→, collapse, Layer ▸ New Group and Layer ▸ Merge Group menu items (en + de strings)

### Out of scope

- Reading and writing groups in `.ora` files — because it is its own user-visible capability with its own tests; see the spec `image-layer-groups-ora`
- Pass-through groups (Photoshop's default) — because §4.2 and ORA's `isolation="isolate"` describe isolated groups, and pass-through is a backlog item
- Group masks and clipping masks — because masks do not exist in Image mode yet; backlog
- Folding group opacity / blend in Pixel mode — because that stays a separate decision (Decision Log 2026-07-09)

## Acceptance criteria

- [x] AC-01 (event): When the user chooses Layer ▸ New Group in Image mode, then an empty group appears in the layers panel and becomes the active layer, and one Undo removes it again.
- [x] AC-02 (event): When the user drags a layer row onto a group row, then the layer becomes a child of that group and its row is shown indented below the group's row; when the user drags a child row above or below a row outside the group, then it leaves the group. Each drag is one undo step.
- [x] AC-03 (event): When the user presses Alt+→ on a focused layer row, then the layer moves into the nearest group row listed above it at the same depth (as in Pixel mode); Alt+← on a child row moves it out of its group to the parent's level. Each is one undo step.
- [x] AC-04 (error): If a group is moved into itself or into one of its descendants, then the move is refused and the layer tree is unchanged.
- [x] AC-05 (event): When a group is moved, deleted or duplicated, then its whole subtree is moved, deleted or duplicated with it, in one undo step, and the duplicate is an independent copy (editing it leaves the original unchanged).
- [x] AC-06 (always): Every sequence of layer add / remove / move / duplicate / merge operations and their undos leaves a well-formed tree: every child's parent exists and is a group, no layer is its own ancestor, each group's descendants are listed directly beneath it, and undoing the whole sequence restores the original tree.
- [x] AC-07 (state): While a group is hidden, none of its descendants contribute to the composite, whatever their own visibility; their own visibility flags are unchanged and take effect again when the group is shown.
- [x] AC-08 (always): A group's opacity applies once to the composite of its children, not to each child: two overlapping fully opaque children in a group at 50 % opacity give 50 % coverage in the overlap (not 75 %).
- [x] AC-09 (always): A group composites in isolation: its children blend only with each other, starting from transparent, and the result blends into the layers below with the group's blend mode — a Multiply child inside a Normal group does not multiply with layers below the group, and a group set to Multiply multiplies its whole composite with the layers below.
- [x] AC-10 (always): A document without groups composes exactly as before this feature (byte-identical composite for the existing compose tests).
- [x] AC-11 (state): While a group is locked, painting, fill and effects on any of its descendants are refused with the locked-layer notice, and the descendants' rows show the inherited lock.
- [x] AC-12 (error): If a painting tool, fill, effect or Move selected pixels targets a group layer itself, then it is refused with the same status-bar notice as for a locked layer and the document is unchanged.
- [x] AC-13 (error): If Merge Down is invoked when the active layer or the layer below it is a group, or the two layers have different parents, then it is refused and the document is unchanged.
- [x] AC-14 (event): When the user chooses Layer ▸ Merge Group with a group active, then the group and its subtree are replaced by one raster layer at the group's position, the document composite is the same before and after, and one Undo restores the group and its subtree.
- [x] AC-15 (event): When Merge Visible or Flatten Image runs on a document with groups, then the resulting composite equals the composite before the merge, with each group's visibility, opacity and blend mode applied.
- [x] AC-16 (event): When the user changes visibility, lock, opacity or blend mode on a group row with the same controls a raster layer has, then the change is stored on the group, acts on its whole subtree, and is one undo step.
- [x] AC-17 (event): When the user collapses a group row, then its descendant rows are hidden in the layers panel and the composite is unchanged; expanding the group shows them again. The collapse is not an undo step and is not saved with the document.
- [x] AC-18 (state): While a group is active, Move (layer) and layer flips / rotations transform all its raster descendants in one undo step.
- [x] AC-19 (event): When the user pastes with a group active, then the new layer is inserted directly above the group, at the group's level.
- [x] AC-20 (error): If deleting a layer or group would leave no raster layer, then it is refused and the tree is unchanged.

## Technical notes

Hints from the legacy plan (2026-10-28); paths verified 2026-10-29, line numbers drift.

**Current Image-mode model.**

- `crates/pincel-image-core/src/document/layer.rs`: `LayerKind { Raster }` only (≈L10). `Layer` has no `parent` field.
- `crates/pincel-image-core/src/document/mod.rs`: `layers: Vec<Layer>`, flat, bottom first; `MAX_LAYERS = 999` (≈L16); `insert_layer` / `remove_layer` / `move_layer` (≈L157–215). `remove_layer` refuses removing the only layer (`LastLayer`).

**Compose.** `crates/pincel-image-core/src/render/mod.rs`: `compose()` (≈L35), `compose_region` (≈L57) and `compose_over` (≈L82) each loop over a flat list; `composite_layer` (≈L116) multiplies opacity into source alpha and blends in linear light into one accumulator. 12 blend modes in `color/mod.rs`.

**Commands that use flat indices.** `command/layers.rs` (`AddLayer`, `RemoveLayer`, `MoveLayer`, `DuplicateLayer`), `merge.rs` (`MergeDown`, `MergeVisible`, `FlattenImage`), `properties.rs` (rename, opacity, blend, visible, lock), also `paste.rs`, `transform.rs`, `resize.rs` and most of `tools/*`.

**wasm.** `crates/pincel-image-wasm/src/lib.rs`: `CommandSpec` (≈L468) is generated to `ui/src/modes/image/core/generated/CommandSpec.ts` by `cargo test -p pincel-image-wasm export_command_spec` (CI fails on drift). Planned additions: `AddGroup`, `MoveLayerTo { index, target, place }`, `MergeGroup`. `LayerInfo` / `DocumentInfo` (≈L1589 / ≈L1602) are hand-mirrored in `ui/src/modes/image/core/wasm.ts`; `LayerInfo` gains `kind`, `parent` / `depth` and `locked_by_parent`.

**Pixel-mode model to mirror** (`crates/pincel-core`):

- `LayerKind::Group` and `Layer.parent: Option<LayerId>` (`document/layer.rs`).
- Invariant: the flat Vec is kept, and a group's subtree is one contiguous run directly after the group header (`command/move_layer.rs`: `is_descendant`, `subtree_end`).
- `MoveLayerTo` with `LayerPlacement { Above, Below, Into }` (`command/move_layer_to.rs`).
- `RemoveLayer` removes the subtree; `MergeDown` rejects groups and cross-parent merges.
- `effectively_visible` walks the parents (`render/compose.rs` ≈L301).
- wasm `addGroup` (`crates/pincel-wasm/src/layer_ops.rs`) adds the group on top of the root stack, named `Group N`, and makes it active.

**Model design (from the plan).** Mirror pincel-core: `LayerKind::Group`, `parent: Option<Uuid>`, contiguous-subtree invariant in the flat Vec. A group layer does not allocate a canvas-sized pixel buffer: it keeps `pixels: ImageBuffer` as an empty 0×0 buffer (doc comment: "canvas-sized for raster layers, 0×0 for groups"); resize and transform skip groups. Canvas / image resize are document-wide and skip the 0×0 group buffers. The collapsed state is per-session view state in the shared layers panel, keyed by layer id (usable by Pixel mode too). `remove_layer`'s `LastLayer` rule becomes "at least one raster layer". Make `compose` / `compose_region` / `compose_over` recursive over subtrees with a transparent temp buffer per group. Suggested tests: `hidden_group_hides_children`, `group_opacity_applies_once_to_composite_not_per_child`, `group_blend_mode_blends_composite`, `merge_group_matches_composite`, plus a proptest over random add / move / remove sequences for the invariant (AC-06).

**Shared layers panel.** `ui/src/lib/components/layers/` already supports trees: `LayerRow.depth` / `isGroup` / `lockedByParent` (`layers.ts`), drop place `'into'`, `onMove`, `onAddGroup`, Alt+←/→ (`LayersPanel.svelte`). It has **no collapse control** yet. The Pixel mapping in `ui/src/modes/pixel/components/LayersPanel.svelte` (≈L78–128) is the example; the Image mapping in `ui/src/modes/image/components/panels/LayersPanel.svelte` (≈L42–79) sets none of these and passes only `onReorder`. Pixel menu key `menu.newGroup` exists; reuse Pixel i18n keys where the wording matches.

**e2e.** Follow `ui/e2e/layers.spec.ts` (`pixel: drag a layer into a group and back out`). Image mode: add a group, drag a layer into it, hide the group and the child disappears from the composite (element screenshot), undo.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Empty group (no children) | Contributes nothing to the composite; can be moved, hidden, deleted like any group |
| Groups nested three or more levels deep | Visibility, lock, opacity and blend apply at every level; opacities multiply level by level through the isolated composites |
| Hidden child inside a visible group | The child stays hidden; its siblings composite normally |
| Duplicating a group when the copy would exceed 999 layers | Refused as a whole, no partial copy; document unchanged |
| Adding a group at 999 layers | Refused with the existing layer-limit behavior |
| Moving a layer into a hidden or locked group | Allowed; the layer then shows as inherited-hidden / inherited-locked |
| Undo after Merge Group of a nested group | Restores the group, its subtree, and its position inside its parent |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ---- | ---- |
| Group layers count toward the 999-layer limit | The plan mirrors pincel-core's flat Vec, where a group header occupies one entry | Low — only matters at the limit |
| New Group places the group like Pixel mode does (top of the root stack, named `Group N`) | The plan says to mirror Pixel mode and reuse its i18n keys | Low — placement above the active layer would match Image mode's Add Layer better |

## Summary

> Completed: 2026-11-03

- Image mode gains layer groups: New Group, drag or Alt+arrow into and out of groups, a tree that never nests a layer into its own subtree, and delete / duplicate / move of a whole subtree
- Groups composite in isolation with their own opacity and blend mode; hidden or locked groups affect every descendant; pixel edits on a group are refused while move and flip transform its content
- Merge rules: Merge Down refuses groups and crossings, Merge Group flattens a group in place, Merge Visible and Flatten keep the composite; paste lands above the active group; a collapse toggle in the shared layers panel

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `image: Layer ▸ New Group adds an active empty group; Undo removes it (AC-01)` (`ui/e2e/image-layer-groups.spec.ts`); `add_group_adds_empty_group_on_top_active_and_undo_removes_it` (`crates/pincel-image-core/src/command/layers.rs`) | verified | ba53299 |
| AC-02 | `image: drag a layer into a group and back out, one undo step each (AC-02)` (`ui/e2e/image-layer-groups.spec.ts`); `move_layer_to_into_group_and_back_out_are_one_undo_step_each` (`command/layers.rs`) | verified | ba53299 |
| AC-03 | `image: Alt+→ moves a layer into the group above, Alt+← out again (AC-03)` (`ui/e2e/image-layer-groups.spec.ts`) | verified | 6aed618 |
| AC-04 | `move_layer_to_into_own_subtree_is_refused_and_tree_unchanged` (`command/layers.rs`); `move_subtree_into_own_descendant_is_refused_and_tree_unchanged` (`crates/pincel-image-core/src/document/tree.rs`) | verified | ba53299 |
| AC-05 | `move_layer_to_carries_the_group_subtree_in_one_undo_step`, `remove_layer_on_group_removes_subtree_and_one_undo_restores_it`, `duplicate_layer_on_group_copies_an_independent_subtree`, `duplicate_group_beyond_the_layer_limit_is_refused_as_a_whole` (`command/layers.rs`); `image: deleting a group deletes its subtree in one undo step (AC-05, AC-20)` (`ui/e2e/image-layer-groups.spec.ts`) | verified | 35f631c |
| AC-06 | `layer_tree_stays_well_formed_and_undo_restores_it` (`crates/pincel-image-core/tests/layer_tree_proptest.rs`) | verified | 6aed618 |
| AC-07 | `hidden_group_hides_children` (`crates/pincel-image-core/src/render/mod.rs`); `image: hiding a group hides its children from the composite (AC-07, AC-16)` (`ui/e2e/image-layer-groups.spec.ts`) | verified | 35f631c |
| AC-08 | `group_opacity_applies_once_to_composite_not_per_child`, `nested_group_opacities_multiply_level_by_level` (`render/mod.rs`) | verified | 9af6b69 |
| AC-09 | `group_children_blend_in_isolation_from_layers_below`, `group_blend_mode_blends_composite`, `compose_region_with_groups_matches_the_sub_rect_of_compose` (`render/mod.rs`) | verified | 9a7f4f0 |
| AC-10 | `fast_kernel_is_byte_identical_to_the_float_path` (`render/mod.rs`, compose vs. the flat per-layer fold); `crates/pincel-image-core/tests/compose_snapshots.rs` | verified | c43a845 |
| AC-11 | `locked_group_refuses_edits_on_its_descendants` (`crates/pincel-image-core/src/command/spec.rs`); `layer_infos_report_kind_depth_parent_and_inherited_lock` (`crates/pincel-image-wasm/src/lib.rs`); `image: a locked group refuses paint on its children and a group refuses paint (AC-11, AC-12)` (`ui/e2e/image-layer-groups.spec.ts`) | verified | 5dd2afe |
| AC-12 | `pixel_edits_on_a_group_are_refused` (`command/spec.rs`); `image: a locked group refuses paint on its children and a group refuses paint (AC-11, AC-12)` (`ui/e2e/image-layer-groups.spec.ts`) | verified | 22f5a52 |
| AC-13 | `merge_down_with_a_group_or_across_parents_is_refused` (`crates/pincel-image-core/src/command/merge.rs`) | verified | 3678aba |
| AC-14 | `merge_group_matches_composite`, `merge_group_of_a_nested_group_keeps_its_place_and_undo_restores_it` (`command/merge.rs`); `image: Layer ▸ Merge Group keeps the composite; Undo restores the group (AC-14)` (`ui/e2e/image-layer-groups.spec.ts`) | verified | 48b5d53 |
| AC-15 | `merge_visible_with_groups_preserves_composite`, `flatten_image_with_groups_preserves_composite` (`command/merge.rs`) | verified | 6aed618 |
| AC-16 | `image: hiding a group hides its children from the composite (AC-07, AC-16)` (visibility, opacity; `ui/e2e/image-layer-groups.spec.ts`); lock: AC-11's e2e; blend: `group_blend_mode_blends_composite` (`render/mod.rs`) | verified | 35f631c |
| AC-17 | `image: collapsing a group hides its rows only, without an undo step (AC-17)` (`ui/e2e/image-layer-groups.spec.ts`) | verified | b5e6482 |
| AC-18 | `move_and_flip_on_a_group_transform_all_raster_descendants_in_one_undo_step`, `move_on_a_group_with_a_locked_descendant_is_refused` (`command/spec.rs`) | verified | 6aed618 |
| AC-19 | `paste_with_a_group_active_inserts_above_the_group_at_its_level` (`crates/pincel-image-core/src/command/paste.rs`) | verified | 6aed618 |
| AC-20 | `remove_layer_leaving_no_raster_layer_is_refused` (`command/layers.rs`); `remove_subtree_refuses_removing_the_last_raster_layer` (`document/tree.rs`); `image: deleting a group deletes its subtree in one undo step (AC-05, AC-20)` (`ui/e2e/image-layer-groups.spec.ts`) | verified | 6aed618 |

## Decisions

- Model mirrors pincel-core exactly (`LayerKind::Group`, `parent`, contiguous subtree in the flat Vec, `MoveLayerTo` with `Above | Below | Into`) — because the shared layers panel and the Pixel commands already work with that shape, and the invariant is proven by Pixel's round-trip tests.
- Groups composite isolated (children into a transparent buffer, then blended with the group's opacity and blend mode) — because §4.2 describes it and ORA's `isolation="isolate"` matches it; pass-through is out of scope.
- Paint, fill and effects on a group are refused with the locked-layer notice; lock and hide inherit down the tree — because a group has no pixels of its own, and inheriting matches Pixel mode's `lockedByParent` / `effectively_visible`.
- Merge Down refuses groups and cross-parent merges; Merge Group rasterizes a group to its composite; Merge Visible and Flatten Image fold through groups — because that matches Pixel mode's rules and keeps every merge composite-preserving.
- The ORA part is split into the spec `image-layer-groups-ora` — because the legacy plan was XL (four M sessions) and the ORA round-trip is a separate user-visible capability.
- 2026-10-29: A Decision Log entry records Image-mode groups mirroring pincel-core (`LayerKind::Group`, `parent`, contiguous subtree in the flat Vec, `MoveLayerTo`), isolated group compositing (§4.2) and the 0×0 group buffer, with Pixel mode's 2026-07-09 rule unaffected — the plan requires it before code, and it records the deliberate cross-mode difference (owner accepted the recommendation).
- 2026-10-29: A group keeps `pixels: ImageBuffer` as an empty 0×0 buffer instead of `Option<ImageBuffer>`; resize and transform skip groups — `.pixels` has about 250 uses in `pincel-image-core`, and an `Option` would touch all of them for no behavioural gain (owner accepted the recommendation).
- 2026-10-29: A group's collapsed state is per-session view state in the shared layers panel, keyed by layer id, not saved and not undoable (AC-17) — collapsing changes no pixels, ORA has no standard attribute for it, and undo entries for view toggles would be noise (owner accepted the recommendation).
- 2026-10-29: With a group active, Move (layer) and layer flips / rotations transform every raster descendant in one undo step (AC-18); Move selected pixels is refused like painting (AC-12); canvas / image resize are document-wide and skip group buffers; Paste inserts above the group as its sibling (AC-19) — moving or flipping a group's content is the expected group operation (Krita, Photoshop), while pixel edits on a group have no target (owner accepted the recommendation).
- 2026-10-29: A document must keep at least one raster layer; deleting a layer or group that would leave none is refused (AC-20) — every tool, fill and paste needs a raster target, and the rule generalizes today's `LastLayer` check (owner accepted the recommendation).
- 2026-10-29: Released together with `image-layer-groups-ora`; the release PR is not merged between the two, and there is no interim `.ora` save guard — autosave also writes ORA, so a save-only guard wouldn't cover recovery, and a guard would be throw-away code (owner accepted the recommendation).
- 2026-11-04 (spec-build): Add Layer and Paste insert directly above the active layer's whole subtree and share its parent — so with a group active the new layer lands above the group at its level, and with a child active it stays in that child's group — because AC-19 asks it for Paste and Add Layer should place layers the same way.
- 2026-11-04 (spec-build): Merge Visible merges every visible root-level layer or group (with its whole subtree) into one layer at the lowest of them and keeps hidden root-level layers and groups; hidden layers inside a merged group are merged away — because they contribute nothing to the composite and pulling them out of their group would change what it composites.
- 2026-11-04 (spec-build): Merge Group's raster layer keeps the group's id, name, parent, opacity, blend mode, visibility and lock — because its pixels are the isolated composite of the children, so those properties make the document composite identical (AC-14).
- 2026-11-04 (spec-build): Move (layer) and layer flips / rotations on a group are refused with the locked-layer notice while the group or any layer inside it is locked — because AC-11 refuses edits on locked descendants and a partial move would split the group's content.
- 2026-11-04 (spec-build): The flat `MoveLayer { from, to }` stays and refuses (`LayerTreeBroken`) any reorder that would split a group's run or leave a child outside its group; the Image panel's arrow keys move a row past its neighbouring sibling with `move_layer_to` — because the old flat index swap cannot keep the tree well formed (AC-06).
- 2026-11-04 (spec-build): `LayerInfo.parent` is absent (not `null`) at the root — because serde-wasm-bindgen turns `None` into `undefined`; the UI reads it as `parent ?? null`.
- 2026-11-04 (spec-build): An effect on a group opens its dialog and is refused on OK with the existing "<effect> failed: layer is locked" notice, as for a locked layer — because `effects-adjustments` AC-12 pins that flow for locked layers and AC-12 here asks for the same notice.
- 2026-11-04 (spec-build): The collapse state lives in the shared layers panel component (Pixel mode's panel gets the toggle too) and a group's thumbnail is the composite of its subtree, hidden or not, as in Pixel mode — because the panel is shared and the plan keyed collapse by row id there.
- 2026-11-04 (spec-build): The AC-06 proptest was written after the commands and passed at once; a mutation check (dropping Merge Down's group refusal) made it fail with a shrunk sequence, so it does detect broken trees. The Image-mode e2e specs were written after the UI; their first run failed on the root `parent` being `undefined`.
