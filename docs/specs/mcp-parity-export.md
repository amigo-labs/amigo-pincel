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
| AC-14 | `doc_save_without_path_writes_back_the_opened_format` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-15 | `a_lossy_flatten_needs_flatten_true_and_names_every_cause` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-16 | `a_lossy_flatten_needs_flatten_true_and_names_every_cause` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-17 | `doc_save_with_a_path_picks_the_format_by_extension` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-18 | `jpeg_quality_is_remembered_per_document` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-19 | `unwritable_formats_are_refused_listing_the_extensions` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-20 | `export_bmp_and_tiff_write_the_composite_and_leave_the_document` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-21 | `doc_open_of_a_psd_opens_the_editors_layers_and_notices` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-22 | `doc_open_of_a_bad_psd_names_the_file_and_reason` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-23 | `doc_open_mode_pixel_opens_a_png_as_the_pixel_editor_does` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-24 | `doc_open_mode_selects_or_refuses_the_mode` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-25 | `doc_open_mode_selects_or_refuses_the_mode` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-26 |      | open   |        |
| AC-27 |      | open   |        |
| AC-28 | `bad_save_arguments_name_the_argument` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |
| AC-29 | `doc_open_mode_pixel_of_a_corrupt_png_names_the_file` (`crates/pincel-agent/src/tests/export.rs`) | built  |        |

## Decisions

- 2026-11-05: `doc.save` writes an Image document back in its own format like the editor's Save, and a lossy flatten (more than one layer, a hidden layer, opacity below 100 % or a non-Normal blend) needs `flatten: true`, otherwise the call fails (AC-14 – AC-16); this supersedes `mcp-headless-image` AC-20 and its Decision "the server never flattens or overwrites on its own" — owner's choice, because agents should reach what the editor's Save does, while the explicit flag stands in for the prompt's *Flatten* button, so no layer is lost without a request when no user is present.
- 2026-11-05: No PNG compression argument — the editor always writes level 6 and offers no choice; the parity rule (Decision Log 2026-11-05) excludes capabilities no editor offers.
- 2026-11-05: `export.svg` refuses out-of-range options instead of clamping them like the dialog's inputs — an agent cannot see a clamped field, so a silently different value would make the file differ from the request.
- 2026-11-05: `doc.open` takes `mode` (`pixel` | `image`) with the same name and values as `doc.new` — one argument per concept across tools; without it routing is unchanged, so existing clients keep their behavior.
- 2026-11-05 (clarification pass): PSD is recognized content first, extension only as fallback, as the editor's `sniffFormat ?? formatFromName` does; `.aseprite` / `.ase` stay recognized by extension, as `doc.open` routes today. GIF warnings and PSD notices are English with stable `code`s / one string per notice, because MCP messages are English and a test needs fixed values; the UI's translated wording stays the UI's. `quality` outside 1–100 is refused, not clamped like the editor's input, and Image-only `doc.save` arguments on a Pixel document are refused — the same rule as `export.jpeg`'s schema and as `palette` on an Image `doc.new`: a value the call cannot use is an error. `export.svg` `fill` takes `#RRGGBB` only, because the trace's fill has no alpha and pixel-exact / colors layers take their colors from the image.
- 2026-11-05: `depends_on` lists `mcp-parity-structure` (PSD groups show in `layer.list` only after its AC-12), `psd-import` and `vectorize-dialog` (both built but not yet accepted; the tools mirror their behavior, so it must be final first). `vectorize-core` comes in through `vectorize-dialog`.
- 2026-11-05 (spec-build): The test `image_doc_save_without_path_after_flat_open_fails_and_writes_nothing` (mcp-headless-image AC-20, "doc.save writes OpenRaster only") is replaced by `image_doc_save_without_path_after_flat_open_refuses_a_lossy_flatten`, because AC-14 / AC-15 of this spec supersede that behavior.
- 2026-11-05 (spec-build): A flat `doc.save` makes the written file the document's save target (`source`) and drops a previous `.ora` path, so a later `doc.save` without `path` writes the last saved file in its format, as the editor's Save does.
