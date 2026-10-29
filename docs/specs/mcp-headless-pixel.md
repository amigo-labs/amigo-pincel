---
status: todo
created: 2026-10-28
size: L
mode: Pixel
design: [mcp.md, cli.md, pincel.md §1.2, pincel.md §3, pincel.md §5.2, pincel.md §6, pincel.md §7.3, pincel.md §9.3]
depends_on: []
---

# MCP server: headless Pixel documents

## Context

LLM agents (Claude Code, Claude Desktop, any MCP client) should create and edit Pincel documents through the Model Context Protocol. The owner considers this the most important missing feature: nothing in pincel.md §18 offers it (Aseprite has Lua scripting but no MCP, Photopea a script API but no agent protocol). It makes Pincel the pixel-art backend for coding agents — "add four enemy variants to `assets/`" in a game repo, Pincel's core audience (§1.2, engine interop).

This spec is the first of three migrated from the legacy plan `18-mcp` (removed 2026-10-29, see git history):

1. **this spec** — a headless `pincel-mcp` binary over stdio with the shared document tools and the Pixel-mode tools;
2. `mcp-headless-image` — the same server gains Image-mode documents and tools, plus the agent guide and effect catalog resources;
3. `mcp-live-desktop` — the desktop app exposes the same tools against the document open on screen.

From the design spec `docs/specs/mcp.md` (draft, 2026-05-13) this keeps the agent loop (plan → draw → preview → validate → export), the preview as the critical tool (the model sees what it draws), and the workspace sandbox. `mcp.md` is out of date on crate names, code location, the indexed-only model, the curriculum and `.ait` export; the plan replaces those points (see Technical notes).

## Scope

### In scope

- A `pincel-mcp` binary speaking MCP over stdio: `pincel-mcp [--workspace <dir>] [--read-only]`
- A workspace sandbox for every path argument
- An in-memory session holding several documents, each with an id; one of them is the active document
- Shared document tools for Pixel documents: `doc.new` (`mode: "pixel"`), `doc.open` (`.aseprite`), `doc.list`, `doc.info`, `doc.close`, `doc.preview`, `doc.read_pixels`, `doc.undo`, `doc.redo`, `doc.save`, `export.png`
- Pixel tools: `layer.*`, `frame.*`, `tag.*`, `palette.*`, `draw.*`, `selection.*`, `effect.apply`, `export.sheet`
- The resource `pincel://palettes/{id}` for the built-in palette presets
- Structured tool errors the model can act on
- Distribution: `cargo install --path crates/pincel-mcp`, release binaries through the existing release workflow, and a README setup section

### Out of scope

- Image-mode documents (`doc.new` with `mode: "image"`, opening PNG / JPEG / WebP / BMP / GIF / TIFF / `.ora`) and the Image tools — because `mcp-headless-image` adds them on top of this server
- The `pincel://guide` and `pincel://effects` resources — because the plan ships them with the Image tools, so the guide covers both modes (`mcp-headless-image`)
- Live control of the document open in the desktop app — because `mcp-live-desktop` covers it
- `validate.run` and the validation rules (`palette_adherence`, `color_count`, `has_outline`, `seamless_edges`, `banding`) — because the plan schedules validation as a later phase after the v1 tools
- Hosted or remote MCP (`mcp.pincel.app`), user accounts and telemetry — because v1 is a local stdio tool and the plan excludes them
- Curriculum and lesson resources (`pincel://lessons/*`) — because the curriculum does not exist (`lessons.md` is future vision) and is not a prerequisite
- Generative image models — because the agent draws with Pincel's tools; it does not generate rasters
- `.ait` export — because it is the Amigo Engine runtime format, which is not in this repository
- The npm wrapper `@amigo-labs/pincel-mcp` — because the plan defers it until after cargo and release-binary distribution
- HTTP transport for the headless binary — because stdio covers local agents; the plan revisits it only when HTTP is needed

## Acceptance criteria

- [ ] AC-01 (event): When an MCP client starts `pincel-mcp` and sends `initialize` over stdio, then the server replies with its name and version and announces the tools and resources capabilities, and afterwards answers `tools/list`, `tools/call`, `resources/list`, `resources/read` and `ping`.
- [ ] AC-02 (always): Every tool returned by `tools/list` has a unique `area.verb` name, a non-empty description and an input schema of type `object` whose `required` entries all appear in its `properties`; the list contains every shared and Pixel tool listed under Technical notes.
- [ ] AC-03 (event): When the client calls `doc.new` with `mode: "pixel"`, `width` and `height`, then the session holds a new Pixel document of that size, it becomes the active document, the result carries its `doc` id and info, and nothing is written to the workspace.
- [ ] AC-04 (option): Where `doc.new` gets a `palette` preset id, the new document's palette equals that built-in preset; without it, the palette equals the default of a New Pixel document in the editor (DB32).
- [ ] AC-05 (always): A tool called without `doc` acts on the active document; a tool called with `doc` acts on that document and leaves every other open document unchanged.
- [ ] AC-06 (error): If `doc` names no open document, or a document tool is called while no document is open, then the call fails with `isError: true` and a message naming the missing id (or saying that no document is open), and no document changes.
- [ ] AC-07 (event): When the client calls `doc.list` or `doc.info`, then each document's width, height, layers, frames, tags, palette size, active layer, active frame and dirty state are reported; the dirty state is true after a successful mutating call and false after `doc.save`.
- [ ] AC-08 (event): When the client calls `doc.close`, then the document is no longer listed by `doc.list`, and a later call naming its id fails as in AC-06.
- [ ] AC-09 (always): Every successful mutating tool call that changes the document is exactly one undo step: one `doc.undo` returns the document to its exact state before the call, and one `doc.redo` re-applies the whole call.
- [ ] AC-10 (event): When the client calls `doc.undo` or `doc.redo`, then the result reports the new undo and redo depths.
- [ ] AC-11 (event): When the client calls `doc.preview`, then the result contains MCP image content holding a PNG of the composed frame in which every document pixel is an s×s block of one color — s being `scale`, or, without `scale`, the smallest integer that makes the longest side at least 256 px (s = 1 when the longest side already is), never below 1 — and the result states the PNG's width and height. The PNG is at most 2048 px on each edge (AC-45).
- [ ] AC-12 (option): Where `doc.preview` gets `frame` or `region`, the preview shows that frame, or only that rectangle of the frame; where it gets `grid: true` and the scale is at least 4, it differs from the same preview without grid only in a 1-px line of `#808080` at 50 % alpha on the first row and the first column of every document-pixel block; where the scale is below 4, no grid is drawn and the result says "grid omitted below scale 4".
- [ ] AC-13 (event): When the client calls `doc.read_pixels` with a region of at most 64×64 and `format: "hex"`, then the result has one text row per pixel row of the region with one color per pixel, equal to the pixels of the given layer's cel in the given frame, or of the composed frame when no layer is given.
- [ ] AC-14 (error): If `doc.read_pixels` gets a region larger than 64×64 in either dimension, or a region outside the canvas, then the call fails with `isError: true` and a message stating the limit or the canvas size.
- [ ] AC-15 (event): When `layer.add`, `layer.set` (name, visible, opacity, blend, locked) or `layer.remove` succeed, then `layer.list` reports the changed layer stack in order with these properties; a layer added or set with `parent` is a child of that group.
- [ ] AC-16 (event): When `frame.add` (with optional `clone_from` and `duration_ms`), `frame.set_duration` or `frame.remove` succeed, then `frame.list` reports the changed frames and durations, and a frame added with `clone_from` has the same cel pixels as its source frame.
- [ ] AC-17 (event): When `tag.add` (name, from, to, direction) or `tag.remove` succeed, then `tag.list` reports the changed tags with these properties.
- [ ] AC-18 (event): When the client calls `palette.get`, `palette.set` (entries) or `palette.load_preset` (DB32, DB16, PICO-8, Game Boy, NES), then `palette.get` afterwards returns the set entries, or exactly the preset's colors.
- [ ] AC-19 (event): When the client calls `draw.pixels` with a batch of `{x, y, color}`, then each listed pixel of the target cel has its color afterwards and no other pixel changed; colors are accepted as `#RRGGBB`, `#RRGGBBAA` and `{ "index": n }`.
- [ ] AC-20 (error): If any pixel of a `draw.pixels` batch lies outside the canvas, then the call fails with `isError: true`, the message names the first offending pixel and the canvas size (e.g. "pixel (40, 3) outside 32×32 canvas"), and the document is unchanged.
- [ ] AC-21 (error): If a color argument is malformed, or a palette index is not in the palette, then the call fails with `isError: true`, the message names the argument and the accepted forms, and the document is unchanged.
- [ ] AC-22 (event): When the client calls `draw.line`, `draw.rect` or `draw.ellipse` (with `filled`), then the target cel has the same pixels as after the editor's Line, Rectangle or Ellipse tool with the same endpoints, color and fill.
- [ ] AC-23 (event): When the client calls `draw.fill`, then the target cel has the same pixels as after the editor's Bucket tool from the same seed with the same color; with `contiguous: false`, every pixel of the seed's color changes.
- [ ] AC-24 (event): When the client calls `draw.clear` with a `region`, then exactly the pixels of the target cel inside the region become transparent; without `region`, every pixel of the target cel does.
- [ ] AC-25 (state): While a selection set by `selection.set_rect` exists, `draw.fill` and `effect.apply` change no pixel outside it; after `selection.clear` they act on the whole cel again.
- [ ] AC-26 (event): When the client calls `effect.apply` with an effect id and parameters from the `pincel-effects` catalog, then the target cel has the same pixels as after applying that effect with these parameters in the editor.
- [ ] AC-27 (error): If `effect.apply` names an unknown effect id, or a parameter outside its range, then the call fails with `isError: true`, the message names the id or the parameter and its allowed range, and the document is unchanged.
- [ ] AC-28 (error): If a mutating tool targets a locked layer (or a layer inside a locked group), then the call fails with `isError: true` naming the layer, and the document is unchanged.
- [ ] AC-29 (event): When the client calls `doc.open` with a workspace-relative `.aseprite` path, then the session holds a Pixel document with the same layers, frames, tags, palette and pixels as the editor opens from that file, it becomes the active document, and the result carries its `doc` id and info.
- [ ] AC-30 (event): When the client calls `doc.save`, then a `.aseprite` file is written to `path`, or without `path` to the file the document was opened from or last saved to, and opening that file again yields the same layers, frames, tags, palette and pixels.
- [ ] AC-31 (error): If `doc.save` without `path` is called on a document that was never opened from or saved to a file, then the call fails with `isError: true` asking for a `path`, and nothing is written.
- [ ] AC-32 (event): When the client calls `export.png` with `path` (and optional `frame`, `scale`), then a PNG file is written there whose pixels equal the `doc.preview` of the same frame at the same scale without grid.
- [ ] AC-33 (event): When the client calls `export.sheet` with `columns` and `path` (and optional `tag`, `scale`), then a PNG is written with the frames — only the tag's frames when `tag` is given — laid out left to right, top to bottom in that many columns; where `json` is given, a JSON file in the Aseprite sheet-data format naming the PNG and listing one rectangle per exported frame is written as well.
- [ ] AC-34 (error): If a path argument resolves outside the workspace (through `..`, an absolute path, or a symlink leading out), then the call fails with `isError: true` and `E007: path outside workspace`, and nothing is read or written.
- [ ] AC-35 (option): Where `--workspace <dir>` is given, paths resolve inside that directory; without it, inside the current working directory.
- [ ] AC-36 (option): Where `--read-only` is given, `doc.save`, `export.png` and `export.sheet` fail with `isError: true` saying the workspace is read-only and write nothing, while `doc.open` and all in-memory tools keep working.
- [ ] AC-37 (error): If `tools/call` names an unknown tool, or its arguments do not match the tool's input schema, then the server answers with an error naming the tool or the offending argument, and keeps serving later requests.
- [ ] AC-38 (error): If a message on stdin is not valid JSON-RPC, then the server answers with a JSON-RPC parse error and keeps serving later requests.
- [ ] AC-39 (event): When the client calls `resources/list`, then it contains one `pincel://palettes/{id}` resource per built-in palette preset, and `resources/read` of one returns that preset's colors.
- [ ] AC-40 (event): When a client spawns `pincel-mcp` in an empty temporary workspace and runs `initialize` → `tools/list` → `doc.new` → `draw.pixels` → `doc.preview` → `export.png`, then the preview result contains image content, and the exported PNG exists inside the workspace and contains the drawn pixels.
- [ ] AC-41 (event, manual: only a published release runs the release workflow): When a release is published, its assets include a `pincel-mcp` binary for each desktop platform the release workflow builds.
- [ ] AC-42 (always, manual: documentation): The README documents installing `pincel-mcp` and registering it with Claude Code via `claude mcp add pincel -- pincel-mcp --workspace .`.
- [ ] AC-43 (always): `draw.*`, `selection.*` and `effect.apply` take optional `layer` (an id from `layer.list`) and `frame` (an index) arguments; without them they act on the document's active layer and frame — the active layer being the active image layer or, if none is active, the lowest image layer, as in the editor. `layer.add` and `frame.add` make the new layer or frame active; explicit `layer` / `frame` arguments do not change which layer or frame is active.
- [ ] AC-44 (event): When the client calls `doc.read_pixels` with `format: "index"`, then each pixel is written as the decimal index of the first palette entry with exactly that RGBA value, or as `-` when no entry matches (fully transparent pixels included when no palette entry is transparent).
- [ ] AC-45 (error): If `doc.preview` at the given `scale` (or at scale 1) would produce a PNG wider or taller than 2048 px, then the call fails with `isError: true`, the message names the 2048 px cap and suggests `region`, and no image is returned.
- [ ] AC-46 (error): If `doc.new` gets a `width` or `height` outside 1–4096, then the call fails with `isError: true` and the message "width must be 1–4096" (or "height must be 1–4096"), and no document is added.

## Technical notes

Hints from the legacy plan (2026-10-28). Paths and line numbers drift — grep for the names.

**Prerequisite refactor (plan "phase 0", no spec of its own).** Much document logic sits behind `#[wasm_bindgen]` types today and a native binary cannot call it:

- `crates/pincel-wasm/src/lib.rs`: `edit_target_layer` (≈L1174), `ensure_paint_cel` (≈L1195), `apply_tool` / `applyTool` (≈L552), `apply_line` / `applyLine` (≈L587), compose with onion skin, palette import.
- Move the Pixel `Document` operations into plain Rust (a plain module, a `pincel-core::session` façade, or the `pincel-agent` crate itself); `pincel-wasm` keeps thin `#[wasm_bindgen]` wrappers (CLAUDE.md §5.5).
- Done when all existing wasm tests stay green with no behavior change, as pure `refactor(...)` commits. The Image counterpart (`CommandSpec` dispatch in `pincel-image-wasm`) is the prerequisite of `mcp-headless-image`.

**Docs prerequisite.** A status note at the top of `docs/specs/mcp.md` points to the MCP feature specs; the Decision Log entries for the protocol loop and the agent architecture are recorded (see Decisions).

**Protocol.** JSON-RPC 2.0 through an in-house loop, newline-delimited over stdio, `serde_json` only (no `rmcp`, no tokio in `pincel-mcp`). The server pins one MCP protocol revision (`2025-06-18` at the time of writing; confirm the current one when implementing) and answers `initialize` with it. The dispatch stays transport-agnostic so `mcp-live-desktop` reuses it over HTTP.

**Schemas.** Each tool's input schema is hand-written as a `serde_json::json!` next to the tool (no `schemars`). The test `tool_schemas_are_valid` checks that every schema parses, that every `required` entry is in `properties` (AC-02), and that one example call per tool validates against its schema.

**Architecture (plan).** One tool layer, two hosts:

```text
crates/pincel-agent/   NEW, platform-free (no tokio, no wasm-bindgen, no file I/O; CLAUDE.md §5.1)
  ToolRegistry: list_tools() -> Vec<ToolDef{name, description, input_schema}>
                call(&mut Workspace, name, args: serde_json::Value) -> Result<ToolOutput, AgentError>
  src/pixel/*.rs   tools over pincel-core
  src/image/*.rs   tools over pincel-image-core   (mcp-headless-image)
  src/preview.rs   compose → nearest-neighbour upscale → PNG bytes
  ToolOutput { json: serde_json::Value, image_png: Option<Vec<u8>> }
crates/pincel-mcp/     NEW binary: MCP over stdio, workspace sandbox, file read/write
```

The host owns file I/O and passes bytes in and out. Session: `Workspace { docs: HashMap<DocId, Doc>, active }`, each `Doc` a Pixel `Sprite` + undo bus; in memory only until `doc.save` / `export.*`.

**Undo.** Seal the bus before and after each call, as `run_sealed` does in `crates/pincel-wasm/src/transforms.rs` (≈L112).

**Tools (v1, Pixel + shared).**

| Tool | Args | Returns |
| ---- | ---- | ------- |
| `doc.new` | `mode: pixel`, `width`, `height`, `palette?` (preset id) | `doc` id, info |
| `doc.open` | `path` (workspace-relative `.aseprite`) | `doc`, info |
| `doc.list` / `doc.info` / `doc.close` | — | sizes, layers, frames, tags, palette size, active layer and frame, dirty |
| `doc.preview` | `frame?`, `scale?`, `grid?: bool`, `region?` | image content (PNG) + size |
| `doc.read_pixels` | `region` (≤ 64×64), `layer?`, `frame?`, `format: hex\|index` | rows of colors as text |
| `doc.undo` / `doc.redo` | — | new undo / redo depth |
| `doc.save` | `path?` | path |
| `export.png` | `frame?`, `scale?`, `path` | path |
| `layer.add` / `layer.remove` / `layer.list` / `layer.set` | `set`: name, visible, opacity, blend, locked; groups via `parent?` | |
| `frame.add` (`clone_from?`, `duration_ms?`) / `frame.remove` / `frame.set_duration` / `frame.list` | | |
| `tag.add` / `tag.remove` / `tag.list` | name, from, to, direction | |
| `palette.get` / `palette.set` (entries) / `palette.load_preset` | | |
| `draw.pixels` | batch `[{x, y, color}]`, `layer?`, `frame?` | |
| `draw.line` / `draw.rect` / `draw.ellipse` | `filled?`, `layer?`, `frame?` | |
| `draw.fill` | `contiguous?`, `layer?`, `frame?` | |
| `draw.clear` | `region?`, `layer?`, `frame?` | |
| `selection.set_rect` / `selection.clear` | `layer?`, `frame?` | |
| `effect.apply` | `id` + `params` (`pincel-effects` catalog), `layer?`, `frame?` | |
| `export.sheet` | `columns`, `tag?`, `scale?`, `path`, `json?` | |

Every tool takes an optional `doc`. `draw.*`, `selection.*` and `effect.apply` also take `layer?` (id from `layer.list`) and `frame?` (index); the defaults reuse the editor's paint target (`paint_target_layer_id` in `crates/pincel-wasm`, AC-43). `doc.new` bounds follow `NEW_DOC_LIMITS.pixel.max` (4096) in `ui/src/lib/shell/new-doc.ts` (AC-46).

**Existing code to reuse.** Sheet export: `export_atlas_png` (`crates/pincel-core/src/codec/png.rs`), `aseprite_sheet_json` (`codec/sheet_data.rs`). PNG: `export_frame_png`. Palette presets: `PalettePreset { Db32, Db16, Pico8, GameBoy, Nes }` (Decision Log 2026-10-12). Effects: `pincel_effects::EffectSpec` (`crates/pincel-effects/src/spec.rs`). Selection: `pincel_core::SelectionMask`. Locked layers: `Sprite::layer_locked` (Decision Log 2026-10-12).

**Errors.** Structured MCP tool errors (`isError: true`) with an actionable message; error codes as in `mcp.md` (`E007 path outside workspace`). CLI conventions in `docs/specs/cli.md`.

**`mcp.md` points the plan replaces.**

| `mcp.md` says | Now |
| ------------- | --- |
| Crate `core`, path `packages/lessons-mcp/` | `pincel-core` (Pixel), `pincel-image-core` (Image), `pincel-effects` (both) |
| Indexed bitmaps only | Pixel mode is RGBA with optional indexed; Image mode exists |
| Curriculum, `pincel://lessons/*` | Does not exist; not a prerequisite |
| `validate.*` with banding / hue-shift | No validation code yet; later phase |
| `.ait` export | Not in this repo; out of scope |

**Testing hints.** Unit tests per tool in `pincel-agent` (args validation, one undo step per call, error messages), e.g. `draw_pixels_batch_is_one_undo_step`, `preview_scales_nearest_and_returns_png`, `read_pixels_hex_grid_matches_cel`, `tool_schemas_are_valid`. An integration test spawns the binary and speaks JSON-RPC over stdin/stdout (AC-40); a sandbox test rejects `../x.png`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `draw.pixels` with an empty batch | Succeeds; the document is unchanged and no undo step is recorded |
| `draw.pixels` batch with one pixel outside the canvas | Whole call rejected (AC-20), document unchanged |
| `draw.line` / `draw.rect` / `draw.ellipse` partly outside the canvas | Clipped to the canvas like the editor's tools (Assumption) |
| `doc.undo` with an empty undo history | Document unchanged, depths reported unchanged (Assumption) |
| Path `../x.png`, `/etc/passwd`, or a workspace symlink pointing outside | `E007: path outside workspace`, nothing read or written |
| `doc.open` of a corrupt or non-`.aseprite` file | `isError: true` naming the file; no document added |
| `doc.preview` of a 1×1 sprite without `scale` | Scale 256, a 256×256 PNG |
| `doc.preview` with `grid: true` at scale 1–3 | No grid drawn; the result says "grid omitted below scale 4" (AC-12) |
| `doc.preview` of a 4096×4096 sprite without `region` | Fails: scale 1 already exceeds the 2048 px cap; the message suggests `region` (AC-45) |
| `doc.read_pixels` `format: "index"` on a color not in the palette | `-` for that pixel (AC-44) |
| `doc.new` with `width: 5000` | Fails with "width must be 1–4096"; no document added (AC-46) |
| `doc.read_pixels` region exactly 64×64 | Allowed; 65 in either dimension fails (AC-14) |
| `doc.close` on a dirty document | Closed without saving (Assumption) |
| Two `doc.new` calls | Two documents with distinct ids; the second becomes active |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| A `draw.pixels` batch is atomic: one bad pixel rejects the whole call | One call is one undo step; the plan's example error names a single pixel | Low |
| `draw.line` / `rect` / `ellipse` clip to the canvas instead of failing | They reuse the editor's tools, which clip | Low |
| `doc.read_pixels` `hex` writes `#RRGGBBAA` per pixel; without `layer` it reads the composed frame | Alpha must survive the text form; the plan's test name `read_pixels_hex_grid_matches_cel` covers the per-layer case | Medium |
| `doc.preview` without `frame` shows the first frame | The plan gives no default | Low |
| `doc.undo` / `doc.redo` with nothing to undo or redo succeed as a no-op | The plan gives no error for it | Low |
| `doc.close` discards unsaved changes; `doc.info` reports `dirty` so the agent can save first | Headless sessions have no user to ask | Medium |
| Layers, frames and tags are referenced by the ids or indices their `*.list` tool returns | The plan does not define references | Low |
| Release binaries cover the desktop platforms the release workflow already builds | The plan says "via the existing release workflow" | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `initialize_announces_name_version_and_capabilities` (`crates/pincel-mcp/src/server.rs`); `end_to_end_draw_preview_export_in_temp_workspace` (`crates/pincel-mcp/tests/stdio.rs`) | built |        |
| AC-02 | `tool_schemas_are_valid` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-03 | `doc_new_creates_active_document_and_writes_nothing` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-04 | `doc_new_palette_is_preset_or_db32` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-05 | `tools_act_on_active_doc_or_named_doc_only` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-06 | `unknown_doc_or_no_open_doc_fails_and_changes_nothing` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-07 | `doc_info_and_list_report_fields_and_dirty_state` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-08 | `doc_close_removes_document_and_later_calls_fail` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-09 | `every_mutating_call_is_exactly_one_undo_step`, `draw_pixels_empty_batch_records_no_undo_step` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-10 | `undo_and_redo_report_depths` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-11 | `preview_scales_nearest_and_returns_png`, `preview_of_one_pixel_sprite_is_256_square` (`crates/pincel-agent/src/tests/preview.rs`) | built |        |
| AC-12 | `preview_frame_and_region_select_what_is_shown`, `preview_grid_only_changes_block_borders_and_is_omitted_below_scale_4` (`crates/pincel-agent/src/tests/preview.rs`) | built |        |
| AC-13 | `read_pixels_hex_grid_matches_cel` (`crates/pincel-agent/src/tests/preview.rs`) | built |        |
| AC-14 | `read_pixels_limits_region_size_and_canvas` (`crates/pincel-agent/src/tests/preview.rs`) | built |        |
| AC-15 | `layer_tools_change_and_list_the_stack` (`crates/pincel-agent/src/tests/structure.rs`) | built |        |
| AC-16 | `frame_tools_change_frames_and_clone_cels` (`crates/pincel-agent/src/tests/structure.rs`) | built |        |
| AC-17 | `tag_tools_change_and_list_tags` (`crates/pincel-agent/src/tests/structure.rs`) | built |        |
| AC-18 | `palette_tools_set_and_load_presets` (`crates/pincel-agent/src/tests/structure.rs`) | built |        |
| AC-19 | `draw_pixels_sets_listed_pixels_only_in_all_color_forms` (`crates/pincel-agent/src/tests/draw.rs`) | built |        |
| AC-20 | `draw_pixels_outside_canvas_rejects_whole_batch` (`crates/pincel-agent/src/tests/draw.rs`) | built |        |
| AC-21 | `malformed_color_or_missing_palette_index_fails_naming_argument` (`crates/pincel-agent/src/tests/draw.rs`) | built |        |
| AC-22 | `draw_shapes_match_the_editor_tools` (`crates/pincel-agent/src/tests/draw.rs`) | built |        |
| AC-23 | `draw_fill_matches_bucket_and_non_contiguous_recolors_all` (`crates/pincel-agent/src/tests/draw.rs`) | built |        |
| AC-24 | `draw_clear_clears_region_or_whole_cel` (`crates/pincel-agent/src/tests/draw.rs`) | built |        |
| AC-25 | `selection_limits_fill_and_effect_until_cleared` (`crates/pincel-agent/src/tests/draw.rs`) | built |        |
| AC-26 | `effect_apply_matches_the_editor_effect` (`crates/pincel-agent/src/tests/draw.rs`) | built |        |
| AC-27 | `effect_apply_unknown_id_or_out_of_range_param_fails` (`crates/pincel-agent/src/tests/draw.rs`) | built |        |
| AC-28 | `locked_layer_or_locked_group_refuses_edits` (`crates/pincel-agent/src/tests/structure.rs`) | built |        |
| AC-29 | `doc_open_matches_what_the_editor_reads_from_the_file`, `doc_open_corrupt_file_fails_naming_it` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-30 | `doc_save_round_trips_and_defaults_to_the_document_file` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-31 | `doc_save_without_path_on_new_document_fails_and_writes_nothing` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-32 | `export_png_equals_preview_without_grid` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-33 | `export_sheet_lays_out_frames_and_writes_sheet_json` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |
| AC-34 | `resolve_rejects_parent_escape_and_absolute_paths`, `write_outside_writes_nothing`, `symlink_leading_out_is_refused` (unix) (`crates/pincel-mcp/src/sandbox.rs`); `paths_resolve_in_cwd_without_workspace_and_escapes_fail` (`crates/pincel-mcp/tests/stdio.rs`) | built |        |
| AC-35 | `parse_args_reads_workspace_and_read_only` (`crates/pincel-mcp/src/lib.rs`); `workspace_option_and_read_only_mode`, `paths_resolve_in_cwd_without_workspace_and_escapes_fail` (`crates/pincel-mcp/tests/stdio.rs`) | built |        |
| AC-36 | `read_only_refuses_writes_but_reads` (`crates/pincel-mcp/src/sandbox.rs`); `workspace_option_and_read_only_mode` (`crates/pincel-mcp/tests/stdio.rs`) | built |        |
| AC-37 | `unknown_tool_and_bad_arguments_are_named_and_serving_continues` (`crates/pincel-mcp/src/server.rs`); `bad_messages_get_errors_and_serving_continues` (`crates/pincel-mcp/tests/stdio.rs`) | built |        |
| AC-38 | `invalid_json_gets_parse_error_and_serving_continues` (`crates/pincel-mcp/src/server.rs`); `bad_messages_get_errors_and_serving_continues` (`crates/pincel-mcp/tests/stdio.rs`) | built |        |
| AC-39 | `resources_list_and_read_palette_presets` (`crates/pincel-mcp/src/server.rs`); `list_resources_has_one_uri_per_preset`, `read_resource_returns_the_preset_colors` (`crates/pincel-agent/src/resources.rs`) | built |        |
| AC-40 | `end_to_end_draw_preview_export_in_temp_workspace` (`crates/pincel-mcp/tests/stdio.rs`) | built |        |
| AC-41 | manual pending — `build pincel-mcp` step in `.github/workflows/release.yml` (desktop job: Windows x64, macOS universal, Linux x64) | built |        |
| AC-42 | manual pending — README section "Use Pincel from an AI agent (MCP)" | built |        |
| AC-43 | `paint_target_defaults_to_active_layer_and_frame` (`crates/pincel-agent/src/tests/draw.rs`) | built |        |
| AC-44 | `read_pixels_index_format_uses_first_exact_palette_match` (`crates/pincel-agent/src/tests/preview.rs`) | built |        |
| AC-45 | `preview_above_2048_cap_fails_suggesting_region` (`crates/pincel-agent/src/tests/preview.rs`) | built |        |
| AC-46 | `doc_new_out_of_range_size_fails_without_adding_a_document` (`crates/pincel-agent/src/tests/doc.rs`) | built |        |

## Decisions

- Tool names follow `area.verb`; every tool takes an optional `doc` and falls back to the active document — one convention for both hosts (plan, 2026-10-28).
- Colors are `#RRGGBB[AA]` strings everywhere; Pixel tools also accept `{ "index": n }` — readable for models and palette-aware for indexed sprites (plan).
- Coordinates are document pixels with the origin top-left — matches the editor and the cores (plan).
- Every mutating tool call is one undo step — agents and users can revert a call as a unit; the same rule holds in live mode (plan).
- The headless server opens no network connection and touches files only inside the workspace; documents stay in memory until `doc.save` / `export.*` — the sandbox from `mcp.md` (plan).
- The Pixel / Image split of the v1 tools across `mcp-headless-pixel` and `mcp-headless-image`, and `pincel://palettes/{id}` here, follow the plan's task order (Pixel tools and host first, Image tools with the guide and effect resources second) — migration, 2026-10-29.
- 2026-10-29: MCP speaks JSON-RPC 2.0 through an in-house newline-delimited stdio loop (no `rmcp`); `pincel-mcp` keeps tokio out, pins one MCP protocol revision, and the dispatch stays transport-agnostic for `mcp-live-desktop` — the surface is small (initialize, tools/*, resources/*, ping) and `rmcp` would add tokio and an SDK that follows a fast-moving protocol; §15 row recorded with the 2026-10-29 Decision Log batch (owner accepted the recommendation).
- 2026-10-29: Tool input schemas are hand-written as `serde_json::json!` next to each tool and checked by `tool_schemas_are_valid` — no new direct dependency (`schemars` is only transitive), and descriptions written for models read better by hand (owner accepted the recommendation).
- 2026-10-29: Agent tools live in one platform-free crate `pincel-agent` (over `pincel-core` / `pincel-image-core`, no I/O), hosted by the new binary `pincel-mcp` (stdio) and later the desktop app (live mode); `mcp.md` carries a status note that the `mcp-*` feature specs supersede it — two new crates and a public tool surface must go on record (CLAUDE.md §3.3); §15 row recorded with the 2026-10-29 Decision Log batch (owner accepted the recommendation).
- 2026-10-29: `draw.*`, `selection.*` and `effect.apply` take optional `layer` / `frame`; omitted, they use the active layer and frame by the editor's paint-target rule, `layer.add` / `frame.add` make the new item active, and `doc.info` reports both (AC-43, AC-07) — live mode then behaves the same (the agent draws where the user is looking) and the defaults reuse the editor rule (owner accepted the recommendation).
- 2026-10-29: `doc.read_pixels` `format: "index"` returns the first palette entry with exactly that RGBA, or `-` when none matches, on any Pixel sprite (AC-44) — exact, lossless to read and the same "first occurrence" rule as shading-ink; Pixel mode has no indexed sprites (owner accepted the recommendation).
- 2026-10-29: `grid: true` draws a 1-px `#808080` line at 50 % alpha on the first row and column of every block and is omitted below scale 4 with a note (AC-12) — mid-grey at half alpha shows on light and dark art, and below scale 4 a grid line would hide most of a block (owner accepted the recommendation).
- 2026-10-29: `doc.preview` never downscales; the PNG is capped at 2048 px per edge and an oversized request fails suggesting `region` (AC-11, AC-45) — the preview must stay pixel-exact, and MCP clients downscale large images anyway (owner accepted the recommendation).
- 2026-10-29: Pixel `doc.new` enforces the editor's 1–4096 per side and fails with "width must be 1–4096" (AC-46) — a document the agent creates must also open and edit in the editor (owner accepted the recommendation).
- 2026-10-29 (build): `pincel-core` gains `CommandBatch` (several commands as one atomic undo entry) and `PutCel` (insert a whole cel undoably), both new public API — one call must be exactly one undo step that restores the document byte for byte (AC-09), and a failing call must change nothing.
- 2026-10-29 (build): the editor's paint-target rule moves into `Sprite::paint_target_layer` and `pincel-wasm` keeps a thin wrapper — the plan's "phase 0" refactor reduced to what the agent shares, with no behavior change.
- 2026-10-29 (build): pixel tools run the editor's own `pincel-core` command (SetPixel, DrawLine, DrawRectangle, DrawEllipse, FillRegion, ClearRegion, the `pincel-effects` spec) on a scratch copy of the target cel and commit one `PutCel` — identical pixels to the editor, atomic, exact undo; a call that changes nothing records no step.
- 2026-10-29 (build): `pincel-agent` does no I/O; file tools call the host's `FileHost` (`read` / `write` of workspace-relative paths), which owns the sandbox and the read-only flag.
- 2026-10-29 (build): `effect.apply` takes named parameters from a Rust mirror of `ui/src/lib/effects/catalog.ts` (names, ranges, defaults; Curves / Levels converted like the editor's `toParams`), and choices accept the option name or index — the Rust effect spec has no ranges, and the editor's ranges are what "allowed range" means.
- 2026-10-29 (build): `draw.*`, `effect.apply` and `layer.remove` refuse locked layers; `layer.set` does not, so an agent can unlock a layer.
- 2026-10-29 (build): `doc.read_pixels` without `frame` reads the active frame; `doc.preview` and `export.png` without `frame` show frame 0 (Assumptions). `export.png` defaults to scale 1; exports are capped at 16384 px per edge.
- 2026-10-29 (build): the selection is document state shared by all layers and frames (as in the editor) and not an undo step; `selection.*` validate `layer` / `frame`, which do not scope it.
- 2026-10-29 (build): `doc.info` reports `layers`, `frames` and `tags` as counts (the `*.list` tools give the details), `active_layer` as the resolved paint target, plus `path` and the undo / redo depths.
- 2026-10-29 (build): arguments are validated against the tool schema before every call by one schema-subset validator; a range error reads "`{arg}` must be {min}–{max}" (AC-46's "width must be 1–4096"), and an enum error names the rejected value (AC-27).
- 2026-10-29 (build): `pincel-agent` encodes PNGs with the workspace `png` crate (already in the lockfile through `pincel-core`) — no new dependency.
- 2026-10-29 (build): the `pincel-agent` block was written code-first in the orchestrated run and its tests right after, so no red run is recorded for it.
- 2026-10-29 (build): `pincel-mcp` pins MCP protocol revision `2025-06-18`; 2025-11-25 is the newest revision known at build time, but the surface used here (tools, image content, `structuredContent`, `isError`, resources) is the same in both, and the older pin keeps more clients connecting — bump it in `server.rs` (`PROTOCOL_VERSION`) when clients drop it.
- 2026-10-29 (build): an unknown tool is a JSON-RPC error (`-32602`) naming the tool; arguments that fail the schema, and every tool refusal, are a tool result with `isError: true` whose text names the tool and the argument, so the model can correct the call; an unknown resource is `-32002`; JSON-RPC batches are answered with `-32600` (MCP 2025-06-18 dropped batching).
- 2026-10-29 (build): `pincel-mcp` is a library (`Server`, `Sandbox`, `parse_args`) plus a thin `main.rs`; `Server::handle` takes one parsed message so `mcp-live-desktop` can reuse it over HTTP. Base64 for image content is in-house (no dependency).
- 2026-10-29 (build): the sandbox canonicalizes the workspace once, resolves `..` lexically (refusing any climb above the root), refuses absolute and drive-prefixed paths, and canonicalizes the deepest existing ancestor of every path so a symlink leading out is refused; writes create missing directories inside the workspace. Exit codes follow `cli.md` (2 usage, 3 I/O).
- 2026-10-29 (build): release binaries are named `pincel-mcp-windows-x64.exe`, `pincel-mcp-macos-universal` (lipo of aarch64 + x86_64) and `pincel-mcp-linux-x64`, built in the existing desktop job, so `SHA256SUMS` covers them.
- 2026-10-29 (build): the gate lists (`.claude/gates.json`, CI clippy / test, AGENTS.md §10) still name only the six library crates; `pincel-agent` and `pincel-mcp` need adding there by the owner (gate configuration).
