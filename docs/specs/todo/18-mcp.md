---
id: todo-mcp
title: MCP server — LLMs work with Pincel (headless first, then live in the app)
status: todo
size: XL (six M sessions; two dependency decisions up front)
mode: Pixel + Image
depends_on: []
created: 2026-10-28
supersedes_in_part: docs/specs/mcp.md (architecture, code location, tool list)
---

# MCP server: LLMs work with Pincel

## Goal

LLM agents (Claude Code, Claude Desktop, any MCP client) create and edit Pincel documents through the **Model Context Protocol**. This happens in two stages that share one tool implementation.

1. **Headless.** A `pincel-mcp` binary over stdio. It opens, creates, edits and exports `.aseprite`, `.png` and `.ora` files inside a workspace folder, and returns **PNG previews as MCP image content**, so the model sees what it draws.
2. **Live.** The desktop app (Tauri) exposes the **same tools** against the document that is open on screen.
   - The user watches every step.
   - Each tool call is one undo step.
   - Saving stays under the user's control.

## Why

The user considers this the most important missing feature. Nothing in §18 (competitive landscape) offers it: Aseprite has Lua scripting but no MCP, and Photopea has a script API but no agent protocol. It turns Pincel into the pixel-art and image backend for coding agents, for example "add four enemy variants to `assets/`" in a game repo, which is Pincel's core audience (§1.2, engine interop).

## Relation to `docs/specs/mcp.md`

`mcp.md` (draft, 2026-05-13) holds the original vision. Keep these parts of it:
- the agent loop: plan → draw → preview → validate → export;
- `canvas.preview` as the critical tool;
- the workspace sandbox;
- validation as a quality gate.

It is **out of date** on the following points, which this spec replaces:

| `mcp.md` says | Reality now |
|---------------|-------------|
| Crate `core`, path `packages/lessons-mcp/` | Crates `pincel-core` (Pixel), `pincel-image-core` (Image), `pincel-effects` (both) |
| Indexed bitmaps only | Pixel mode is RGBA with optional indexed; Image mode exists (layers, brushes, effects) |
| Curriculum at `lessons.pincel.app`, `pincel://lessons/*` | Does not exist. Stays future vision (`lessons.md`) and is **not a prerequisite** |
| `validate.*` with banding / hue-shift functions | No validation code exists yet. That is a later phase here |
| `.ait` export | Amigo Engine runtime format, not in this repo. Out of scope |

The session's first commit adds a status note at the top of `mcp.md` pointing here.

## Architecture

### One tool layer, two hosts

```text
crates/pincel-agent/          NEW, platform-free (no tokio, no wasm-bindgen, no file I/O)
  src/lib.rs                  ToolRegistry: list_tools() -> Vec<ToolDef{name, description, input_schema}>
                              call(&mut Workspace, name, args: serde_json::Value) -> Result<ToolOutput, AgentError>
  src/pixel/*.rs              tools over pincel-core (sprite, layer, frame, tag, palette, draw, effect, export)
  src/image/*.rs              tools over pincel-image-core (image, layer, draw, adjust/effect, canvas, export)
  src/preview.rs              compose → nearest-neighbour upscale → PNG bytes
ToolOutput { json: serde_json::Value, image_png: Option<Vec<u8>> }

crates/pincel-mcp/            NEW binary: MCP over stdio, workspace sandbox, file read/write
pincel-wasm / pincel-image-wasm   + `agentCall(json) -> json` against the open document (live mode)
src-tauri/                    + MCP endpoint that forwards calls to the webview (live mode)
```

- **The same Rust tool code runs in both hosts.**
  - Headless, `pincel-mcp` calls `pincel-agent` natively.
  - Live, the editor's wasm module calls `pincel-agent` inside the webview against the document on screen. Documents live in the webview's wasm (§9.3, ADR-001), not in the Tauri process. The Tauri backend therefore only forwards JSON and never re-implements a tool.
- `pincel-agent` follows CLAUDE.md §5.1 (platform-free, `thiserror`, std-only). The **host** owns file I/O and passes bytes in and out.

### Prerequisite refactor (phase 0)

A lot of document logic lives inside the wasm crates today, for example:
- `crates/pincel-wasm/src/lib.rs`: `edit_target_layer`, `ensure_paint_cel`, `applyTool`, `applyLine`, compose with onion, palette import;
- the image `CommandSpec` dispatch, `run_command` in `crates/pincel-image-wasm/src/lib.rs`.

That logic sits behind `#[wasm_bindgen]` types, so a native binary can't call it.

**Extract it into plain Rust before writing any tool:**
- **Pixel:** move the `Document` operations from `pincel-wasm` into a plain module, a `pincel-core::session` façade, or the `pincel-agent` crate itself. `pincel-wasm` keeps thin `#[wasm_bindgen]` wrappers. That matches CLAUDE.md §5.5's "thin wrappers".
- **Image:** move `CommandSpec` and its dispatch out of `pincel-image-wasm` into a plain crate or module. Keep ts-rs generation working: `cargo test -p pincel-image-wasm export_command_spec` must still write `ui/src/modes/image/core/generated/`, and CI checks for drift.
- **Done when:** all existing wasm tests stay green, there is no behavior change, and it is pure refactor commits (`refactor(pincel-wasm)`, …).

### Dependency decisions (stop and ask, CLAUDE.md §9 / §12)

1. **MCP protocol.**
   - Either the official Rust SDK **`rmcp`** (modelcontextprotocol/rust-sdk; pulls in tokio for the binary only, never into `pincel-agent`),
   - or a **minimal in-house JSON-RPC 2.0 stdio loop** for `initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/read` and `ping`. That is roughly 300 lines and has no dependency.
   - Before asking: check `rmcp`'s current version, license, protocol revision and binary size.
   - Recommendation: in-house for stdio, because the protocol surface is small, and revisit when HTTP transport is needed.
2. **JSON Schema for tool inputs.** Either `schemars` (derive from the args structs) or hand-written schemas next to each tool. Recommendation: hand-written, so the descriptions stay precise; add a test that every schema parses and every example call validates.

## Tools (v1)

**Naming.** `area.verb`. Every tool takes an optional `doc` (document id); without it, the active document is used.

**Colors.**
- `#RRGGBB[AA]` strings everywhere.
- Pixel tools also accept `{ index: n }` for palette entries.

**Coordinates.** Document pixels, origin top-left.

**Undo.** **Every mutating tool call is one undo step.** Seal the bus before and after the call, as `run_sealed` does in `crates/pincel-wasm/src/transforms.rs`.

**Shared (both modes)**

| Tool | Args | Returns |
|------|------|---------|
| `doc.new` | `mode: pixel\|image`, `width`, `height`, `palette?` (Pixel preset id) | `doc` id, info |
| `doc.open` | `path` (workspace-relative; `.aseprite` → Pixel, images / `.ora` → Image) | `doc`, info |
| `doc.list` / `doc.info` / `doc.close` | — | sizes, layers, frames, tags, palette size, dirty |
| `doc.preview` | `frame?`, `scale?` (auto: longest side ≥ 256 px), `grid?: bool`, `region?` | **image content** (PNG) + size |
| `doc.read_pixels` | `region` (≤ 64×64), `layer?`, `frame?`, `format: hex\|index` | rows of colors as text. For small sprites, models reason better over a text grid than over an image |
| `doc.undo` / `doc.redo` | — | new undo / redo depth |
| `doc.save` | `path?` (defaults to where it was opened) | path |
| `export.png` | `frame?`, `scale?`, `path` | path |

**Pixel mode** (`pincel-core`)

| Tool | Notes |
|------|-------|
| `layer.add` / `layer.remove` / `layer.list` / `layer.set` | `set`: name, visible, opacity, blend, locked; groups via `parent?` |
| `frame.add` (`clone_from?`, `duration_ms?`) / `frame.remove` / `frame.set_duration` / `frame.list` | |
| `tag.add` / `tag.remove` / `tag.list` | name, from, to, direction |
| `palette.get` / `palette.set` (entries) / `palette.load_preset` (DB32, PICO-8, …) | |
| `draw.pixels` | batch `[{x, y, color}]`, the workhorse |
| `draw.line` / `draw.rect` / `draw.ellipse` | `filled?` |
| `draw.fill` | flood fill (`contiguous?`) |
| `draw.clear` | `region?` |
| `selection.set_rect` / `selection.clear` | so effects and fills can be scoped |
| `effect.apply` | `id` + `params`, from the shared `pincel-effects` catalog |
| `export.sheet` | `columns`, `tag?`, `scale?`, `path`, `json?` |

**Image mode** (`pincel-image-core`)

| Tool | Notes |
|------|-------|
| `layer.*` | as in Pixel mode, minus groups until `03-image-layer-groups.md` is done |
| `draw.stroke` | brush: `points`, size, hardness, opacity, color; or `tool: eraser` |
| `draw.shape` | rect / rounded / ellipse / polygon / line, stroke and fill |
| `draw.text` | text, position, size, color, align |
| `draw.fill` / `draw.gradient` | |
| `select.rect` / `select.ellipse` / `select.wand` / `select.clear` | |
| `adjust.apply` / `effect.apply` | `pincel-effects` ids + params |
| `canvas.resize` / `image.scale` / `image.crop_to_selection` / `image.flip` / `image.rotate` | |
| `export.png` / `export.jpeg` (`quality`) / `export.webp` / `doc.save` (`.ora`) | |

**Resources**
- `pincel://guide`: a concise usage guide for agents, covering the workflow, coordinate system, colors, undo, preview and pitfalls.
- `pincel://effects`: effect ids with their parameter ranges, generated from the catalog.
- `pincel://palettes/{id}`: the built-in presets.

**Prompts (optional)**
- `create-sprite` (size, subject, palette)
- `edit-screenshot` (crop, annotate, export)

**Validation (later phase, from `mcp.md`)**
- `validate.run` with the first rules: `palette_adherence`, `color_count`, `has_outline`, `seamless_edges`, and later `banding`.
- Implement them in `pincel-agent`, not in the editor, until the editor wants them too.

## Headless host (`pincel-mcp`)

**CLI.** `pincel-mcp [--workspace <dir>] [--read-only]` (CLI conventions in `docs/specs/cli.md`).

**Sandbox.**
- Every path is resolved inside the workspace (canonicalize, reject `..`, reject symlinks leading out). Use error codes as in `mcp.md`, e.g. `E007 path outside workspace`.
- No network.

**Session.**
- A `Workspace { docs: HashMap<DocId, Doc>, active }`, where each `Doc` is a Pixel `Sprite` + undo bus or an Image `Document` + bus.
- In memory only, until `doc.save` or `export.*`.

**Errors.** Structured MCP tool errors (`isError: true`) with a message the model can act on, e.g. "pixel (40, 3) outside 32×32 canvas".

**Distribution, phase 1.**
- `cargo install --path crates/pincel-mcp`, plus release binaries via the existing release workflow (`docs/RELEASING.md`).
- Document `claude mcp add pincel -- pincel-mcp --workspace .`.
- An npm wrapper (`@amigo-labs/pincel-mcp`, per `mcp.md`) can come later.

## Live host (desktop app)

**Off by default.**
- The user enables **"Allow AI agents to control Pincel"** in a settings entry (Help or View menu until a settings dialog exists). The setting is persisted in the IndexedDB prefs.
- While a client is connected, the status bar shows **"AI connected"**, with a disconnect button.

**Transport.**
- Tauri starts an MCP endpoint on **127.0.0.1 only**, with a random bearer token, using MCP's Streamable HTTP transport.
- The settings entry shows the exact client command or config to copy.
- Optionally, `pincel-mcp --attach` acts as a stdio bridge for clients that only speak stdio. It reads the port and token from a file in the app's config dir.

**Routing.**
1. The Tauri command receives `tools/call`.
2. It emits an event to the webview with a request id.
3. The mounted editor calls `agentCall(json)` on its wasm document.
4. The result comes back via `invoke`.

The UI refreshes like after any command (`syncInfo` in Image mode; recompose in Pixel mode).

**Limits.**
- In live mode, `doc.open`, `doc.new` and `doc.save` act through the shell's existing flows. **Saving to a new path asks the user** (the normal Save As dialog).
- Agents can't touch the file system beyond that.

**PWA.** A browser page can't host a server, so live mode is desktop-only. Watch the browser-side proposals for exposing page tools to agents ("WebMCP") as a later option, and record that in the Decision Log.

## Tasks (one M session each)

0. **`docs(spec)` + dependency decisions.**
   - Add a status note to `mcp.md`.
   - Decision Log entries: agent architecture (one `pincel-agent` crate, two hosts), off-by-default live mode, localhost plus token.
   - Ask the user about `rmcp` versus in-house and `schemars` versus hand-written schemas.
1. **Refactor:** extract the Pixel document operations from `pincel-wasm` (no behavior change, all wasm tests green).
2. **Refactor:** extract the Image `CommandSpec` dispatch from `pincel-image-wasm`. ts-rs generation must still work.
3. **`feat(pincel-agent)`: Pixel tools, preview and read_pixels.**
   - Unit tests per tool (args validation, one undo step per call, error messages). For example:
     - `draw_pixels_batch_is_one_undo_step`
     - `preview_scales_nearest_and_returns_png`
     - `read_pixels_hex_grid_matches_cel`
4. **`feat(pincel-mcp)`: stdio host, sandbox, `doc.open` / `doc.save` / `export.*`.**
   - An integration test that spawns the binary, speaks JSON-RPC over stdin/stdout, and runs `initialize` → `tools/list` → `doc.new` → `draw.pixels` → `doc.preview` (image content present) → `export.png` → the file exists inside a temp workspace.
   - A sandbox test: `../x.png` is rejected.
   - Docs: README section and a Claude Code setup snippet.
5. **`feat(pincel-agent)`: Image tools**, plus the `pincel://guide` and `pincel://effects` resources.
6. **Live mode:**
   - `agentCall` in both wasm crates;
   - the Tauri endpoint and routing;
   - the settings toggle, status indicator and token display;
   - i18n.
   - e2e:
     - the UI side via a Playwright test that calls the webview's `agentCall` bridge directly: the canvas updates and Undo reverts it;
     - the Tauri side via a Rust test of the request/response routing with a mocked webview.

## Out of scope

- Hosted or remote MCP (`mcp.pincel.app` in `mcp.md`), user accounts and telemetry.
- The curriculum and lesson resources (`lessons.md`).
- Live mode in the PWA.
- Generative image models. The agent draws with Pincel's tools, it does not generate rasters.
