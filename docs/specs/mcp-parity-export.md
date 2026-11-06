---
status: todo
created: 2026-11-05
size: L
mode: both
design: [mcp.md, pincel.md §7, pincel.md §15 (2026-10-29 animated export), pincel.md §15 (2026-10-29 PSD import), pincel.md §15 (2026-10-29 Image-mode save), pincel.md §15 (2026-10-29 agent tools in pincel-agent), pincel.md §15 (2026-11-01 Vectorizer core as pincel-trace), pincel.md §15 (2026-11-01 SVG export dialog), pincel.md §15 (2026-11-05 MCP tool parity)]
depends_on: [mcp-parity-structure, psd-import, vectorize-dialog]
---

# MCP parity: import and export

## Context

`mcp-headless-pixel` and `mcp-headless-image` give agents a headless `pincel-mcp` server. Since then the editor has learned to export SVG (`vectorize-dialog`), animated APNG and GIF (`animated-export`), to save an Image document back in its own format including BMP and TIFF (`save-original-format`), to open PSD files (`psd-import`), and the Pixel editor opens a PNG as a one-layer sprite. None of this is reachable through MCP: `doc.save` writes Image documents only as `.ora`, `doc.open` of a `.psd` fails in the decoder, and every PNG becomes an Image document.

This is the import / export spec of the five `mcp-parity-*` specs (Decision Log 2026-11-05 "MCP tool parity"). It exposes existing capabilities; it adds no new file format and no new codec.

**"The editor" in this spec** means the result the editor's UI produces for the same action in a document of that mode, which is the result of the core call (codec, `pincel-trace` session, animation export) the UI issues for it. A test compares the tool's output with that call applied directly to an equal document, and checks the concrete values this spec states.

**Conventions** inherited from `mcp-headless-pixel` and `mcp-parity-structure`: every tool takes an optional `doc`; frame indexes are 0-based; colors follow `mcp-headless-pixel` AC-21, except `export.svg` `fill`, which takes `#RRGGBB` only (AC-07); paths are workspace-relative and go through the sandbox (`E007`); `--read-only` refuses every write (`mcp-headless-pixel` AC-36). A tool works on every mode whose editor offers the capability and fails with the mode error of `mcp-headless-image` AC-05 elsewhere.

This spec supersedes `mcp-headless-image` AC-20 (and its Decision "the server never flattens or overwrites on its own"): `doc.save` now writes an Image document back in its own format, as the editor's Save does since `save-original-format`, but only flattens when the call says so (AC-14).

Terms:

- **Source image** of an export: Pixel mode, one frame composed at 1× from the visible layers, as `export.png`; Image mode, the composite of the visible layers, as `export.png`.
- **Flat format**: PNG, JPEG, WebP, BMP or TIFF — the formats Image-mode Save writes back. GIF and PSD are not flat formats in this spec: Pincel opens them but cannot write them.
- **Detected format**: the format of an opened file as recognized from its content at open (`save-original-format` Assumptions), whatever its extension.
- **Tool messages** (errors, `warnings`, `notices`) are English, as every `pincel-mcp` message is.
- **Lossy flatten**: writing a flat format would lose something, by the editor's flatten-prompt rule: the document has more than one layer, a hidden layer, or a layer whose opacity is below 100 % or whose blend mode is not Normal.

## Scope

### In scope

- `export.svg` in both modes: colors, monochrome and pixel-exact tracing with the options of the editor's Export as SVG dialog, frame choice in Pixel mode, counts in the result
- `export.apng` and `export.gif` for Pixel documents: all frames or one tag in its direction, scale 1–10, loop count, the GIF warnings of the editor's dialog
- `export.bmp` and `export.tiff` for Image documents
- `doc.save` of Image documents in every format the editor's Save / Save As writes (`.ora` and the flat formats), save back to the opened file's own format, JPEG quality, and an explicit `flatten` for a lossy flatten
- `doc.open` of `.psd` files as Image documents, with the editor's import notices
- `doc.open` with `mode`, so a PNG opens as a one-layer Pixel document as in the Pixel editor's File ▸ Open
- `--read-only` and the workspace sandbox for every new path argument
- `pincel://guide` documents the new tools and arguments

### Out of scope

- A PNG compression level argument — because the editor offers none (it always writes level 6, as `export.png` and `doc.save` already do), and parity reaches what the editor can do
- A pinned palette and the turn policy for `export.svg` — because the editor's dialog offers neither (`vectorize-dialog` Out of scope); a later spec adds them to the dialog and the tool together
- Writing GIF or PSD from Image documents — because the editor has no writer for them (`save-original-format` AC-14, `psd-import` AC-12)
- Animated export from Image documents, BMP / TIFF / JPEG / WebP export from Pixel documents, Pixel `doc.save` to anything but `.aseprite` — because no editor of that mode offers it
- Opening JPEG, WebP, BMP, GIF, TIFF, PSD or `.ora` as Pixel documents — because the Pixel editor opens only `.aseprite` and PNG and hands every other file to Image mode
- Returning the SVG, APNG or GIF as MCP image content — because MCP image content is a still raster and Pincel has no SVG rasterizer; `doc.preview` shows the frames, and the file is in the workspace
- Animated WebP, video and per-layer export — because the editor has none of them (`animated-export` Out of scope)
- Live control of the open desktop document — because `mcp-live-desktop` covers it with the same `pincel-agent` tools

## Acceptance criteria

### Tools and modes

- [ ] AC-01 (always): `tools/list` contains `export.svg`, `export.apng`, `export.gif`, `export.bmp` and `export.tiff`, each meeting `mcp-headless-pixel` AC-02, and the schemas of `doc.open` and `doc.save` list the new arguments of this spec.
- [ ] AC-02 (error): If `export.apng` or `export.gif` is called on an Image document, or `export.bmp` or `export.tiff` on a Pixel document, then the call fails with `isError: true`, the message names the document's mode, and nothing is written.
- [ ] AC-03 (option): Where `--read-only` is given, `export.svg`, `export.apng`, `export.gif`, `export.bmp`, `export.tiff` and `doc.save` in every format fail as the other writing tools do (`mcp-headless-pixel` AC-36) and write nothing, while `doc.open` with `mode` and of `.psd` files keeps working.
- [ ] AC-04 (error): If a `path` argument of a tool of this spec resolves outside the workspace, then the call fails with `isError: true` and `E007: path outside workspace`, and nothing is read or written.

### SVG

- [ ] AC-05 (event): When the client calls `export.svg` with a `path`, then an SVG file is written there that is byte-identical to the SVG the editor's Export as SVG dialog saves for the same source image and options, and the result reports `layers`, `outlines`, the file size in `bytes`, and `threshold` (the threshold used in monochrome mode, `null` in the other modes).
- [ ] AC-06 (option): Where `export.svg` gets no `mode`, it traces pixel-exact on a Pixel document and in colors mode on an Image document, as the dialog preselects; every argument left out takes the dialog's default: `colors` 6, `threshold` `"auto"`, `invert` false, `fill` `#000000`, `despeckle` false, `min_area` 2, `alpha_max` (corner threshold) 1.0, `opt_curve` (curve optimisation) true, `opt_tolerance` (optimisation tolerance) 0.2.
- [ ] AC-07 (option): Where `export.svg` gets `mode` `colors` (`colors` 2–16), `mono` (`threshold` `"auto"` or 0–255, `invert`, `fill` as `#RRGGBB` only, because the traced fill has no alpha and no palette) or `pixel_exact`, together with `despeckle`, `min_area` (0–100000), `alpha_max` (0–1.34) and `opt_curve` / `opt_tolerance` (≥ 0), then the file equals the dialog's export with these options; the options a mode ignores do not change the file.
- [ ] AC-08 (option): Where `export.svg` on a Pixel document gets `frame`, that frame is traced; without it, the active frame is traced, as the dialog traces the current frame; on an Image document `frame` may only be 0, as for the other Image tools; hidden layers never appear in the file.
- [ ] AC-09 (error): If an `export.svg` option is outside its range or of the wrong form (including options the chosen mode ignores, and a `fill` in any form but `#RRGGBB`), `mode` is unknown, or `frame` does not exist (any `frame` but 0 on an Image document), then the call fails with `isError: true` naming the first invalid argument and its accepted range, and nothing is written.
- [ ] AC-10 (error): If `export.svg` in `pixel_exact` mode meets more than 256 distinct colors with alpha > 0, then the call fails with `isError: true` stating the exact number of colors and suggesting `mode: "colors"`, and nothing is written.

### Animation (Pixel)

- [ ] AC-11 (event): When the client calls `export.apng` or `export.gif` with a `path` (and optional `tag`, `scale`, `loops`), then the file written is byte-identical to the editor's File ▸ Export ▸ Animation… output with the same format, frames choice, scale and loop count, and the result reports `path`, `frames` (the number of exported frames), `width` and `height` of the output and `loops` (the loop count used, 0 for forever).
- [ ] AC-12 (option): Where `tag` is given, only the tag's frames are exported in the order of its direction (`animated-export` AC-04); without it every frame once, forward. `scale` is 1–10 (default 1); `loops` is 0 for forever or N ≥ 1 plays, and defaults to the tag's repeat count when it is non-zero, else 0 (`animated-export` AC-10).
- [ ] AC-13 (state): While an `export.gif` has a frame that does not fit a 256-entry color table, or a frame with semi-transparent pixels (alpha 1–254) — the two cases the editor's dialog warns about (`animated-export` AC-14, AC-15) — the result's `warnings` list holds `{ "code": "colors_approximated" }` or `{ "code": "transparency_on_off" }` respectively, each with an English `message`, and the GIF is still written; otherwise `warnings` is empty. `export.apng` results carry no `warnings`.
- [ ] AC-27 (error): If `tag` names no tag of the document, `scale` is outside 1–10, `loops` is negative or above 65535, the scaled output is wider or taller than 16384 px, or the renderer refuses the sprite, then the call fails with `isError: true` stating the reason as the editor's export error does, and nothing is written.

### Image save and export

- [ ] AC-14 (event): When `doc.save` is called without `path` on an Image document opened from a flat format, then the file it was opened from is written in its detected format, as the editor's Save writes it; the document keeps all its layers, its dirty state becomes false, and a later `doc.save` without `path` writes the same file again.
- [ ] AC-15 (error): If such a `doc.save` would be a lossy flatten and the call does not carry `flatten: true`, then the call fails with `isError: true` naming every cause that applies (the layer count, each hidden layer, each layer below 100 % opacity, each non-Normal blend mode) and suggesting `flatten: true` or a `.ora` path, and nothing is written.
- [ ] AC-16 (option): Where `doc.save` carries `flatten: true`, a lossy flatten writes the flattened composite in the file's format, equal to the editor's Save after choosing *Flatten*, and the document keeps every layer.
- [ ] AC-17 (event): When `doc.save` on an Image document gets a `path` ending in `.ora`, `.png`, `.jpg` / `.jpeg`, `.webp`, `.bmp` or `.tif` / `.tiff` (any case), then the document is written in that format (flat formats under the rules of AC-15 and AC-16), the path becomes the document's file and format for later `doc.save` calls without `path`, and the dirty state becomes false.
- [ ] AC-18 (option): Where `doc.save` writes JPEG, `quality` 1–100 sets the JPEG quality; without it the quality of the document's last JPEG `doc.save` is reused, else 90, as the editor asks once per document with default 90.
- [ ] AC-28 (error): If `quality` is outside 1–100 or not an integer, or `flatten` or `quality` is passed to `doc.save` on a Pixel document, then the call fails with `isError: true` naming the argument (and, for a Pixel document, saying it applies to Image documents only), and nothing is written.
- [ ] AC-19 (error): If `doc.save` without `path` is called on an Image document opened from a GIF or a PSD, or with a `path` of another extension (`.gif`, `.psd`, unknown), then the call fails with `isError: true` saying that format cannot be written and listing the extensions `doc.save` writes; for a PSD source or a document with more than one layer the message suggests a `.ora` path first, as the editor's Save As preselects `.ora` there. Nothing is written.
- [ ] AC-20 (event): When the client calls `export.bmp` or `export.tiff` with a `path` on an Image document, then a file is written that is byte-identical to the editor's BMP or TIFF save of the composite, it decodes pixel-equal to the composite, and the document's file, format and dirty state do not change.

### Opening

- [ ] AC-21 (event): When the client calls `doc.open` with a PSD file — content starting with `8BPS`, or, when the content matches no format Pincel reads, a name ending in `.psd`, as the editor recognizes it (content first) — then the session holds an Image document with the layers, groups, layer properties and pixels the editor opens from that file, it becomes active, and the result carries `notices`: one English string per item the editor's open notice lists (`psd-import` AC-04, AC-06, AC-07, AC-14), in the same order (bottom layer first), each naming the layer or PSD blend key it concerns; empty when nothing was dropped or changed.
- [ ] AC-22 (error): If a PSD is PSB, not 8 bits per channel, not RGB, has an invalid or truncated header or unreadable composite, or exceeds Image mode's size limits (`mcp-headless-image` AC-21), then `doc.open` fails with `isError: true` naming the file and the editor's reason, and no document is added.
- [ ] AC-23 (option): Where `doc.open` gets `mode: "pixel"` and the file's content is a PNG, then the session holds a Pixel document equal to the one the Pixel editor's File ▸ Open makes from it — one layer `Layer 1` holding the decoded pixels, one 100 ms frame, the New Pixel document's palette — with no file of its own, so `doc.save` without `path` fails as in `mcp-headless-pixel` AC-31, suggests `<stem>.aseprite`, and never writes the PNG.
- [ ] AC-24 (option): Where `doc.open` gets no `mode`, files open as before this spec (a name ending in `.aseprite` or `.ase` as Pixel, everything else as Image, now including PSD); where it gets `mode: "image"`, the file opens as an Image document; where it gets `mode: "pixel"`, a name ending in `.aseprite` or `.ase` opens as without `mode`, and otherwise the content decides (AC-23).
- [ ] AC-25 (error): If `doc.open` gets `mode: "pixel"` for a file whose name does not end in `.aseprite` / `.ase` and whose content is not PNG, `mode: "image"` for a file whose name ends in `.aseprite` / `.ase`, or a `mode` other than `pixel` and `image`, then the call fails with `isError: true` naming the mode and the file's detected format (or "unknown"), and no document is added.
- [ ] AC-29 (error): If `doc.open` with `mode: "pixel"` reads a PNG the decoder refuses (truncated or corrupt data), then the call fails with `isError: true` naming the file and the decoder's reason, and no document is added.

### Guide

- [ ] AC-26 (always): The `pincel://guide` resource names every tool and argument of this spec, the modes each works in, that `doc.save` of a layered Image document to a flat format needs `flatten: true`, and that a PNG opens as a Pixel document only with `mode: "pixel"`.

## Technical notes

Every capability exists already; grep for the names, line numbers drift.

| Tool | Pixel | Image |
| ---- | ----- | ----- |
| `export.svg` | `pincel_trace::TraceSession` over the frame composed as wasm `Document::vectorizer(frame)` (`crates/pincel-wasm/src/vectorize.rs`), `to_svg` | the same over `compose(layers)` as wasm `vectorizer(handle)` (`crates/pincel-image-wasm/src/vectorize.rs`) |
| `export.apng` / `export.gif` | `pincel_core::export_animation` with `AnimationOptions` / `AnimationFormat`; warnings from `animation_info` (as wasm `exportAnimation` / `animationInfo`, `crates/pincel-wasm/src/animation.rs`) | — |
| `export.bmp` / `export.tiff` | — | `to_bmp_bytes` / `to_tiff_bytes` (`crates/pincel-image-core/src/codec/mod.rs`), as wasm `export_bmp` / `export_tiff` |
| `doc.save` flat formats | — | `encode` in `ui/src/modes/image/core/document-file.ts` (PNG level 6, JPEG clamped 1–100, WebP lossless); lossy-flatten rule `flatteningLoses` |
| `doc.open` `.psd` | — | `codec::psd::from_psd_bytes` with `PsdLimits` 16384 / 64 Mpx; `PsdNotice` |
| `doc.open` `mode: "pixel"` PNG | wasm `Document::open_png` (`crates/pincel-wasm/src/import.rs`) over `pincel_core::import_png`; UI `openOpened` in `ui/src/modes/pixel/file-ops.ts` | — |

- `pincel-agent` gains `pincel-trace` as a dependency. It is a workspace crate that both wasm crates already depend on, with `thiserror` only, so no new external crate enters `Cargo.lock`.
- The SVG options map onto `TraceOptions` directly (not through `from_wire`): `TraceSession::trace` already checks the Potrace ranges, and the tool checks `colors`, `threshold`, `fill` and `min_area` in the schema. The dialog clamps silently; the tool refuses instead (AC-09), because an agent needs to know its value was not used.
- The Image document keeps its detected format next to `source` today (the helper `flat_format` in `crates/pincel-agent/src/image/doc.rs`, which despite its name also yields GIF and must gain PSD); AC-14 / AC-17 turn it into the save target, like `document-file.ts` does in the UI.
- All writes go through `FileHost::write`, which owns the sandbox and the read-only flag, so AC-03 / AC-04 need no per-tool code beyond tests.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `export.svg` of a fully transparent document | Succeeds; a valid SVG without paths, `layers: 0`, `outlines: 0` |
| `export.svg` `mono` on an all-white image | Succeeds with zero layers, as the dialog |
| `export.svg` with `colors: 40` and `mode: "mono"` | Fails naming `colors` (AC-09), although mono ignores it |
| `export.svg` on a Pixel document with a hidden layer holding all content | Zero layers (AC-08) |
| `export.apng` of a one-frame sprite | A valid APNG with one frame, as the editor |
| `export.gif` with `loops: 1` | GIF without a NETSCAPE loop extension (`animated-export` AC-11) |
| `export.gif` at `scale: 10` of a 2000 px wide sprite | Fails: 20000 px exceeds 16384 (AC-27) |
| `doc.save` without `path` on a single-layer, full-opacity, Normal document opened from PNG | Writes the PNG back; no `flatten` needed |
| `doc.save` with `flatten: true` when nothing would be lost | Same as without it |
| PNG bytes in `photo.jpg`, `doc.save` without `path` | Writes PNG bytes to `photo.jpg` (format detected at open) |
| `doc.save` to `shot.PNG` | Accepted as PNG (extension case ignored) |
| `doc.save` with `quality` on a PNG target | `quality` is ignored for formats other than JPEG, as the editor applies it to JPEG only |
| `doc.open` of a `.psd` whose content is a PNG | Opens by its content as a flat Image document, as in the editor |
| `doc.open` with `mode: "pixel"` of an `.aseprite` file | Opens as without `mode` |
| `doc.open` with `mode: "pixel"` of a `.png` whose content is JPEG | Fails as in AC-25 naming JPEG |
| `export.bmp` onto an existing file or into a missing directory | Same as `export.png` with that path |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| `export.svg` on a Pixel document defaults to the active frame, while `export.png` defaults to frame 0 | The dialog traces the current frame, and the editor's behavior is the reference; `frame` makes either explicit | Low |
| One tool per output format (`export.apng`, `export.gif`, `export.bmp`, `export.tiff`) instead of `export.animation { format }` or `export.image { format }` | Follows `export.png` / `export.jpeg` / `export.webp` and the parity rule "small schemas are unambiguous for models" (`mcp-parity-structure` Decisions) | Low |
| `doc.open` with `mode: "pixel"` has no size cap of its own beyond what the core accepts | The Pixel editor's `openPng` has none; the 1–4096 cap of `doc.new` is the New dialog's | Low |
| The `--read-only` rule needs no new mechanism | Every write already goes through the host's `write`, which refuses in read-only mode | Low |
| PSD notices are returned in the result's `notices`, not as an error | The editor opens the file and shows a status notice; an agent reads the result | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `export_tools_are_listed_and_open_save_take_the_new_arguments` (`crates/pincel-agent/src/tests/export.rs`), `tool_schemas_are_valid` (`crates/pincel-agent/src/tests/doc.rs`) | built  |        |
| AC-02 | `animation_exports_on_image_documents_fail_naming_the_mode` (`crates/pincel-agent/src/tests/export_anim.rs`), `export_bmp_and_tiff_write_the_composite_and_leave_the_document` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-03 | `file_tools_refuse_read_only_and_escaping_paths`, `read_only_refuses_every_save_format_and_keeps_opening` (`crates/pincel-mcp/tests/stdio.rs`) | built  |        |
| AC-04 | `file_tools_refuse_read_only_and_escaping_paths`, `every_path_argument_outside_the_workspace_is_refused` (`crates/pincel-mcp/tests/stdio.rs`) | built  |        |
| AC-05 | `export_svg_reports_the_dialogs_counts`, `export_svg_defaults_equal_the_dialog_in_each_mode`, `export_svg_options_equal_the_dialogs`, `export_svg_of_nothing_visible_has_no_layers` (`crates/pincel-agent/src/tests/export_svg.rs`) | built  |        |
| AC-06 | `export_svg_each_option_and_default_is_the_dialogs`, `export_svg_defaults_to_pixel_exact_on_pixel_and_colors_on_image`, `export_svg_defaults_equal_the_dialog_in_each_mode` (`crates/pincel-agent/src/tests/export_svg.rs`) | built  |        |
| AC-07 | `export_svg_each_option_and_default_is_the_dialogs`, `export_svg_options_equal_the_dialogs` (`crates/pincel-agent/src/tests/export_svg.rs`) | built  |        |
| AC-08 | `export_svg_traces_the_active_or_given_frame_and_skips_hidden_layers`, `export_svg_defaults_equal_the_dialog_in_each_mode`, `export_svg_of_nothing_visible_has_no_layers` (`crates/pincel-agent/src/tests/export_svg.rs`) | built  |        |
| AC-09 | `bad_svg_arguments_write_nothing`, `bad_svg_argument_forms_name_the_argument_and_its_range` (`crates/pincel-agent/src/tests/export_svg.rs`), `validate_type_error_names_the_accepted_range_or_values` (`crates/pincel-agent/src/schema.rs`) | built  |        |
| AC-10 | `pixel_exact_svg_refuses_more_than_256_colors`, `pixel_exact_svg_counts_only_visible_colors_up_to_256` (`crates/pincel-agent/src/tests/export_svg.rs`) | built  |        |
| AC-11 | `export_apng_and_gif_equal_the_editors_animation_export`, `animation_loops_follow_the_tag_or_the_argument`, `export_gif_warns_about_each_case_on_its_own` (`crates/pincel-agent/src/tests/export_anim.rs`), `median_cut_same_colors_give_the_same_palette_whatever_the_hash_order` (`crates/pincel-core/src/codec/animation/quantize.rs`) | built  |        |
| AC-12 | `export_apng_and_gif_equal_the_editors_animation_export`, `animation_loops_follow_the_tag_or_the_argument` (`crates/pincel-agent/src/tests/export_anim.rs`) | built  |        |
| AC-13 | `export_gif_warns_about_each_case_on_its_own`, `export_gif_warns_about_what_gif_cannot_keep` (`crates/pincel-agent/src/tests/export_anim.rs`) | built  |        |
| AC-14 | `doc_save_without_path_writes_back_the_opened_format`, `doc_save_writes_back_the_detected_format_whatever_the_extension` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-15 | `a_lossy_flatten_names_each_cause_and_flatten_writes_the_visible_composite`, `a_lossy_flatten_needs_flatten_true_and_names_every_cause`, `doc_save_with_a_flat_path_refuses_a_lossy_flatten_and_flatten_writes_it` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-16 | `a_lossy_flatten_names_each_cause_and_flatten_writes_the_visible_composite`, `a_lossy_flatten_needs_flatten_true_and_names_every_cause`, `flatten_true_without_loss_writes_the_same_file`, `doc_save_with_a_flat_path_refuses_a_lossy_flatten_and_flatten_writes_it` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-17 | `doc_save_writes_each_format_and_makes_the_path_the_documents_file`, `doc_save_with_a_path_picks_the_format_by_extension`, `doc_save_with_a_flat_path_refuses_a_lossy_flatten_and_flatten_writes_it` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-18 | `jpeg_quality_is_remembered_per_document`, `jpeg_quality_applies_to_jpeg_only_and_per_document` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-19 | `unwritable_formats_are_refused_listing_the_extensions`, `doc_save_of_a_psd_source_or_layered_document_suggests_ora_first` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-20 | `export_bmp_and_tiff_write_the_composite_and_leave_the_document` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-21 | `doc_open_of_a_psd_equals_the_editors_document_and_becomes_active`, `doc_open_recognizes_psd_by_content_first_then_by_name`, `doc_open_of_a_psd_opens_the_editors_layers_and_notices` (`crates/pincel-agent/src/tests/export_open.rs`), `psd_notice_names_the_layer_or_blend_key_of_each_item` (`crates/pincel-agent/src/image/doc.rs`) | built  |        |
| AC-22 | `doc_open_of_an_unsupported_psd_states_the_editors_reason`, `doc_open_of_a_bad_psd_names_the_file_and_reason` (`crates/pincel-agent/src/tests/export_open.rs`) | built  |        |
| AC-23 | `doc_open_mode_pixel_equals_a_new_pixel_document_with_the_pixels`, `doc_open_mode_pixel_opens_a_png_as_the_pixel_editor_does` (`crates/pincel-agent/src/tests/export_open.rs`) | built  |        |
| AC-24 | `doc_open_mode_routes_by_name_then_content`, `doc_open_mode_selects_or_refuses_the_mode` (`crates/pincel-agent/src/tests/export_open.rs`) | built  |        |
| AC-25 | `doc_open_with_an_unfitting_mode_names_the_mode_and_detected_format`, `doc_open_mode_selects_or_refuses_the_mode` (`crates/pincel-agent/src/tests/export_open.rs`) | built  |        |
| AC-26 | `read_resource_guide_covers_workflow_and_both_modes` (`crates/pincel-agent/src/resources.rs`) | built  |        |
| AC-27 | `animation_errors_state_the_editors_reason`, `bad_animation_arguments_write_nothing` (`crates/pincel-agent/src/tests/export_anim.rs`) | built  |        |
| AC-28 | `bad_save_arguments_name_the_argument` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-29 | `doc_open_mode_pixel_of_a_corrupt_png_states_the_decoders_reason`, `doc_open_mode_pixel_of_a_corrupt_png_names_the_file` (`crates/pincel-agent/src/tests/export_open.rs`) | built  |        |

## Decisions

- 2026-11-05: `doc.save` writes an Image document back in its own format like the editor's Save, and a lossy flatten (more than one layer, a hidden layer, opacity below 100 % or a non-Normal blend) needs `flatten: true`, otherwise the call fails (AC-14 – AC-16); this supersedes `mcp-headless-image` AC-20 and its Decision "the server never flattens or overwrites on its own" — owner's choice, because agents should reach what the editor's Save does, while the explicit flag stands in for the prompt's *Flatten* button, so no layer is lost without a request when no user is present.
- 2026-11-05: No PNG compression argument — the editor always writes level 6 and offers no choice; the parity rule (Decision Log 2026-11-05) excludes capabilities no editor offers.
- 2026-11-05: `export.svg` refuses out-of-range options instead of clamping them like the dialog's inputs — an agent cannot see a clamped field, so a silently different value would make the file differ from the request.
- 2026-11-05: `doc.open` takes `mode` (`pixel` | `image`) with the same name and values as `doc.new` — one argument per concept across tools; without it routing is unchanged, so existing clients keep their behavior.
- 2026-11-05 (clarification pass): PSD is recognized content first, extension only as fallback, as the editor's `sniffFormat ?? formatFromName` does; `.aseprite` / `.ase` stay recognized by extension, as `doc.open` routes today. GIF warnings and PSD notices are English with stable `code`s / one string per notice, because MCP messages are English and a test needs fixed values; the UI's translated wording stays the UI's. `quality` outside 1–100 is refused, not clamped like the editor's input, and Image-only `doc.save` arguments on a Pixel document are refused — the same rule as `export.jpeg`'s schema and as `palette` on an Image `doc.new`: a value the call cannot use is an error. `export.svg` `fill` takes `#RRGGBB` only, because the trace's fill has no alpha and pixel-exact / colors layers take their colors from the image.
- 2026-11-05: `depends_on` lists `mcp-parity-structure` (PSD groups show in `layer.list` only after its AC-12), `psd-import` and `vectorize-dialog` (both built but not yet accepted; the tools mirror their behavior, so it must be final first). `vectorize-core` comes in through `vectorize-dialog`.
- 2026-11-05 (spec-build): The test `image_doc_save_without_path_after_flat_open_fails_and_writes_nothing` (mcp-headless-image AC-20, "doc.save writes OpenRaster only") is replaced by `image_doc_save_without_path_after_flat_open_refuses_a_lossy_flatten`, because AC-14 / AC-15 of this spec supersede that behavior.
- 2026-11-05 (spec-build): A flat `doc.save` makes the written file the document's save target (`source`) and drops a previous `.ora` path, so a later `doc.save` without `path` writes the last saved file in its format, as the editor's Save does.
- 2026-11-05 (spec-build): `export.svg` builds the dialog's flat option vector and traces through `TraceOptions::from_wire`, the path both wasm `Vectorizer`s take, so the file is the dialog's by construction; the tool checks the ranges first so an agent learns which value was refused. `pincel-agent` depends on `pincel-trace` (Decision Log 2026-11-01 names the agent tools among its users; no new external crate).
- 2026-11-05 (spec-build): The stdio test `image_document_open_edit_save_and_export_over_stdio` now expects `doc.save` without `path` to write an opened PNG back (AC-14 supersedes `mcp-headless-image` AC-20).
- 2026-11-06 (spec-build): A Pixel document opened from a PNG with `mode: "pixel"` keeps the PNG's path as `source_name` (not as its file), and `doc.save` without `path` suggests that path with its extension replaced by `.aseprite`, in the same directory (`shot.png` → `shot.aseprite`, `art/hero.v2.png` → `art/hero.v2.aseprite`) — the acceptance check found the message always suggested `sprite.aseprite` (AC-23); the test was red before the fix.
- 2026-11-06 (spec-build): `doc_save_of_a_psd_source_or_layered_document_suggests_ora_first` (AC-19) was green at once, because the behavior existed; it went red against a temporary break (the PSD clause removed from the `.ora`-first hint in `crates/pincel-agent/src/image/doc.rs`: "a PSD source, even one layer: … pass a path ending in .ora, …"), which was reverted.
- 2026-11-06 (spec-build): AC-19 with a `path`: `doc.save` to an extension it cannot write named `.ora` only for a layered document and only after the extension list, and not at all for a one-layer PSD source; it now uses the no-path wording ("pass a `.ora` path (keeps the layers) or a path ending in …") for a PSD source or several layers. `doc_save_of_a_psd_source_or_layered_document_suggests_ora_first` was red before the fix ("a PSD source, x.gif: doc.save cannot write `x.gif`: Image documents are saved as …").
- 2026-11-06 (spec-build): AC-26: the guide's mode table and tool lists now name `export.svg` (both modes), `export.apng` / `export.gif` (Pixel), `export.bmp` / `export.tiff` (Image), `.psd` and the flat formats `doc.save` writes, `doc.open`'s `mode` (`"image"` or none lets the file decide) and `export.svg`'s `frame`; `read_resource_guide_covers_workflow_and_both_modes` checks each with its arguments and was red against the guide before ("missing `export.png` and `export.svg`. Pixel-only").
- 2026-11-06 (spec-build): Tightened checks: the accepted range or values in each `export.svg` message, options the chosen mode ignores and wrong forms (AC-09; red with the schema's range wording changed: "colors must lie in 2–16"), the editor's reason for a PSB and a truncated PSD (AC-22), "tag \"jump\" not found", `scale: 0` and both sizes with "exceeds" for an oversized animation (AC-27, replacing `contains("16384") || contains("20000")`), the blend mode's name (AC-15) and the refused argument's name with the mode (AC-28). The AC-22 / AC-27 / AC-15 / AC-28 checks were green at once: they pin message text the code already produced.
- 2026-11-06 (spec-build, audit): A mistyped argument named only its type ("colors must be of type integer"), so `colors: 3.5`, `min_area: 2.5` or `mode: 5` did not state the accepted range AC-09 asks for; the schema's type error now appends the range or values and the description, for every tool (the shared validator). `bad_svg_argument_forms_name_the_argument_and_its_range` was red before the fix ("colors must be of type integer").
- 2026-11-06 (spec-build, audit): AC-09 "the first invalid argument": the message names exactly one argument with its range, the first one the check meets (schema order, then `fill`); the spec does not order several invalid arguments, so the test pins that only one is named.
- 2026-11-06 (spec-build, audit): The GIF median cut (`pincel-core`, frames with more than 256 colors) broke ties in the HashMap's random iteration order, so two exports of the same sprite wrote different bytes and `export.gif` could not equal the editor's export (AC-11). Ties now sort by the whole color. `median_cut_same_colors_give_the_same_palette_whatever_the_hash_order` and the "many colors" case of `export_gif_warns_about_each_case_on_its_own` were red before the fix.
- 2026-11-06 (spec-build, audit): An opened PSD is compared with the editor's import as JSON with the random document and layer ids replaced by stable names (each import mints fresh UUIDs), so every layer property, group parent and pixel is compared (AC-21).
- 2026-11-06 (spec-build, audit): AC-23 "the New Pixel document's palette" is checked as sprite equality with `doc.new` of the same size (layers, frames, palette, color mode), since `doc.new` builds the New dialog's document.
- 2026-11-06 (spec-build, audit): Tests green at once, each shown red against a temporary break that was reverted: `export_svg_each_option_and_default_is_the_dialogs` (default `min_area` 0), `export_svg_reports_the_dialogs_counts` (`layers` reporting the outline count), `export_svg_traces_the_active_or_given_frame_and_skips_hidden_layers` (frame 0 as default; hidden Image layers composed), `export_svg_defaults_to_pixel_exact_on_pixel_and_colors_on_image` (pixel_exact on Image), `export_svg_of_nothing_visible_has_no_layers` (`layers` at least 1), `pixel_exact_svg_counts_only_visible_colors_up_to_256` (limit 255 in `pincel-trace`), `export_gif_warns_about_each_case_on_its_own` (colors warning on semi-transparent frames), `animation_loops_follow_the_tag_or_the_argument` (tag repeats overriding `loops`), `animation_errors_state_the_editors_reason` (a generic message), `doc_save_writes_each_format_and_makes_the_path_the_documents_file` (a later save always PNG; case-sensitive extensions), `doc_save_writes_back_the_detected_format_whatever_the_extension` (format from the extension), `a_lossy_flatten_names_each_cause_and_flatten_writes_the_visible_composite` (only the first hidden layer named; one layer counted as "1 layers"; hidden layers in the composite), `flatten_true_without_loss_writes_the_same_file` (another PNG level with `flatten`), `jpeg_quality_applies_to_jpeg_only_and_per_document` (quality remembered on a PNG save), `doc_open_of_a_psd_equals_the_editors_document_and_becomes_active` (notices reversed; PSD flattened), `doc_open_recognizes_psd_by_content_first_then_by_name` (no `.psd` name fallback), `doc_open_of_an_unsupported_psd_states_the_editors_reason` (a generic message; a looser size limit), `doc_open_mode_pixel_equals_a_new_pixel_document_with_the_pixels` (the PNG as the document's file), `doc_open_mode_routes_by_name_then_content` (`.ase` routed to Image), `doc_open_with_an_unfitting_mode_names_the_mode_and_detected_format` (format dropped from the message), `doc_open_mode_pixel_of_a_corrupt_png_states_the_decoders_reason` (a generic message), `psd_notice_names_the_layer_or_blend_key_of_each_item` (blend key dropped), `read_only_refuses_every_save_format_and_keeps_opening` (read-only letting `.webp` through; a PSD open asking for a writable path), `every_path_argument_outside_the_workspace_is_refused` (an escaping `.tif` written; an escaping `.png` read), `bad_save_arguments_name_the_argument` (without the type-error fix).
- 2026-11-06 (acceptance rework): AC-17's "flat formats under the rules of AC-15 and AC-16" had no test with an explicit `path`: every lossy-flatten test saved back without `path`, and every `doc.save` with `path` used one opaque Normal layer. `doc_save_with_a_flat_path_refuses_a_lossy_flatten_and_flatten_writes_it` saves a three-layer document (one at 50 % opacity with Multiply, one hidden) to `.png`, `.jpg` and `.TIFF` paths: without `flatten` the call names every cause, suggests `flatten: true` and a `.ora` path and writes nothing; with `flatten: true` it writes the editor's encoding of the composite, keeps all layers, clears the dirty state and makes `path` the document's file. Green at once; red against a temporary loss check restricted to saves without `path` in `image/doc.rs` `save` ("doc.save unexpectedly succeeded"), reverted.
