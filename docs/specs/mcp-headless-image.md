---
status: todo
created: 2026-10-28
size: L
mode: Image
design: [mcp.md, pincel.md §1, pincel.md §5.4, pincel.md §6, pincel.md §15 (2026-10-11 OpenRaster project format), pincel.md §15 (2026-10-12 effects honour the selection)]
depends_on: [mcp-headless-pixel]
---

# MCP server: headless Image documents

## Context

`mcp-headless-pixel` gives LLM agents a headless `pincel-mcp` server over stdio with Pixel-mode documents. Pincel's second core, Image mode (`pincel-image-core`: layers, brushes, shapes, text, selections, adjustments, effects), is just as useful to agents: crop and annotate a screenshot, build a mockup, export a web image. This spec adds Image-mode documents and tools to the same server, plus two resources that teach agents how to use Pincel: `pincel://guide` and `pincel://effects`.

Second of three specs migrated from the legacy plan `18-mcp` (removed 2026-10-29, see git history) (owner's top priority). The mode follows the document, as in the editor (pincel.md §1): `.aseprite` → Pixel; PNG / JPEG / WebP / BMP / GIF / TIFF / `.ora` → Image.

## Scope

### In scope

- `doc.new` with `mode: "image"`; `doc.open` of PNG, JPEG, WebP, BMP, GIF, TIFF and `.ora`
- The shared document tools of `mcp-headless-pixel` working on Image documents
- Image tools: `layer.*` (without groups), `draw.stroke`, `draw.shape`, `draw.text`, `draw.fill`, `draw.gradient`, `select.rect`, `select.ellipse`, `select.wand`, `select.clear`, `adjust.apply`, `effect.apply`, `canvas.resize`, `image.scale`, `image.crop_to_selection`, `image.flip`, `image.rotate`, `export.png`, `export.jpeg`, `export.webp`, `doc.save` (`.ora`)
- Resources `pincel://guide` (usage guide for agents, both modes) and `pincel://effects` (effect ids with parameter ranges, generated from the catalog)
- Clear errors when a tool does not fit the document's mode

### Out of scope

- Layer groups in Image documents — because Image mode has none until `image-layer-groups` is built; the tools follow the core
- Live control of the open desktop document — because `mcp-live-desktop` covers it
- `validate.run` — because the plan schedules validation as a later phase
- Hosted MCP, accounts, telemetry, curriculum resources, generative image models — because the plan excludes them for v1 (see `mcp-headless-pixel`)
- MCP prompts (`create-sprite`, `edit-screenshot`) — follow-up spec `mcp-prompts` once the tools are stable, because `pincel://guide` already covers both workflows and prompts add a third MCP capability to test with no new tool power

## Acceptance criteria

- [ ] AC-01 (event): When the client calls `doc.new` with `mode: "image"`, `width` and `height`, then the session holds a new Image document of that size, it becomes the active document, the result carries its `doc` id and info, and nothing is written to the workspace.
- [ ] AC-02 (event): When the client calls `doc.open` with a workspace-relative PNG, JPEG, WebP, BMP, GIF, TIFF or `.ora` path, then the session holds an Image document with the same layers and pixels as the editor opens from that file, and it becomes the active document; a `.aseprite` path still opens a Pixel document.
- [ ] AC-03 (always): On Image documents the shared tools behave as on Pixel documents: `doc.info` / `doc.list` report size, layers and dirty state, `doc.preview` returns PNG image content of the composed image, `doc.read_pixels` with `format: "hex"` returns the region's colors, and every successful mutating call is exactly one undo step for `doc.undo` / `doc.redo`.
- [ ] AC-04 (event): When `layer.add`, `layer.set` (name, visible, opacity, blend, locked) or `layer.remove` succeed on an Image document, then `layer.list` reports the changed layer stack in order with these properties.
- [ ] AC-05 (error): If a Pixel-only tool (`frame.*`, `tag.*`, `palette.*`, `draw.pixels`, `draw.line`, `draw.rect`, `draw.ellipse`, `draw.clear`, `selection.*`, `export.sheet`, `doc.read_pixels` with `format: "index"`, `layer.*` with `parent`) is called on an Image document, or an Image-only tool on a Pixel document, then the call fails with `isError: true`, the message names the document's mode, and the document is unchanged.
- [ ] AC-06 (event): When the client calls `draw.stroke` with `points`, size, hardness, opacity and color, then the target layer has the same pixels as after the editor's Brush tool along the same points with the same settings; with `tool: "eraser"`, the same pixels as after the Eraser tool.
- [ ] AC-07 (event): When the client calls `draw.shape` (rect, rounded, ellipse, polygon or line, with stroke and fill), then the target layer has the same pixels as after the editor's matching shape tool with the same geometry, stroke and fill.
- [ ] AC-08 (event): When the client calls `draw.text` with text, position, size, color and align, then the target layer has the same pixels as after the editor's Text tool with the same settings.
- [ ] AC-09 (event): When the client calls `draw.fill` or `draw.gradient`, then the target layer has the same pixels as after the editor's Bucket or Gradient tool with the same arguments.
- [ ] AC-10 (event): When the client calls `select.rect`, `select.ellipse` or `select.wand`, then the document's selection equals the one the editor's matching selection tool makes with the same arguments; `select.clear` removes it.
- [ ] AC-11 (state): While a selection exists, `draw.fill`, `adjust.apply` and `effect.apply` change no pixel outside it, and blend feathered edges by coverage as the editor does.
- [ ] AC-12 (event): When the client calls `adjust.apply` or `effect.apply` with an id and parameters from the `pincel-effects` catalog, then the target layer has the same pixels as after the editor applies it; an unknown id or a parameter outside its range fails with `isError: true` naming it, and the document is unchanged.
- [ ] AC-13 (event): When the client calls `canvas.resize`, `image.scale`, `image.crop_to_selection`, `image.flip` or `image.rotate`, then the document's size and pixels equal those after the editor's matching command, and one `doc.undo` restores size and pixels.
- [ ] AC-14 (error): If `image.crop_to_selection` is called while no selection exists, then the call fails with `isError: true` saying a selection is needed, and the document is unchanged.
- [ ] AC-15 (event): When the client calls `export.png`, `export.webp` or `export.jpeg` (with `quality`) with a workspace path, then a file of that format is written there that decodes to the composed image (pixel-equal for PNG and WebP lossless output, within the codec's loss for JPEG and lossy WebP).
- [ ] AC-16 (event): When the client calls `doc.save` with a `.ora` path on an Image document, then an OpenRaster file is written that reopens with the same layers, layer properties and pixels.
- [ ] AC-17 (event): When the client calls `resources/read` for `pincel://guide`, then it returns a text guide with a section each on the workflow (plan → draw → preview → export), the coordinate system, colors, undo, preview and common pitfalls, covering both modes.
- [ ] AC-18 (always): `pincel://effects` lists exactly the effect and adjustment ids of the `pincel-effects` catalog, each with its parameters and their ranges, so a change to the catalog changes the resource without a hand edit.
- [ ] AC-19 (error): If an Image file passed to `doc.open` is corrupt or of an unsupported format, then the call fails with `isError: true` naming the file, and no document is added.
- [ ] AC-20 (error): If `doc.save` without `path` is called on an Image document opened from a flat image (PNG, JPEG, WebP, BMP, GIF or TIFF), then the call fails with `isError: true` saying the document was opened from that format and asking for a `.ora` path to `doc.save`, or `export.png` for a flat image; nothing is written and the source file is not overwritten.
- [ ] AC-21 (error): If `doc.new`, `doc.open`, `canvas.resize` or `image.scale` would produce an Image document wider or taller than 16384 px, or larger than 8192 × 8192 px (64 Mpx) in area, then the call fails with `isError: true` naming the limit and the requested size, and no document is added or the document is unchanged.

## Technical notes

Hints from the legacy plan (2026-10-28). Paths drift — grep for the names.

**Prerequisite refactor (plan "phase 0", no spec of its own).** Move the Image `CommandSpec` enum and its dispatch `run_command` out of `crates/pincel-image-wasm/src/lib.rs` (`enum CommandSpec` ≈L468, `fn run_command` ≈L846) into a public module of `pincel-image-core`, e.g. `command::spec` (`CommandSpec` + `run_command`), so `pincel-agent` can call them natively. No new crate. The `ts_rs::TS` derives move behind an optional feature `ts` of `pincel-image-core` (optional dependency on the `ts-rs` 12 the workspace already uses); `pincel-image-wasm` enables that feature in its dev-dependency. ts-rs generation must keep working: `cargo test -p pincel-image-wasm export_command_spec` still writes `ui/src/modes/image/core/generated/`, and CI checks for drift. No behavior change; all wasm tests green; `refactor(...)` commits only.

**Existing commands to reuse** (`CommandSpec` variants today): `PencilStroke`, `EraserStroke`, `RecolorStroke`, `SmudgeStroke`, `FillBucket`, `DrawShape`, `DrawGradient`, `DrawText`, `AddLayer`, `RemoveLayer`, `MoveLayer`, `RenameLayer`, `SetLayerOpacity`, `SetLayerBlendMode`, `SetLayerVisible`, `SetLayerLocked`, `SelectRectangle`, `SelectEllipse`, `SelectWand`, `Deselect`, `FlipCanvas`, `RotateCanvas`, `ScaleImage`, `ResizeCanvas`, `CropToSelection`. Effects through `pincel_effects::EffectSpec`. Codecs: the `image` crate in `pincel-image-core` (png, jpeg, webp, bmp, gif, tiff) and the ORA codec.

**Tools (v1, Image).**

| Tool | Notes |
| ---- | ----- |
| `layer.*` | as in Pixel mode, minus groups |
| `draw.stroke` | brush: `points`, size, hardness, opacity, color; or `tool: eraser` |
| `draw.shape` | rect / rounded / ellipse / polygon / line, stroke and fill |
| `draw.text` | text, position, size, color, align |
| `draw.fill` / `draw.gradient` | |
| `select.rect` / `select.ellipse` / `select.wand` / `select.clear` | |
| `adjust.apply` / `effect.apply` | `pincel-effects` ids + params |
| `canvas.resize` / `image.scale` / `image.crop_to_selection` / `image.flip` / `image.rotate` | |
| `export.png` / `export.jpeg` (`quality`) / `export.webp` / `doc.save` (`.ora`) | |

**Text.** `pincel-image-core` rasterises text with `ab_glyph`; the caller supplies the font bytes and the app ships Liberation Sans (Decision Log 2026-09-06). The headless binary must embed a font.

**Size limits.** The editor caps Image documents at 16384 px per side and `MAX_CANVAS_AREA` = 8192 × 8192 (`ui/src/modes/image/stores/editor.svelte.ts`), a wasm32 memory limit. The headless server enforces the same limits on `doc.new`, `doc.open`, `canvas.resize` and `image.scale` (AC-21), although the native binary is not bound by wasm32, so every file it writes opens in the editor.

**Saving.** `doc.save` writes only `.ora`; a document opened from a flat image has no `.ora` file to save to, so `doc.save` without `path` fails (AC-20). `save-original-format` is an editor feature and does not apply headless.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `draw.stroke` with a single point | One dab at that point, like a click with the Brush |
| `draw.stroke` with an empty `points` list | `isError: true` naming `points`; document unchanged |
| `select.wand` on a uniform layer | The whole canvas is selected |
| `image.rotate` by 90° on a non-square image | Width and height swap; one undo restores both |
| `doc.open` of an animated GIF | Opens with the same layers and pixels as the editor opens it |
| `export.jpeg` of an image with transparency | Written as the editor's JPEG export writes it (alpha flattened the same way) |
| `layer.remove` of the only layer | Fails as the editor's Remove Layer does; document unchanged |
| `doc.open` of `shot.png`, then `doc.save` without `path` | Fails (AC-20) naming PNG and suggesting a `.ora` path or `export.png`; `shot.png` untouched |
| `doc.new` with 20000 × 100 | Fails (AC-21): side limit 16384 |
| `image.scale` of 8192 × 8192 to 200 % | Fails (AC-21): area limit 64 Mpx; document unchanged |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| `draw.text` uses the same bundled font as the editor (Liberation Sans) | Same output as the editor; no font file needed in the workspace | Low |
| The draw tools act on the same target layer as the Pixel tools (optional `layer`, else the active layer — `mcp-headless-pixel` AC-43) | One convention for both modes | Low |
| The shape, text and stroke tools reuse the editor's commands, so "same pixels as the editor" is the correctness reference | The plan: one tool implementation, no re-implementation | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `image_doc_new_creates_active_image_document_and_writes_nothing` (`crates/pincel-agent/src/tests/image.rs`) | built |        |
| AC-02 | `image_doc_open_matches_what_the_editor_opens` (`crates/pincel-agent/src/tests/image.rs`) | built |        |
| AC-03 | `image_shared_tools_report_preview_read_and_undo` (`crates/pincel-agent/src/tests/image.rs`); the `one_undo_step` checks in `crates/pincel-agent/src/tests/image_draw.rs` | built |        |
| AC-04 | `image_layer_tools_change_and_list_the_stack`, `image_layer_remove_of_only_layer_fails_like_the_editor` (`crates/pincel-agent/src/tests/image.rs`) | built |        |
| AC-05 | `tools_of_the_other_mode_fail_naming_the_mode_and_change_nothing` (`crates/pincel-agent/src/tests/image.rs`); `image_tools_refuse_locked_layers_and_pixel_documents` (`crates/pincel-agent/src/tests/image_draw.rs`) | built |        |
| AC-06 | `draw_stroke_matches_brush_and_eraser`, `draw_stroke_with_empty_points_fails_naming_points` (`crates/pincel-agent/src/tests/image_draw.rs`) | built |        |
| AC-07 | `draw_shape_matches_the_shape_tools` (`crates/pincel-agent/src/tests/image_draw.rs`) | built |        |
| AC-08 | `draw_text_matches_the_text_tool` (`crates/pincel-agent/src/tests/image_draw.rs`) | built |        |
| AC-09 | `draw_fill_and_gradient_match_bucket_and_gradient_tools` (`crates/pincel-agent/src/tests/image_draw.rs`) | built |        |
| AC-10 | `select_tools_match_the_selection_tools_and_clear_removes_it` (`crates/pincel-agent/src/tests/image_draw.rs`) | built |        |
| AC-11 | `selection_limits_fill_adjust_and_effect_with_feathered_edges` (`crates/pincel-agent/src/tests/image_draw.rs`) | built |        |
| AC-12 | `adjust_and_effect_apply_match_the_editor`, `adjust_and_effect_apply_refuse_unknown_ids_and_bad_params` (`crates/pincel-agent/src/tests/image_draw.rs`) | built |        |
| AC-13 | `canvas_and_image_transforms_match_the_editor_and_undo_in_one_step` (`crates/pincel-agent/src/tests/image_draw.rs`) | built |        |
| AC-14 | `crop_to_selection_without_selection_fails` (`crates/pincel-agent/src/tests/image_draw.rs`) | built |        |
| AC-15 | `image_exports_decode_to_the_composed_image` (`crates/pincel-agent/src/tests/image.rs`) | built |        |
| AC-16 | `image_doc_save_writes_ora_that_reopens_identically` (`crates/pincel-agent/src/tests/image.rs`) | built |        |
| AC-17 | `read_resource_guide_covers_workflow_and_both_modes`, `list_resources_has_guide_and_effects` (`crates/pincel-agent/src/resources.rs`); `resources_read_guide_and_effects_with_their_mime_types` (`crates/pincel-mcp/src/server.rs`); `image_document_open_edit_save_and_export_over_stdio` (`crates/pincel-mcp/tests/stdio.rs`) | built |        |
| AC-18 | `effects_resource_lists_exactly_the_catalog_with_ranges` (`crates/pincel-agent/src/resources.rs`); `catalog_covers_every_effect_in_menu_order` (`crates/pincel-agent/src/pixel/effects.rs`) | built |        |
| AC-19 | `image_doc_open_corrupt_or_unsupported_file_fails_naming_it` (`crates/pincel-agent/src/tests/image.rs`) | built |        |
| AC-20 | `image_doc_save_without_path_after_flat_open_fails_and_writes_nothing` (`crates/pincel-agent/src/tests/image.rs`) | built |        |
| AC-21 | `image_size_limits_refuse_new_and_open` (`crates/pincel-agent/src/tests/image.rs`); `resize_and_scale_refuse_sizes_above_the_limits` (`crates/pincel-agent/src/tests/image_draw.rs`) | built |        |

## Decisions

- Image tools are implemented in the platform-free `pincel-agent` crate over `pincel-image-core`, never in the editor — the same code later serves live mode (plan, 2026-10-28).
- `pincel://effects` is generated from the `pincel-effects` catalog, not hand-written — it cannot drift from the effects the tools accept (plan).
- `layer.*` on Image documents has no groups until `image-layer-groups` is built — the tools expose what the core supports (plan).
- 2026-10-29: The Image `CommandSpec` and `run_command` move into `pincel-image-core` as a public module (`command::spec`), with the `ts_rs::TS` derives behind an optional feature `ts` that `pincel-image-wasm` enables in its dev-dependency; no new crate — the core already depends on serde, so this is the smallest move both `pincel-agent` and the wasm crate can call; §15 row recorded with the 2026-10-29 Decision Log batch (owner accepted the recommendation).
- 2026-10-29: `doc.save` without `path` on a document opened from a flat image fails and asks for a `.ora` path or `export.png`; the server never flattens or overwrites on its own (AC-20) — a silent flattening overwrite loses layers with no user present, and it matches the "ask for a path" pattern of `mcp-headless-pixel` AC-31 (owner accepted the recommendation).
- 2026-10-29: The headless server enforces the editor's Image limits (side ≤ 16384, area ≤ 64 Mpx) on `doc.new`, `doc.open`, `canvas.resize` and `image.scale` (AC-21) — files written headlessly must open in the editor, whose wasm build has these limits (owner accepted the recommendation).
- 2026-10-29: The MCP prompts `create-sprite` / `edit-screenshot` are not part of v1; they become the follow-up spec `mcp-prompts` — the plan marks them optional, `pincel://guide` covers both workflows, and prompts add a capability to test with no new tool power (owner accepted the recommendation).
- 2026-10-29 (build): the phase-0 move puts `CommandSpec`, `run_command`, `StrokeScratch`, `layer_locked` and `parse_sample` into the public module `pincel_image_core::command::spec` (new public API); `run_command` takes the stroke scratch and the font faces as arguments instead of wasm thread-locals, and `pincel-image-wasm` keeps them per handle — the generated `CommandSpec.ts` is unchanged.
- 2026-10-29 (build): Image tools build the editor's `CommandSpec` and run it through `run_command` (strokes with a fresh `stroke_id` per call, so two calls never merge); `effect.apply` / `adjust.apply` repeat the wasm `apply_effect` (effect on the layer, `masked_region` by coverage, one `SetPixels`) because `pincel-image-core` does not depend on `pincel-effects`. Several layer property changes in one call are one agent-local `Batch` command, so `layer.add` / `layer.set` stay one undo step.
- 2026-10-29 (build): Image layers get stable integer ids (assigned per core layer UUID, never reused, kept across undo) so `layer` arguments use the same integer form as Pixel documents; `layer.list` reports opacity on the Pixel 0–255 scale and only the 12 blend modes Image mode has. `layer.add` adds on top of the stack and makes the layer active, as in Pixel mode.
- 2026-10-29 (build): Image selections are undo steps (as in the editor's Image mode), unlike the Pixel `selection.*` tools; select tools live under `select.*`, Pixel ones keep `selection.*`.
- 2026-10-29 (build): Image tool parameters follow the editor's option bar: brush / fill / gradient `opacity` and brush `hardness` in % (0–100), defaults from the editor's tool store (brush 8 px, tolerance 32, stroke width 3, corner radius 16, 5 polygon sides, text 48 px, anti-aliasing on, JPEG quality 90); polygon `rotation` is in degrees (the core takes radians); `canvas.resize` anchors top_left and `image.scale` resamples bilinear by default, as the core commands do.
- 2026-10-29 (build): `adjust.apply` takes the ids of the editor's Adjust menu (`group: 'adjust'` in `catalog.ts`) on both modes; `effect.apply` keeps taking every catalog id, so Pixel behavior is unchanged.
- 2026-10-29 (build): `draw.fill` gains `tolerance` and `opacity` for Image documents; on Pixel documents they are refused, because the Pixel Bucket matches the seed color exactly.
- 2026-10-29 (build): `doc.new` checks sizes in code instead of the schema (Pixel 1–4096 with the old message, Image 1–16384 plus the 64 Mpx area), so the AC-21 message can name both the limit and the requested size.
- 2026-10-29 (build): `doc.open` routes by extension (`.aseprite` / `.ase` → Pixel) and otherwise opens an Image document — `.ora` (or a ZIP signature) as a project, anything else through the core decoder; the flat format named in the AC-20 message comes from the file signature. Image `doc.preview` keeps the Pixel rules (no downscale, 2048 px cap, `region` for larger images).
- 2026-10-29 (build): the Text tool's font is embedded in `pincel-agent` with `include_bytes!` from `ui/public/fonts/LiberationSans-Regular.ttf` — one copy of the font in the repository; `cargo install --path` builds in place, so the path resolves.
- 2026-10-29 (build): `pincel-agent` gains `pincel-image-core` as a dependency and the workspace `image` crate as a dev-dependency (GIF / TIFF / WebP fixtures for AC-02); both are already in the lockfile, so no new external dependency.
- 2026-10-29 (build): `pincel://guide` is Markdown (`text/markdown`) kept in `crates/pincel-agent/src/guide.md`; its test checks the required sections and that every tool it names exists. `pincel://effects` is JSON generated from the same catalog table `effect.apply` validates against (ids in menu order, `kind` effect / adjustment, each parameter with type, range or options and default); that table is pinned to `pincel_effects::EFFECT_NAMES` by `catalog_covers_every_effect_in_menu_order`. `resources/read` now answers with the MIME type `resources/list` gives.
- 2026-10-29 (build): the Image document and shared-tool code was written before its tests in this run; the only red run recorded for that block is `doc.info` refusing Image documents. The editing tools' tests ran red first (unknown tools).
