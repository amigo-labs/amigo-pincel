---
status: todo
created: 2026-10-29
size: M
mode: Pixel
design: [pincel.md §7.1, pincel.md §7.2, pincel.md §8]
---

# Aseprite round-trip

## Context

`.aseprite` is Pixel mode's project format: Pixel mode opens a file into the `pincel-core` document and saves it back to the same format. The engine pipeline also consumes it directly (pincel.md §7.4), so a file Pincel writes must parse in `aseprite-loader` and in Aseprite itself, and opening then saving must not lose data Pincel does not edit.

Reading wraps `aseprite-loader` (§7.1). Writing goes through the standalone `aseprite-writer` crate (§7.2, §8). That crate has no Pincel dependency, mirrors the loader's data model and is publishable on its own. This spec records the behavior as built on 2026-10-29. Indexed and grayscale files are refused because Phase 1 is RGBA-only. External tilesets are refused because resolving them is Phase 2 (§7.1).

## Scope

### In scope

- Reading an Aseprite v1.3 byte stream into a Pixel document: header, layers (image, group, tilemap) with flags, frames with durations, cels (image, linked, tilemap), palette, tags, slices, tilesets, user data, External Files entries
- Writing a Pixel document back to an Aseprite v1.3 byte stream
- Errors when the input file or the document cannot be represented
- The standalone `aseprite-writer` crate: what it writes, where user data goes, what input it rejects
- The `openAseprite` / `saveAseprite` round trip through the wasm `Document`

### Out of scope

- Indexed and grayscale color modes on read or write — because Phase 1 is RGBA-only; `indexed-grayscale-compose` covers the render side
- Resolving a tileset or palette stored in another file — because §7.1 defers it to Phase 2
- Keeping unknown chunks as opaque blobs — because §7.1 lists the general opaque-chunk mechanism as still open
- Undo history inside the file (cross-session undo) — because §6.2 defers it to Phase 2
- `WriteOptions` (uncompressed cels, linking identical cels) from the §8.2 sketch — because they are not built and nothing needs them yet
- File pickers, save locations, recents and autosave — because the `file-access` and `autosave-recovery` specs own them
- PNG and sprite-sheet export — because the `pixel-export` spec owns them

## Acceptance criteria

- [ ] AC-01 (event): When a Pixel document is saved as `.aseprite` and the bytes are opened again, the canvas size, RGBA color mode, layer names and kinds, frame count, every frame's duration, and every image cel's position, opacity and pixel bytes are unchanged.
- [ ] AC-02 (event): When a document is saved and reopened, each layer keeps its visibility, opacity and blend mode, and a cel placed at an offset keeps that offset and size.
- [ ] AC-03 (event): When a document with nested groups is saved and reopened, every layer keeps its parent group, including a layer that was moved into a group before the save.
- [ ] AC-04 (event): When layers were reordered before a save, the reopened document has the new stacking order and every cel stays on the layer that owned it. Layer ids are renumbered by position on reopen.
- [ ] AC-05 (event): When a linked cel is saved and reopened, it is still linked to the same source frame.
- [ ] AC-06 (event): When tags are saved and reopened, each tag keeps its name, frame range, direction (forward, reverse, ping-pong, ping-pong reverse), color and repeat count.
- [ ] AC-07 (event): When a file has a tag direction value Pincel does not know (above 3), the tag plays forward and a save writes the original value back.
- [ ] AC-08 (event): When a palette is saved and reopened, every entry keeps its color, alpha and name.
- [ ] AC-09 (event): When a tilemap document is saved and reopened, the tilemap layer keeps its tileset, the tileset keeps its name, tile size and tile pixels, and every tilemap cel keeps its grid size, tile ids and horizontal flip, vertical flip and 90° rotation flags.
- [ ] AC-10 (event): When a file is opened that was not written by Pincel and holds a tileset with inline tiles and a compressed tilemap cel, the tilemap layer, tile images and tile grid with decoded flip flags are loaded.
- [ ] AC-11 (event): When slices are saved and reopened, each slice keeps its name, its per-frame keys (bounds, 9-patch center, pivot) and its overlay color including alpha. Slice ids are reassigned from 0 in file order.
- [ ] AC-12 (event): When a file is opened and saved, the header's pixel aspect ratio and grid, layer flag bits other than visible and editable (for example lock movement, background, collapsed, reference), and each cel's z-index are written back unchanged.
- [ ] AC-13 (event): When a file is opened and saved, user data is written back unchanged. This covers text, color and property maps (nested maps included) on the sprite, layers, cels, tags, tilesets, individual tiles and slices, plus the entries of the External Files chunk. Property maps are written back byte for byte.
- [ ] AC-14 (always): Chunk types Pincel does not model are dropped on open and are not in the saved file. Examples are Color Profile (`0x2007`), the legacy palettes (`0x0004` / `0x0011`) and chunk types from future Aseprite versions.
- [ ] AC-15 (error): If the bytes are empty or not an Aseprite file, opening fails with a parse error and no document is created.
- [ ] AC-16 (error): If the file uses indexed or grayscale color mode, opening fails with an "unsupported color mode" error.
- [ ] AC-17 (error): If the file is valid but contains something Pincel cannot load, opening fails with an error that names the problem. The cases are: a tileset stored only in an external file, an unknown layer type, an unknown blend mode, an unknown cel type, a linked cel that points past the last frame, a cel on a layer index that does not exist, or a tilemap cel that does not use 32 bits per tile.
- [ ] AC-18 (error): If the document cannot be represented in the format, saving fails with an error instead of producing a file. The cases are: a non-RGBA color mode, zero frames, or a canvas or cel position outside the format's 16-bit range. For zero frames, no bytes are written.
- [ ] AC-19 (error): If the document is structurally inconsistent, saving fails with an error that names the problem. The cases are:
  - a layer whose parent is missing, is not a group, comes after it, or is hidden behind a sibling group
  - a cycle in the parent graph
  - a cel on an unknown layer
  - an image cel that is not RGBA or has a malformed buffer
  - a linked cel whose target is missing, past the last frame or itself linked
  - a tileset that is external or has a tile of the wrong size
- [ ] AC-20 (always): Every file `aseprite-writer` produces parses with `aseprite-loader`, and its header, layers and blend modes, palette, tags, image, linked and tilemap cels, tilesets, slices, user data and External Files entries read back as they were written.
- [ ] AC-21 (always): `aseprite-writer` writes each User Data chunk directly after the chunk it belongs to:
  - a layer or a cel: right after that chunk
  - the sprite: right after the first frame's palette, before any layer, tileset, tag, slice or cel chunk
  - tags: one per tag, in order, after the Tags chunk, written only when at least one tag has user data
  - a tileset: right after the Tileset chunk, then one for every tile of the tileset, written only when the tileset or at least one tile has user data
  - a slice: right after the Slice chunk
- [ ] AC-22 (error): If `aseprite-writer` is given input the format cannot hold, it returns an error instead of a file. The cases are:
  - zero frames (nothing is written) or more than 65,535 frames
  - an empty palette
  - a tilemap layer without a tileset index
  - a tag whose start frame is after its end frame
  - a cel image whose size does not match its pixel data
  - a cel on a layer index that does not exist
  - a linked cel that points past the last frame
  - a slice with no keys, with keys not in ascending frame order, or with keys that disagree on having a 9-patch
  - a user data property block whose size field does not match its length
- [ ] AC-23 (event): When a document is painted through the wasm `Document`, saved with `saveAseprite` and opened with `openAseprite`, the composed frame shows the painted pixels, and unpainted pixels stay transparent.
- [ ] AC-24 (always, manual: needs the Aseprite application): A file saved by Pincel opens in Aseprite with its layers, frames, tags, slices and tilemaps intact.
- [ ] AC-25 (always): `aseprite-writer` writes the first frame's chunks in the order of Aseprite's own encoder: External Files, Palette, Tilesets, Tags, Layers, Slices, then the cels. In particular every Tileset chunk comes before the Layer chunks that reference it.

## Technical notes

Hints that drift; check the code before relying on them.

- **Read:** `pincel_core::read_aseprite(bytes: &[u8]) -> Result<AsepriteReadOutput, CodecError>` (`codec/aseprite_read.rs`) returns `AsepriteReadOutput { sprite: Sprite, cels: CelMap }`. It runs three passes over the bytes:
  - `aseprite_loader::binary::file::parse_file` reads the header, layers, frames, cels and tags.
  - `parse_raw_file` reads tilesets (`0x2023`), slices (`0x2022`), palette (`0x2019`) and User Data (`0x2020`). `parse_file` drops all of these for RGBA files.
  - Pincel's own envelope walk `raw_chunk_extras` reads property-map blocks (User Data flag `0x4`) and External Files (`0x2008`).

  Layer parents come from `child_level`. Several palette chunks fold into one, with later chunks winning. Legacy palette chunks are not read.
- **Write:** `pincel_core::write_aseprite<W: Write>(sprite: &Sprite, cels: &CelMap, out: &mut W) -> Result<(), CodecError>` (`codec/aseprite_write.rs`). It pre-validates the document, builds an `aseprite_writer::AseFile` and calls `aseprite_writer::write<W: Write>(file: &AseFile, out: &mut W) -> Result<(), WriteError>`. The writer stages the whole file in memory before writing it to `out`.
- **Chunk order in frame 0:**
  1. External Files `0x2008`, only when non-empty
  2. Palette `0x2019`, then the sprite's User Data
  3. per tileset: Tileset `0x2023`, then its own User Data and one per tile
  4. Tags `0x2018`, then one positional User Data per tag
  5. per layer: Layer `0x2004`, then its User Data
  6. per slice: Slice `0x2022`, then its User Data (always carries the overlay color)
  7. Cels `0x2005`, each followed by its User Data

  This matches Aseprite's encoder (`src/dio/aseprite_encoder.cpp`), and Aseprite's decoder depends on it. A tilemap layer looks up its tileset while its Layer chunk is read. A User Data chunk belongs to the sprite only before the first layer, cel, tag, slice or tileset chunk. After a tileset's own User Data, exactly one User Data chunk per tile is read. `aseprite-loader` collects all chunks before resolving them, so a loader round trip does not catch an order problem.

  Later frames hold only cels. Cel pixels and tile pixels are zlib-compressed. Old Palette `0x0004` and Color Profile `0x2007` are never written.
- **Manual check files:** `crates/pincel-core/tests/fixtures/aseprite-compat-check.aseprite` and `aseprite-compat-tilemap-min.aseprite` are Pincel's writer output for AC-24. `tests/aseprite_compat_fixture.rs` fails when the writer drifts from them; regenerate with `PINCEL_REGENERATE_FIXTURES=1`. The checklist for opening them in Aseprite is `crates/pincel-core/tests/fixtures/aseprite-compat-check.md`.
- **Tilemap cel encoding** (Cel Type 3): always `bits_per_tile = 32`, with masks `TILE_ID_MASK = 0x1fff_ffff`, `Y_FLIP_MASK = 0x2000_0000`, `X_FLIP_MASK = 0x4000_0000`, `DIAG_FLIP_MASK = 0x8000_0000`. `TileRef::rotate_90` maps to the diagonal flag.
- **Document fields that exist only for round-tripping:**
  - `Metadata::pixel_ratio`, `Metadata::grid`, `Metadata::user_data`, `Metadata::external_files`
  - `Layer::extra_flags` (every flag bit except visible / editable)
  - `Cel::z_index`, `Cel::user_data`
  - `Tag::raw_direction`, `Tag::user_data`
  - `Tileset::user_data`, `Tileset::tile_user_data` (sparse, by tile id)
  - `Slice::user_data`
  - `UserData::properties` (opaque property block)
- **Header on write:**
  - `color_count` is the palette size, or 256 if that does not fit in a `u16`.
  - A pixel ratio of `(0, 0)` is written as 1:1.
  - Grid values that do not fit fall back to `0` (x / y) and `16` (width / height).
  - `Header::new` sets `flags = 0x1` (layer opacity valid) and `speed = 100`.
- **Errors:** `pincel_core::CodecError` covers both read and write. Writer-side failures pass through as `CodecError::Write(aseprite_writer::WriteError)`.
- **wasm:** `Document.openAseprite(bytes)` and `doc.saveAseprite()` (`crates/pincel-wasm/src/lib.rs`) wrap the two functions. Errors cross as thrown strings prefixed `failed to open Aseprite:` and `failed to save Aseprite:`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Pixel aspect ratio stored as `(0, 0)` (legacy "unset") | Written back as 1:1 |
| Document with an empty palette | No Palette chunk is written; the reopened document has an empty palette |
| Slice without a trailing User Data color | Overlay color defaults to opaque white |
| Tileset chunk with neither the inline-tiles flag nor the external-file flag | Kept as a tileset with zero tiles so the tilemap layer still resolves |
| Palette chunks that leave some indices unwritten | Those entries open as transparent |
| Malformed property block or truncated chunk envelope inside an otherwise valid file | That block is dropped; the file still opens |
| Tag direction edited after opening a file with an unknown direction value | The edited direction is written; the raw value is no longer kept |
| Reopening a saved document | Layer ids and slice ids are renumbered by position; the undo history is empty |
| Grid offset or size beyond 16-bit range | Saved with the defaults (offset 0, size 16) instead of failing |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `aseprite_codec::multi_frame_document_round_trips_canvas_layers_durations_and_cels`, `aseprite_codec::single_layer_single_frame_round_trips_pixels` (crates/pincel-core/tests/aseprite_codec.rs) | built |        |
| AC-02 | `aseprite_codec::multi_layer_blend_modes_and_offsets_are_preserved`, `aseprite_codec::every_blend_mode_visibility_and_opacity_round_trip_per_layer` (crates/pincel-core/tests/aseprite_codec.rs) | built |        |
| AC-03 | `aseprite_codec::group_hierarchy_round_trips_via_child_level`, `aseprite_codec::layer_moved_into_a_group_keeps_its_parent_through_a_round_trip` (crates/pincel-core/tests/aseprite_codec.rs) | built |        |
| AC-04 | `aseprite_codec::reordered_layers_round_trip_with_cels_intact` (crates/pincel-core/tests/aseprite_codec.rs) | built |        |
| AC-05 | `aseprite_codec::linked_cel_within_range_round_trips` (crates/pincel-core/tests/aseprite_codec.rs) | built |        |
| AC-06 | `aseprite_codec::tags_round_trip_every_direction_range_color_and_repeat_count`, `aseprite_codec::tags_round_trip_with_directions` (crates/pincel-core/tests/aseprite_codec.rs) | built |        |
| AC-07 | `aseprite_codec::user_data_properties_and_unknown_tag_direction_round_trip` (crates/pincel-core/tests/aseprite_codec.rs) | built |        |
| AC-08 | `aseprite_codec::palette_round_trips_colors_and_names` (crates/pincel-core/tests/aseprite_codec.rs) | built |        |
| AC-09 | `aseprite_codec::tilemap_round_trips_non_square_tiles_and_grid_on_its_own_tileset`, `aseprite_codec::tilemap_round_trips_layer_tileset_and_cel`, `aseprite_codec::tilemap_round_trips_rotate_90_flag` (crates/pincel-core/tests/aseprite_codec.rs) | built |        |
| AC-10 | `aseprite_codec_tilemap::tilemap_layer_hydrates_with_tileset_id`, `aseprite_codec_tilemap::tileset_hydrates_with_inline_tile_images`, `aseprite_codec_tilemap::tilemap_cel_hydrates_into_grid_with_decoded_flips` (crates/pincel-core/tests/aseprite_codec_tilemap.rs) | built |        |
| AC-11 | `aseprite_codec::slices_round_trip_plain_and_nine_patch_with_pivot` (crates/pincel-core/tests/aseprite_codec.rs) | built |        |
| AC-12 | `aseprite_codec::header_flags_z_index_and_user_data_round_trip` (crates/pincel-core/tests/aseprite_codec.rs) | built |        |
| AC-13 | `aseprite_codec::header_flags_z_index_and_user_data_round_trip`, `aseprite_codec::user_data_properties_and_unknown_tag_direction_round_trip`, `aseprite_codec::external_files_tileset_tile_and_slice_user_data_round_trip` (crates/pincel-core/tests/aseprite_codec.rs) | built |        |
| AC-14 | `aseprite_codec_foreign::read_aseprite_with_unmodeled_chunks_drops_them_on_open_and_save` (crates/pincel-core/tests/aseprite_codec_foreign.rs) | built |        |
| AC-15 | `pincel_core::codec::aseprite_read::tests::empty_byte_slice_returns_parse_error` (crates/pincel-core/src/codec/aseprite_read.rs), `aseprite_codec_foreign::read_aseprite_non_aseprite_bytes_returns_parse_error` (crates/pincel-core/tests/aseprite_codec_foreign.rs), `pincel_wasm::tests::open_aseprite_rejects_garbage_bytes` (crates/pincel-wasm/src/lib.rs) | built |        |
| AC-16 | `aseprite_codec_foreign::read_aseprite_indexed_or_grayscale_returns_unsupported_color_mode` (crates/pincel-core/tests/aseprite_codec_foreign.rs) | built |        |
| AC-17 | `aseprite_codec_foreign::read_aseprite_external_only_tileset_returns_tileset_unsupported`, `…::read_aseprite_unknown_layer_type_returns_unsupported_layer_kind`, `…::read_aseprite_unknown_blend_mode_returns_unsupported_blend_mode`, `…::read_aseprite_unknown_cel_type_returns_unsupported_cel_kind`, `…::read_aseprite_linked_cel_past_last_frame_returns_linked_frame_not_found`, `…::read_aseprite_cel_on_missing_layer_returns_parse_error_naming_layer_index`, `…::read_aseprite_tilemap_cel_not_32_bits_returns_bits_per_tile_unsupported` (crates/pincel-core/tests/aseprite_codec_foreign.rs) | built |        |
| AC-18 | `pincel_core::codec::aseprite_write::tests::write_rejects_indexed_color_mode`, `…::write_rejects_grayscale_color_mode`, `…::write_rejects_canvas_wider_than_u16`, `…::write_rejects_canvas_taller_than_u16`, `…::write_rejects_cel_position_out_of_i16`, `…::write_rejects_cel_y_below_i16` (crates/pincel-core/src/codec/aseprite_write.rs), `aseprite_codec::zero_frame_sprite_fails_to_write_instead_of_emitting_invalid_file` (crates/pincel-core/tests/aseprite_codec.rs) | built |        |
| AC-19 | `pincel_core::codec::aseprite_write::tests::write_detects_unknown_parent`, `…::write_rejects_layer_parent_that_is_not_a_group`, `…::write_rejects_layer_whose_parent_appears_after_it`, `…::write_rejects_layer_whose_parent_is_shadowed_by_sibling_group`, `…::write_detects_layer_parent_cycle`, `…::write_rejects_cel_for_unknown_layer`, `…::write_rejects_image_cel_with_indexed_buffer`, `…::write_rejects_image_cel_with_malformed_buffer`, `…::write_rejects_linked_cel_with_missing_target`, `…::write_rejects_linked_cel_past_frames`, `…::write_rejects_chained_linked_cel`, `…::write_rejects_external_file_tileset`, `…::write_rejects_tileset_tile_with_wrong_dimensions` (crates/pincel-core/src/codec/aseprite_write.rs) | built |        |
| AC-20 | `roundtrip::empty_rgba_sprite_roundtrips_through_loader`, `roundtrip::three_layers_with_blend_modes_roundtrip`, `roundtrip::palette_roundtrips_via_raw_file`, `roundtrip::tags_roundtrip_with_directions`, `roundtrip::single_image_cel_roundtrips_with_pixels`, `roundtrip::linked_cel_roundtrips_pointing_at_source_frame`, `roundtrip::multi_cel_across_layers_and_frames_roundtrips`, `roundtrip::full_file_with_layers_palette_tags_roundtrips`, `roundtrip::tileset_chunk_roundtrips_through_loader`, `roundtrip::tilemap_cel_roundtrips_through_loader`, `roundtrip::user_data_properties_are_written_verbatim_and_parse`, `roundtrip::unknown_tag_direction_roundtrips_as_its_raw_byte`, `roundtrip::header_fields_roundtrip_through_loader`, `roundtrip::every_blend_mode_roundtrips_through_loader`, `roundtrip::every_tag_direction_and_color_roundtrips_through_loader`, `roundtrip::external_files_and_tileset_user_data_are_written_in_place`, `roundtrip::slice_user_data_follows_its_slice_chunk` (crates/aseprite-writer/tests/roundtrip.rs), `aseprite_writer::write::tests::slice_chunk_round_trips_through_loader` (crates/aseprite-writer/src/write.rs) | built |        |
| AC-21 | `roundtrip::user_data_follows_its_layer_cel_palette_and_tags`, `roundtrip::sprite_user_data_precedes_every_object_chunk`, `roundtrip::tags_get_no_user_data_chunks_when_no_tag_has_any`, `roundtrip::external_files_and_tileset_user_data_are_written_in_place`, `roundtrip::tileset_user_data_has_one_chunk_per_tile`, `roundtrip::slice_user_data_follows_its_slice_chunk` (crates/aseprite-writer/tests/roundtrip.rs) | built |        |
| AC-22 | `aseprite_writer::write::tests::write_rejects_zero_frames`, `…::write_rejects_more_frames_than_u16_can_hold`, `…::empty_palette_is_rejected`, `…::tilemap_layer_without_index_is_rejected`, `…::invalid_tag_range_is_rejected`, `…::cel_image_size_mismatch_is_rejected`, `…::cel_layer_index_out_of_range_is_rejected`, `…::linked_cel_frame_out_of_range_is_rejected`, `…::slice_without_keys_is_rejected`, `…::slice_keys_must_be_strictly_ascending`, `…::slice_keys_must_agree_on_nine_patch_presence` (crates/aseprite-writer/src/write.rs), `roundtrip::write_rejects_unrepresentable_input_without_writing_bytes`, `roundtrip::user_data_properties_with_a_wrong_size_are_rejected` (crates/aseprite-writer/tests/roundtrip.rs) | built |        |
| AC-23 | `paint_save_open_roundtrip::paint_save_open_roundtrip_preserves_pixels` (crates/pincel-wasm/tests/paint_save_open_roundtrip.rs) | built |        |
| AC-24 | manual check pending: `crates/pincel-core/tests/fixtures/aseprite-compat-check.aseprite` and `aseprite-compat-tilemap-min.aseprite`, checklist `crates/pincel-core/tests/fixtures/aseprite-compat-check.md`; drift check `aseprite_compat_fixture::aseprite_compat_check_fixture_matches_writer_output`, `…::aseprite_compat_tilemap_min_fixture_matches_writer_output` (crates/pincel-core/tests/aseprite_compat_fixture.rs) | open   |        |
| AC-25 | `roundtrip::tilesets_precede_the_layers_that_reference_them`, `roundtrip::first_frame_chunk_order_matches_aseprite` (crates/aseprite-writer/tests/roundtrip.rs) | built |        |

## Decisions

- 2026-10-29: Extracted from the code as built (reverse engineering). Unknown chunks are dropped (AC-14), matching the "still open" note in pincel.md §7.1 rather than its opaque-blob paragraph.
- 2026-10-29: Every new test in this pass was green at once (the behavior already existed). Each was shown to fail by a temporary, reverted break of the code it pins:
  - Write adapter: frame durations clamped to 100, `Reverse` written as ping-pong, `Screen` as multiply, linked targets forced to frame 0, tileset tile width / height swapped, tile user-data color dropped, the canvas-height / cel-y range checks and the grayscale color-mode check removed.
  - Reader: layer ids kept by name, slice ids offset by 3, negative cel y clamped, every read error branch (layer type, blend mode, cel type, linked frame, bits per tile, external tileset) turned into a fallback, grayscale accepted, parse errors re-labelled, and the legacy `0x0004` palette folded.
  - Writer: a Color Profile chunk emitted, header `grid_y` written as `grid_x`, tag color bytes swapped, the `Hue` blend code changed, tag user data written when no tag has any, slice user data skipped without a color, and empty palettes silently skipped.
  - wasm: one pixel the old spot checks did not cover made opaque on read.
  - The old `tags_round_trip_with_directions`, `tilemap_round_trips_layer_tileset_and_cel`, `multi_layer_blend_modes_and_offsets_are_preserved` and `write::tests::empty_palette_is_rejected` stayed green under the same breaks. That is why the stronger tests were added next to them.
- 2026-10-29: AC-17's "cel on a layer index that does not exist" fails inside `aseprite-loader`'s `parse_file`, which checks the index first. The error is `CodecError::Parse("… LayerIndexOutOfBounds")`, so Pincel's own `CodecError::LayerIndexOutOfRange` is not reached from a file. The error still names the problem, so the AC holds as written, and the test pins the loader's message.
- 2026-10-29: Aseprite 1.3 failed to open a Pincel file with a tilemap. Its Console showed "Error: tileset 0 not found" and "Frame 0 didn't found layer with index 4". `aseprite-writer` wrote the Tileset chunks after the Layer chunks, so Aseprite dropped the tilemap layer and its cel. Checking Aseprite's decoder (`src/dio/aseprite_decoder.cpp`) turned up two more order dependencies:
  - The sprite's User Data went to the last layer, because it followed the layers.
  - A tileset's User Data run stopped at the last tile with data, and Aseprite warns unless it gets one chunk per tile.

  The fix in `aseprite-writer` writes Aseprite's encoder order. This revises AC-21 (sprite user data placement, one chunk for every tile) and adds AC-25 for the order itself. The AC-21 revision changes behavior: Aseprite would not otherwise open these files correctly, and AC-24 requires that it does.
- 2026-10-29: The AC-24 check files are committed fixtures with a byte-for-byte drift test, so the file checked in Aseprite is the file the writer produces. The fixture test reads both files back to assert their structure. Regenerating them is an explicit step (`PINCEL_REGENERATE_FIXTURES=1`), the same idea as the generated TS bindings.
- 2026-10-29: The AC-16 fixtures carry a palette chunk. `aseprite-loader` rejects an indexed file without one as a parse error (`PaletteError(Missing)`) before the color mode is checked. Aseprite always writes a palette, so a palette-less indexed file is malformed and falls under AC-15, not AC-16.
