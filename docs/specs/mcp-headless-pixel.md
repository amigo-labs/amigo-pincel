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
- [ ] AC-07 (event): When the client calls `doc.list` or `doc.info`, then each document's width, height, layers, frames, tags, palette size and dirty state are reported; the dirty state is true after a successful mutating call and false after `doc.save`.
- [ ] AC-08 (event): When the client calls `doc.close`, then the document is no longer listed by `doc.list`, and a later call naming its id fails as in AC-06.
- [ ] AC-09 (always): Every successful mutating tool call that changes the document is exactly one undo step: one `doc.undo` returns the document to its exact state before the call, and one `doc.redo` re-applies the whole call.
- [ ] AC-10 (event): When the client calls `doc.undo` or `doc.redo`, then the result reports the new undo and redo depths.
- [ ] AC-11 (event): When the client calls `doc.preview`, then the result contains MCP image content holding a PNG of the composed frame in which every document pixel is an s×s block of one color — s being `scale`, or, without `scale`, the smallest integer that makes the longest side at least 256 px (s = 1 when the longest side already is) — and the result states the PNG's width and height.
- [ ] AC-12 (option): Where `doc.preview` gets `frame` or `region`, the preview shows that frame, or only that rectangle of the frame; where it gets `grid: true`, it differs from the same preview without grid only on pixels at the boundaries between document-pixel blocks.
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

## Technical notes

Hints from the legacy plan (2026-10-28). Paths and line numbers drift — grep for the names.

**Prerequisite refactor (plan "phase 0", no spec of its own).** Much document logic sits behind `#[wasm_bindgen]` types today and a native binary cannot call it:

- `crates/pincel-wasm/src/lib.rs`: `edit_target_layer` (≈L1174), `ensure_paint_cel` (≈L1195), `apply_tool` / `applyTool` (≈L552), `apply_line` / `applyLine` (≈L587), compose with onion skin, palette import.
- Move the Pixel `Document` operations into plain Rust (a plain module, a `pincel-core::session` façade, or the `pincel-agent` crate itself); `pincel-wasm` keeps thin `#[wasm_bindgen]` wrappers (CLAUDE.md §5.5).
- Done when all existing wasm tests stay green with no behavior change, as pure `refactor(...)` commits. The Image counterpart (`CommandSpec` dispatch in `pincel-image-wasm`) is the prerequisite of `mcp-headless-image`.

**Docs prerequisite.** The plan's first commit adds a status note at the top of `docs/specs/mcp.md` pointing to the MCP feature specs, plus the Decision Log entries (see Open questions).

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
| `doc.list` / `doc.info` / `doc.close` | — | sizes, layers, frames, tags, palette size, dirty |
| `doc.preview` | `frame?`, `scale?`, `grid?: bool`, `region?` | image content (PNG) + size |
| `doc.read_pixels` | `region` (≤ 64×64), `layer?`, `frame?`, `format: hex\|index` | rows of colors as text |
| `doc.undo` / `doc.redo` | — | new undo / redo depth |
| `doc.save` | `path?` | path |
| `export.png` | `frame?`, `scale?`, `path` | path |
| `layer.add` / `layer.remove` / `layer.list` / `layer.set` | `set`: name, visible, opacity, blend, locked; groups via `parent?` | |
| `frame.add` (`clone_from?`, `duration_ms?`) / `frame.remove` / `frame.set_duration` / `frame.list` | | |
| `tag.add` / `tag.remove` / `tag.list` | name, from, to, direction | |
| `palette.get` / `palette.set` (entries) / `palette.load_preset` | | |
| `draw.pixels` | batch `[{x, y, color}]` | |
| `draw.line` / `draw.rect` / `draw.ellipse` | `filled?` | |
| `draw.fill` | `contiguous?` | |
| `draw.clear` | `region?` | |
| `selection.set_rect` / `selection.clear` | | |
| `effect.apply` | `id` + `params` (`pincel-effects` catalog) | |
| `export.sheet` | `columns`, `tag?`, `scale?`, `path`, `json?` | |

Every tool takes an optional `doc`. `mcp.md` gives drawing tools `layer_id?` / `frame_id?` as well; the plan does not (see Open questions).

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

**Testing hints.** Unit tests per tool in `pincel-agent` (args validation, one undo step per call, error messages), e.g. `draw_pixels_batch_is_one_undo_step`, `preview_scales_nearest_and_returns_png`, `read_pixels_hex_grid_matches_cel`. An integration test spawns the binary and speaks JSON-RPC over stdin/stdout (AC-40); a sandbox test rejects `../x.png`.

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

- MCP protocol: the official Rust SDK `rmcp` (new dependency, pulls tokio into the binary only; check version, license, protocol revision and binary size first) or a minimal in-house JSON-RPC 2.0 stdio loop (≈300 lines, no dependency; the plan recommends in-house)? Needs the user's decision and a Decision Log entry.
- Tool input schemas: derived with `schemars` (new dependency) or hand-written next to each tool (the plan recommends hand-written, with a test that every schema parses and every example call validates)? Needs the user's decision.
- The Decision Log entry for the agent architecture (one platform-free `pincel-agent` crate, two hosts; new crates `pincel-agent` and `pincel-mcp`) and the status note in `mcp.md` are not written yet — confirm and write them with this spec.
- Which layer and frame do `draw.*`, `selection.*` and `effect.apply` act on? `mcp.md` gives drawing tools `layer_id?` / `frame_id?`, the plan lists none — are they arguments, and which layer and frame are used when omitted?
- What does `doc.read_pixels` with `format: "index"` return for a pixel whose color is not in the palette, or for an RGBA (non-indexed) sprite?
- What does `grid: true` draw in `doc.preview` (line color, width), and what happens at scales 1–2 where a grid line would cover a whole pixel block?
- Should `doc.preview` cap its output size for large sprites (up to 4096²), e.g. by downscaling, or always return scale ≥ 1?
- What size limits does `doc.new` enforce for Pixel documents (the editor's New-dialog limits?), and what error does an oversized request get?

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
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |
| AC-22 |      | open   |        |
| AC-23 |      | open   |        |
| AC-24 |      | open   |        |
| AC-25 |      | open   |        |
| AC-26 |      | open   |        |
| AC-27 |      | open   |        |
| AC-28 |      | open   |        |
| AC-29 |      | open   |        |
| AC-30 |      | open   |        |
| AC-31 |      | open   |        |
| AC-32 |      | open   |        |
| AC-33 |      | open   |        |
| AC-34 |      | open   |        |
| AC-35 |      | open   |        |
| AC-36 |      | open   |        |
| AC-37 |      | open   |        |
| AC-38 |      | open   |        |
| AC-39 |      | open   |        |
| AC-40 |      | open   |        |
| AC-41 |      | open   |        |
| AC-42 |      | open   |        |

## Decisions

- Tool names follow `area.verb`; every tool takes an optional `doc` and falls back to the active document — one convention for both hosts (plan, 2026-10-28).
- Colors are `#RRGGBB[AA]` strings everywhere; Pixel tools also accept `{ "index": n }` — readable for models and palette-aware for indexed sprites (plan).
- Coordinates are document pixels with the origin top-left — matches the editor and the cores (plan).
- Every mutating tool call is one undo step — agents and users can revert a call as a unit; the same rule holds in live mode (plan).
- The headless server opens no network connection and touches files only inside the workspace; documents stay in memory until `doc.save` / `export.*` — the sandbox from `mcp.md` (plan).
- The Pixel / Image split of the v1 tools across `mcp-headless-pixel` and `mcp-headless-image`, and `pincel://palettes/{id}` here, follow the plan's task order (Pixel tools and host first, Image tools with the guide and effect resources second) — migration, 2026-10-29.
