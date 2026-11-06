---
status: todo
created: 2026-11-05
size: L
mode: both
design: [mcp.md, pincel.md §15 (2026-10-29 agent tools in pincel-agent), pincel.md §15 (2026-11-05 MCP tool parity)]
depends_on: [mcp-headless-pixel, mcp-headless-image]
---

# MCP parity: document structure

## Context

`mcp-headless-pixel` and `mcp-headless-image` give agents a headless `pincel-mcp` server, but its tools cover only part of what the editor can do. The owner wants every document-changing capability of the editor reachable through MCP. A gap inventory (2026-11-05) found the missing capabilities in five areas, and each area gets its own spec: structure (this one), selection and transform, drawing, Pixel assets (slices, tilesets, grid), and import / export.

This spec closes the gaps in the document structure. It adds the missing layer operations (duplicate, move, merge, flatten, Image-mode groups), tag editing, single palette entries, palette files, and the color mode and selection in `doc.info`. Every capability already exists in `pincel-core` / `pincel-image-core` (commands and the palette codecs) and is used by the editor. The tools expose these capabilities; they do not add new editing behavior.

**"The editor" in this spec** means the result the editor's UI produces for the same action in a document of that mode, which is the result of the core command (or codec call) the UI issues for it. A test compares the tool's result with that command applied directly to an equal document, and checks the concrete values this spec states.

**Conventions** inherited from `mcp-headless-pixel`: `layer.list` lists layers bottom to top, so "above" means later in that list and a group's "top child" is its last child in it. Frame and palette indexes are 0-based. Layers are addressed by the ids `layer.list` reports. Color arguments follow `mcp-headless-pixel` AC-21.

This spec supersedes the entry "`layer.*` with `parent`" in `mcp-headless-image` AC-05: groups and `parent` now work on Image documents (AC-12).

Decision Log 2026-11-05 "MCP tool parity" fixes the naming rule for all five specs. A capability gets one tool name. The tool works on every mode whose editor offers the capability and fails with the mode error of `mcp-headless-image` AC-05 elsewhere. No aliases are kept.

## Scope

### In scope

- New layer tools: `layer.duplicate`, `layer.move`, `layer.merge_down`, `layer.merge_visible`, `layer.merge_group`, `layer.flatten`
- `layer.add`: places the new layer as the document's editor does, and takes an optional `target` + `place`
- Layer groups in Image documents through `layer.add` (`kind: "group"`, `parent`), `layer.set`, `layer.remove`, `layer.move` and `layer.list`
- Tags: `color` on `tag.add` and in `tag.list`; new `tag.set` (name, range, direction)
- Palette: entry names in `palette.get`; new `palette.set_entry`, `palette.add_entry`, `palette.remove_entry`, `palette.move_entry`, `palette.import`, `palette.export`
- `doc.info` / `doc.list`: color mode and selection bounds for both modes
- `pincel://guide` documents the new tools

### Out of scope

- Tilemap layers, tilesets, slices, the document grid, and slice / tileset counts in `doc.info` — because they need the tileset and slice tools, which come in `mcp-parity-pixel-assets`
- Selection tools and the removal of `selection.set_rect` / `selection.clear` — because `mcp-parity-selection-transform` replaces them with `select.*`
- Capabilities that no editor offers: changing a tag's color after it was added, Merge Visible or Merge Group in Pixel mode, tags and palettes in Image mode, converting between RGBA and indexed — because parity means reaching what the editor can do; new editing features need their own spec and core work
- View state (zoom, tabs, onion skin, thumbnails, tool options) — because it does not change the document, and `doc.preview` already covers rendering
- Live control of the open desktop document, and concurrent edits by an agent and the user — because `mcp-live-desktop` covers both; it runs the same `pincel-agent` tools (Decision Log 2026-10-29) and owns their undo and refresh in the editor

## Acceptance criteria

- [ ] AC-01 (always): `tools/list` contains `layer.duplicate`, `layer.move`, `layer.merge_down`, `layer.merge_visible`, `layer.merge_group`, `layer.flatten`, `tag.set`, `palette.set_entry`, `palette.add_entry`, `palette.remove_entry`, `palette.move_entry`, `palette.import` and `palette.export`, each meeting `mcp-headless-pixel` AC-02.
- [ ] AC-27 (error): If a tool of this spec names a `layer`, `target`, tag or palette index that does not exist, then the call fails with `isError: true` naming the argument and its value, and the document is unchanged.
- [ ] AC-02 (error): If a tool of this spec is called on a document whose mode's editor lacks the capability (`layer.merge_visible` or `layer.merge_group` on a Pixel document; `tag.*` or `palette.*` on an Image document), then the call fails with `isError: true`, the message names the document's mode, and the document is unchanged.
- [ ] AC-03 (event): When `layer.add` is called without `target`, then the new layer sits where the editor of the document's mode puts a new layer: on top of the root stack (or as the top child of `parent`) in a Pixel document, and in an Image document directly above the active layer (above a group's whole subtree when a group is active), in the same parent group.
- [ ] AC-04 (option): Where `layer.add` gets `target` (a layer id) and `place` (`above`, `below` or `into`), the new layer sits directly above or below that layer as its sibling, or as the top child of that group for `into`.
- [ ] AC-05 (event): When the client calls `layer.duplicate` with a layer, then a copy with the same pixels in every frame and the same visibility, opacity and blend mode sits directly above the source with the source's lock state, named `"<source name> copy"`. In an Image document a group is copied with its whole subtree. The copy becomes the active layer, and the result carries its id.
- [ ] AC-28 (error): If `layer.duplicate` names a group in a Pixel document, then the call fails with `isError: true` saying that Pixel groups cannot be duplicated, and the document is unchanged.
- [ ] AC-06 (event): When the client calls `layer.move` with `layer`, `target` and `place` (`above`, `below` or `into`), then `layer.list` reports the layer, with its whole subtree for a group, at that position, and the composed image of every frame equals the editor's after the same move.
- [ ] AC-07 (error): If `layer.move` or `layer.add` uses `into` on a target that is not a group, names the moved layer itself as `target`, would move a group into its own subtree, gets `place` without `target` or `target` without `place`, or (`layer.add`) gets both `parent` and `target`, then the call fails with `isError: true` naming the argument, and the document is unchanged.
- [ ] AC-08 (event): When the client calls `layer.merge_down` with a layer, then the document's layers and pixels equal the editor's after Merge Down of that layer.
- [ ] AC-09 (error): If `layer.merge_down` names a layer the editor refuses to merge down (the bottom layer, a group, or a layer with no image layer directly below it in the same group; a tilemap layer is not an image layer), then the call fails with `isError: true` stating the reason, and the document is unchanged.
- [ ] AC-10 (event): When the client calls `layer.merge_visible` or `layer.merge_group` (with a group) on an Image document, then its layers and pixels equal the editor's after Merge Visible, or after merging that group; `layer.merge_group` on a layer that is not a group fails with `isError: true` naming it, and the document is unchanged.
- [ ] AC-29 (error): If a merge or flatten tool would change the pixels of a locked layer, or remove a locked layer (or a layer inside a locked group), then the call fails with `isError: true` naming the layer, and the document is unchanged; `layer.duplicate` and `layer.move` work on locked layers.
- [ ] AC-11 (event): When the client calls `layer.flatten`, then the document's layers and pixels equal the editor's after Flatten in that mode.
- [ ] AC-12 (event): When `layer.add` with `kind: "group"` (named `Group N` like the editor's, or `name`), `layer.set`, `layer.remove` or `layer.move` succeed on an Image document, then `layer.list` reports each layer's kind (`image` or `group`) and the id of its parent group (`null` at the root), matching the document's layer tree. Removing a group removes its subtree.
- [ ] AC-13 (error): If a pixel-changing tool (`draw.*`, `adjust.apply`, `effect.apply`) targets a group layer in an Image document, then the call fails with `isError: true` saying that the layer is a group, and the document is unchanged.
- [ ] AC-14 (event): When `tag.add` gets a `color` (`#RRGGBB`), then `tag.list` reports that color for the tag. Without `color`, the tag gets the timeline's next tag color: entry n mod 6 of `#5B8DEF`, `#E0A83C`, `#58B368`, `#D0607A`, `#9A6AD8`, `#3CB7C4`, n being the number of tags before adding. `tag.list` reports every tag's color.
- [ ] AC-15 (event): When the client calls `tag.set` with a tag and at least one of `name`, `from` + `to`, or `direction`, then `tag.list` reports the tag with exactly these fields changed.
- [ ] AC-16 (error): If `tag.set` names no existing tag, gets none of the editable fields, an empty name, `from` greater than `to`, or a frame beyond the last frame, then the call fails with `isError: true` naming the argument, and the document is unchanged.
- [ ] AC-17 (event): When the client calls `palette.get`, then each entry carries its name, or an empty string for an unnamed entry.
- [ ] AC-18 (event): When `palette.set_entry` (`index`, `color`), `palette.add_entry` (`color`, returns the new index), `palette.remove_entry` (`index`) or `palette.move_entry` (`from`, `to`: the entry ends up at index `to`, and the entries in between shift by one) succeed, then `palette.get` returns the palette with exactly that change.
- [ ] AC-19 (error): If `palette.add_entry` is called on a palette of 256 entries, or `palette.remove_entry` / `palette.move_entry` is called on a document whose color mode is not `rgba` (indexed and grayscale pixels reference entries by position), then the call fails with `isError: true` stating the reason, and the palette is unchanged.
- [ ] AC-20 (event): When the client calls `palette.import` with a workspace-relative GIMP `.gpl`, Lospec `.hex`, JASC `.pal` file or a PNG palette image (every distinct color of the image, in reading order), then the palette equals the colors the editor imports from that file, whatever the file's extension, and the result states the number of colors.
- [ ] AC-21 (error): If the `palette.import` file does not exist, is in none of the four formats, holds no colors or more than 256 colors, or the document's color mode is not `rgba` (replacing the palette would recolor or orphan pixels that reference entries by position), then the call fails with `isError: true` stating the reason, and the palette is unchanged.
- [ ] AC-22 (event): When the client calls `palette.export` with `path` and `format` (`gpl` with optional `name`, default `palette` as in the editor; `hex`; `pal`; or `png`, a one-row strip of `cell` x `cell` px swatches with `cell` 1-64, default 8), then a file in that format is written regardless of the path's extension. For a palette without duplicate and without fully transparent entries, a `palette.import` of it yields the palette's colors in order, with alpha dropped for `gpl`, `hex` and `pal`, which store none.
- [ ] AC-30 (error): If `palette.export` gets an unknown `format`, `cell` outside 1-64, or a `png` export of an empty palette, then the call fails with `isError: true` stating the reason, and nothing is written.
- [ ] AC-23 (option): Where `--read-only` is given, `palette.export` fails as the other writing tools do (`mcp-headless-pixel` AC-36), while `palette.import` works.
- [ ] AC-24 (event): When the client calls `doc.info` or `doc.list`, then each document reports its `color_mode` (`rgba`, `indexed` or `grayscale` for Pixel documents, `rgba` for Image documents) and its `selection` as the bounding box `{x, y, w, h}` in canvas pixels of every selected (or partly selected) pixel, within the canvas, or `null` without a selection.
- [ ] AC-25 (always): Every successful call of a tool of this spec that changes the document is exactly one undo step, as in `mcp-headless-pixel` AC-09; a successful call that leaves the document unchanged adds no undo step.
- [ ] AC-26 (always): The `pincel://guide` resource names every tool of this spec, the modes it works in, and that `palette.export` drops alpha for `gpl`, `hex` and `pal`.

## Technical notes

Every capability exists already. Grep for these names, because line numbers drift:

| Tool | Pixel (`pincel-core`, used by `pincel-wasm`) | Image (`pincel-image-core` `CommandSpec`) |
| ---- | -------------------------------------------- | ----------------------------------------- |
| `layer.add` placement | `AddLayer::on_top`, `MoveLayerTo` (`LayerPlacement`) | `AddLayer { active }`, `AddGroup`, `MoveLayerTo` |
| `layer.duplicate` | `DuplicateLayer` (groups rejected) | `DuplicateLayer` |
| `layer.move` | `MoveLayerTo` | `MoveLayerTo { place }` |
| `layer.merge_down` | `MergeDown` | `MergeDown` |
| `layer.merge_visible` | — | `MergeVisible` |
| `layer.merge_group` | — | `MergeGroup` |
| `layer.flatten` | `FlattenImage` | `FlattenImage` (onto white) |
| `tag.set` | `SetTagName` / `SetTagRange` / `SetTagDirection` (`tag_ops.rs`), one `CommandBatch` | — |
| `tag.add` color | `Tag` color as in wasm `add_tag(name, from, to, color)` | — |
| `palette.*_entry`, names | wasm `set_palette_color`, `add_palette_entry`, `remove_palette_entry`, `move_palette_entry`, `palette_name` | — |
| `palette.import` / `.export` | `codec/palette_formats.rs` (`PaletteFormat`), as wasm `import_palette` / `export_palette` | — |

Today `layer.list` reports every layer of an Image document as `kind: "image", parent: null`, and an edit on a group fails with "layer is locked". AC-12 and AC-13 fix both.

`palette.import` reads through the workspace sandbox (`E007`). The host passes the bytes in; `pincel-agent` stays free of I/O.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `layer.move` of a layer onto itself (`target` = `layer`) | Fails as in AC-07, document unchanged |
| `layer.move` to the position the layer already has | Succeeds; document unchanged and no undo step |
| `layer.flatten` on a document with one layer | Same result as the editor; one undo step only if the document changed (AC-25) |
| `layer.merge_visible` with no visible layer | Same result as the editor; no undo step if the document is unchanged |
| `layer.remove` of the only layer | Same result as the editor's Delete Layer on the only layer |
| `tag.set` with a name another tag already has | Allowed, as in the editor and the Aseprite format |
| `palette.remove_entry` on the last remaining entry | Same result as the editor |
| `palette.move_entry` with `from` = `to` | Succeeds; palette unchanged, no undo step |
| `palette.import` of a file larger than `doc.open` accepts | Fails like `doc.open` with an oversized file |
| `palette.export` onto an existing file or into a missing directory | Same as `export.png` with that path |
| `layer.merge_down` on a locked layer or onto a locked layer | Fails as in `mcp-headless-pixel` AC-28 |
| `tag.set` with `from` + `to` but only one of them given | Fails as in AC-16, naming the missing one |
| `palette.set_entry` on an indexed document | Changes the entry's color; pixels using the index show the new color, as in the editor |
| `palette.import` of a `.gpl` with names | Entry names are kept, as in the editor |
| `palette.export` to `../x.gpl` | `E007: path outside workspace`, nothing written |
| `palette.export` `png` of an empty palette, or `cell: 0` | Fails with `isError: true` stating the reason, nothing written |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The error cases this spec lists are all the tools must check; where the core command does not check one (lock checks live in the wasm layer), the tool checks it | Decision Log 2026-10-12: "every wasm edit path refuses a locked target"; the tools must not be more permissive than the UI | Medium: a guard that lives only in a Svelte component is missed; found by `/spec-check` against the UI path |
| Lock guards pixels, not position: `layer.duplicate` and `layer.move` work on locked layers (AC-29) | Aseprite's lock flag (`editable`, Decision Log 2026-10-12) prevents edits of the layer's content; the editor's layers panel moves locked layers | Low |
| `palette.set_entry` and `palette.add_entry` work in every color mode while remove, move and import are refused outside `rgba` | Recoloring or appending keeps every pixel's index valid; removing, moving or replacing entries would re-point pixels (`palette_ops.rs`: "RGBA documents only") | Low |
| An Image `layer.add` with `parent` but without `target` adds the new layer as the top child of that group | The same as Pixel `layer.add` with `parent` (`mcp-headless-pixel` AC-15), so one rule holds for both modes | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `tool_schemas_are_valid` (`crates/pincel-agent/src/tests/doc.rs`) | built  |        |
| AC-02 | `image_only_merges_on_a_pixel_document_fail_naming_the_mode` (`crates/pincel-agent/src/tests/layers.rs`), `tag_and_palette_tools_on_an_image_document_fail_naming_the_mode` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |
| AC-03 | `layer_add_without_target_places_the_layer_like_the_editor` (`crates/pincel-agent/src/tests/layers.rs`) | built  |        |
| AC-04 | `layer_add_with_target_and_place_puts_the_layer_there` (`crates/pincel-agent/src/tests/layers.rs`) | built  |        |
| AC-05 | `layer_duplicate_copies_the_layer_directly_above_its_source` (`crates/pincel-agent/src/tests/layers.rs`) | built  |        |
| AC-06 | `layer_move_equals_the_editors_move` (`crates/pincel-agent/src/tests/layers.rs`) | built  |        |
| AC-07 | `layer_move_and_add_with_bad_placement_fail_naming_the_argument` (`crates/pincel-agent/src/tests/layers.rs`) | built  |        |
| AC-08 | `layer_merge_down_equals_the_editors_merge_down` (`crates/pincel-agent/src/tests/layers.rs`) | built  |        |
| AC-09 | `layer_merge_down_the_editor_refuses_fails_with_the_reason` (`crates/pincel-agent/src/tests/layers.rs`) | built  |        |
| AC-10 | `layer_merge_visible_and_merge_group_equal_the_editors` (`crates/pincel-agent/src/tests/layers.rs`) | built  |        |
| AC-11 | `layer_flatten_equals_the_editors_flatten` (`crates/pincel-agent/src/tests/layers.rs`) | built  |        |
| AC-12 | `image_groups_are_listed_with_kind_and_parent` (`crates/pincel-agent/src/tests/layers.rs`) | built  |        |
| AC-13 | `image_pixel_tools_on_a_group_fail_saying_it_is_a_group` (`crates/pincel-agent/src/tests/layers.rs`) | built  |        |
| AC-14 | `tag_add_reports_the_given_color_or_the_timelines_next_color` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |
| AC-15 | `tag_set_changes_exactly_the_given_fields` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |
| AC-16 | `tag_set_with_bad_arguments_fails_naming_the_argument` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |
| AC-17 | `palette_get_reports_entry_names` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |
| AC-18 | `palette_entry_tools_change_exactly_that_entry` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |
| AC-19 | `palette_tools_refuse_a_full_palette_and_non_rgba_documents` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |
| AC-20 | `palette_import_reads_all_four_formats_whatever_the_extension` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |
| AC-21 | `palette_import_of_a_bad_file_fails_with_the_reason`, `palette_tools_refuse_a_full_palette_and_non_rgba_documents` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |
| AC-22 | `palette_export_writes_each_format_and_imports_back` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |
| AC-23 | `read_only_mode_refuses_palette_export_and_allows_palette_import` (`crates/pincel-mcp/tests/stdio.rs`) | built  |        |
| AC-24 | `doc_info_reports_color_mode_and_selection_bounds` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |
| AC-25 | `layer_tools_record_one_undo_step_and_none_without_a_change` (`crates/pincel-agent/src/tests/layers.rs`), `tag_and_palette_edits_record_one_undo_step_and_none_without_a_change` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |
| AC-26 | `read_resource_guide_covers_workflow_and_both_modes`, `guide_names_the_modes_of_the_structure_tools` (`crates/pincel-agent/src/resources.rs`) | built  |        |
| AC-27 | `layer_tools_naming_a_missing_layer_fail_with_argument_and_value` (`crates/pincel-agent/src/tests/layers.rs`), `tag_and_palette_tools_naming_a_missing_index_fail_with_argument_and_value` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |
| AC-28 | `layer_duplicate_of_a_pixel_group_fails_and_changes_nothing` (`crates/pincel-agent/src/tests/layers.rs`) | built  |        |
| AC-29 | `merge_and_flatten_refuse_locked_layers_while_duplicate_and_move_work` (`crates/pincel-agent/src/tests/layers.rs`) | built  |        |
| AC-30 | `palette_export_with_bad_arguments_writes_nothing` (`crates/pincel-agent/src/tests/tags_palette.rs`) | built  |        |

## Decisions

- 2026-11-05: Tool names are unified across modes, with one name per capability and no aliases (Decision Log 2026-11-05) — owner's choice, because one name per capability keeps the tool list small, and clients re-read `tools/list` on every connect.
- 2026-11-05: `layer.add` without `target` follows the editor of the document's mode: Pixel on top, Image above the active layer. `target` + `place` give exact placement — owner's choice, because an agent should get the same result as a user doing the same thing. Only Image documents change behavior; before this spec they put the new layer on top.
- 2026-11-05: One tool per verb (`layer.merge_down`, `layer.merge_visible`, `layer.merge_group`, `layer.flatten`) rather than one `layer.merge { mode }` tool — owner's choice, because small schemas are unambiguous for models.
- 2026-11-05: The parity specs are built in parallel with `mcp-live-desktop` — owner's choice. Live mode runs the same `pincel-agent` code, so every new tool reaches it.
- 2026-11-05 (spec-build): `layer.add` with `kind: "group"` and no `target` / `parent` on an Image document puts the group on top of the root stack, because that is where the editor's Add Group puts it (`AddGroup::on_top`); AC-03's "above the active layer" is the editor's rule for a new image layer.
- 2026-11-05 (spec-build): `layer.move`, `layer.merge_visible` and `layer.flatten` report `changed` (whether an undo step was recorded), because AC-25 lets these calls succeed without changing the document and an agent should not have to diff `layer.list` to find out.
- 2026-11-05 (spec-build): "No change" (AC-25) is decided by a dry run of the same commands on a copy of the document; for Image documents a change of the active layer alone does not count, because the active layer is selection state, not document content.
- 2026-11-05 (spec-build): `palette.get` entries are objects `{color, name}` (color `#RRGGBBAA`), because AC-17 has each entry carry its name; `palette.set`, `palette.load_preset` and the entry tools return the same shape.
- 2026-11-05 (spec-build): Tag colors are reported as `#RRGGBB`, the form `tag.add` takes and AC-14 lists, because tag colors carry no alpha in the timeline or the Aseprite format.
- 2026-11-05 (spec-build): `palette.export` takes `format` as a required argument and ignores the path's extension; `name` applies only to `gpl` and `cell` only to `png`, as in the editor's export.
- 2026-11-06 (spec-build): Acceptance found test gaps in AC-26, AC-27 and AC-29; the behavior existed, so the added checks were green at once. Counter-checks, each reverted: renaming the guide lines "Layers, both modes:" and "Tags, Pixel documents only:" to "Layers:" / "Tags:" made `guide_names_the_modes_of_the_structure_tools` fail (AC-26); committing the new layer before `layer.add` resolves `target` made `layer_tools_naming_a_missing_layer_fail_with_argument_and_value` fail on the unchanged-document check (AC-27); dropping the layer id from the Image lock message made `merge_and_flatten_refuse_locked_layers_while_duplicate_and_move_work` fail (AC-29, which now also covers a Pixel layer inside a locked group).
