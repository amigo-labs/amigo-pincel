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

## Technical notes

Hints from the legacy plan (2026-10-28). Paths drift — grep for the names.

**Prerequisite refactor (plan "phase 0", no spec of its own).** Move the Image `CommandSpec` enum and its dispatch `run_command` out of `crates/pincel-image-wasm/src/lib.rs` (`enum CommandSpec` ≈L468, `fn run_command` ≈L846) into a plain crate or module that `pincel-agent` can call natively. ts-rs generation must keep working: `cargo test -p pincel-image-wasm export_command_spec` still writes `ui/src/modes/image/core/generated/`, and CI checks for drift. No behavior change; all wasm tests green; `refactor(...)` commits only.

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

**Size limits.** The editor caps Image documents at 16384 px per side and `MAX_CANVAS_AREA` = 8192 × 8192 (`ui/src/modes/image/stores/editor.svelte.ts`), a wasm32 memory limit; the native binary is not bound by it (see Open questions).

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

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| `draw.text` uses the same bundled font as the editor (Liberation Sans) | Same output as the editor; no font file needed in the workspace | Low |
| The draw tools act on the same target layer as the Pixel tools (whatever `mcp-headless-pixel` settles) | One convention for both modes | Medium |
| The shape, text and stroke tools reuse the editor's commands, so "same pixels as the editor" is the correctness reference | The plan: one tool implementation, no re-implementation | Low |

## Open questions

- Phase 0 for Image: may the extracted `CommandSpec` and its dispatch live in a new crate (which?) or must they move into `pincel-image-core`? The plan leaves "a plain crate or module" open, and a new crate is a public-API / crate-boundary decision.
- What does `doc.save` without `path` do for an Image document opened from a flat image (PNG / JPEG / …): write `.ora`, overwrite the original format (flattened, as `save-original-format` proposes for the editor), or fail and ask for a path?
- Should the headless server enforce the editor's Image limits (side ≤ 16384, area ≤ 64 Mpx) on `doc.new`, `doc.open`, `canvas.resize` and `image.scale`, although the native binary is not bound by wasm32's 4 GB — and what is the error?
- Are the optional MCP prompts `create-sprite` (size, subject, palette) and `edit-screenshot` (crop, annotate, export) part of v1, and in which spec?

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

- Image tools are implemented in the platform-free `pincel-agent` crate over `pincel-image-core`, never in the editor — the same code later serves live mode (plan, 2026-10-28).
- `pincel://effects` is generated from the `pincel-effects` catalog, not hand-written — it cannot drift from the effects the tools accept (plan).
- `layer.*` on Image documents has no groups until `image-layer-groups` is built — the tools expose what the core supports (plan).
