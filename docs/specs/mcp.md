---
id: mcp
title: MCP Server (AI-Driven Asset Creation)
status: draft
version: 0.1.0
owner: daniel
created: 2026-05-13
last_updated: 2026-05-13
related:
  - canvas
  - lessons
  - cli
---

# MCP Server (AI-Driven Asset Creation)

## Overview

The Pincel MCP server exposes Pincel's pixel-editing core and lesson
curriculum as Model Context Protocol tools and resources. Its primary
purpose is to let AI agents — particularly Claude in Claude Code, Claude
Desktop, and Claude.ai — **create real pixel art assets**, with Pincel's
validation acting as a quality gate and the lesson curriculum acting as
the constitutional reference for what "good" pixel art looks like.

This is not a tutoring assistant. It is a pixel-art *production tool* for
AI agents. The output is `.aseprite` files (and `.png`, `.ait`) that drop
directly into game projects like Amigo Engine titles.

## Goals

- **AI as pixel artist**: AI agents create pixel-perfect assets through
  Pincel's command API, not by generating rastered fake-pixel-art.
- **Quality through validation**: validation functions act as a feedback
  loop. The AI iterates against measurable constraints (banding score,
  hue-shift presence, palette adherence) rather than aesthetic guesses.
- **Curriculum as reference**: lessons and their underlying principles are
  exposed as resources the AI consults before and during creation.
- **One core, many surfaces**: the same Rust `core` crate powers the editor,
  the lessons player, and the MCP server. No drift between human and AI
  outputs.
- **Workflow integration**: usable from Claude Code in any project,
  particularly Amigo Engine repos where assets land directly in `assets/`.

## Non-Goals

- Replacing human pixel artists. The MCP produces solid placeholder and
  prototype assets; hero art still benefits from human hands.
- General image generation. The MCP cannot produce non-pixel art; commands
  only operate on indexed bitmaps.
- A multi-tenant SaaS. Hosted deployment is for individual or small-team
  use, not a public service.
- Long-running training loops. The AI consults the curriculum at inference
  time; no fine-tuning involved.

## Mental Model

Think of the MCP as turning Pincel into a `cargo`-like tool for AI:

- **Resources** are documentation and reference (the curriculum, palettes,
  validation function descriptions, examples).
- **Tools** are commands that mutate canvas state, then export the result.
- **Validation** is the equivalent of `cargo check` — the AI runs it,
  reads the structured diagnostics, and iterates.

The AI's workflow becomes: read relevant lesson → plan → create canvas →
apply commands → validate → iterate → export. Same pattern as code agents
with compilers.

## Architecture

### Process Model

Two transport modes from the same Rust binary `pincel-mcp`:

| Mode | Use | Transport |
|---|---|---|
| `stdio` | Claude Desktop, Claude Code, local dev | stdin/stdout JSON-RPC per MCP spec |
| `http` | Hosted, Claude.ai, remote agents | HTTP + SSE per MCP spec |

```
pincel-mcp                  # default stdio
pincel-mcp --transport http --port 7711
```

### Code Location

```
packages/lessons-mcp/
  Cargo.toml
  src/
    main.rs                 # entrypoint, transport selection
    server.rs               # MCP protocol handling
    tools/
      canvas.rs             # canvas creation and pixel ops
      drawing.rs            # high-level drawing tools
      palette.rs            # palette management
      validation.rs         # validation runs
      export.rs             # .aseprite / .png / .ait export
      curriculum.rs         # lesson and reference queries
    resources/
      lessons.rs            # pincel://lessons/*
      palettes.rs           # pincel://palettes/*
      validations.rs        # pincel://validations/*
    state.rs                # session state (canvases by id)
    cdn_client.rs           # fetches curriculum from lessons.pincel.app
  worker/
    wrangler.toml           # Cloudflare Workers config for hosted
    src/
      worker.rs             # HTTP entry, calls into shared lib
```

The crate depends on `core` directly. Validation runs natively, not via
WASM. For Cloudflare Workers deployment, the same code compiles to WASM
and runs at the edge.

### Session State

The MCP server holds an in-memory `Session` per client connection:

```rust
pub struct Session {
    pub canvases: HashMap<CanvasId, Canvas>,
    pub active_canvas: Option<CanvasId>,
    pub workspace_root: Option<PathBuf>,  // for relative file ops
    pub locale: String,                   // for lesson content delivery
}
```

Canvases are created with `canvas.new` and persist for the session.
Multiple canvases allow side-by-side experimentation. `export` writes to
disk (stdio mode) or returns base64 (HTTP mode).

## Resources

Resources are read-only and addressable. AI agents enumerate them and
fetch on demand.

| URI Pattern | Description |
|---|---|
| `pincel://lessons/index` | full curriculum manifest with tree structure |
| `pincel://lessons/{path}/{id}` | single lesson, locale-resolved |
| `pincel://lessons/by-topic/{topic}` | lessons relevant to a topic (hue shifting, walking, etc.) |
| `pincel://palettes/index` | available palette IDs with attribution |
| `pincel://palettes/{id}` | single palette with colours and metadata |
| `pincel://validations/index` | all validation functions with descriptions |
| `pincel://validations/{name}` | single validation: signature, args, examples |
| `pincel://achievements/index` | achievement definitions (as reference for what "good" looks like) |
| `pincel://principles/{topic}` | distilled principles per topic, shorter than full lessons |

The `principles` resource is a convenience layer: each topic has a
condensed paragraph with the actionable rules, suitable for inclusion in
an AI's working context. Example: `pincel://principles/hue-shifting`
returns the key takeaways from the hue-shifting lesson without the
exercises and validation steps.

### Resource Hydration

The MCP server fetches from `https://lessons.pincel.app` and caches in
memory (and optionally on disk per `cli.md` config). New content available
on next session start, or via explicit `curriculum.refresh` tool call.

## Tools

Tools are grouped by namespace. Naming follows `namespace.verb_noun`.

### Canvas Lifecycle

| Tool | Args | Returns |
|---|---|---|
| `canvas.new` | `width`, `height`, `palette_id?` | `canvas_id` |
| `canvas.open` | `path` (`.aseprite` file) | `canvas_id` |
| `canvas.list` | — | list of `{id, width, height, layers, frames}` |
| `canvas.delete` | `canvas_id` | — |
| `canvas.set_active` | `canvas_id` | — |
| `canvas.info` | `canvas_id?` | full metadata |
| `canvas.preview` | `canvas_id?`, `frame?`, `scale?` | base64 PNG of current state |

`canvas.preview` is critical: the AI sends an image back to itself
(multimodal) to visually verify intermediate state. Without it the AI
draws blind.

### Drawing

Each tool takes `canvas_id?` (defaults to active), `layer_id?`, `frame_id?`.

| Tool | Args | Notes |
|---|---|---|
| `draw.pixel` | `x`, `y`, `color_idx` | single pixel |
| `draw.pixels` | array of `{x, y, color_idx}` | batched, single undo step |
| `draw.line` | `from`, `to`, `color_idx`, `pixel_perfect?` | Bresenham |
| `draw.rect` | `from`, `to`, `color_idx`, `filled` | |
| `draw.ellipse` | `center`, `radii`, `color_idx`, `filled` | midpoint |
| `draw.fill` | `seed_x`, `seed_y`, `color_idx`, `connectivity?` | flood fill |
| `draw.replace_color` | `from_idx`, `to_idx`, `region?` | |
| `draw.clear` | `region?` | |

### High-Level Drawing

Composite operations that capture pixel-art best practices in a single call:

| Tool | Description |
|---|---|
| `draw.shaded_sphere` | given a circle and 3 colour indices (mid, shadow, highlight), draws a shaded sphere with proper light source |
| `draw.outline` | adds a selective outline to existing pixels, optionally hue-shifted |
| `draw.tile_seamless` | helper to mirror edges for seamless tiling |
| `draw.ramp_pixel` | draw using a colour ramp, varying along an axis |

These are not magic — they are well-defined operations layered on the
primitive commands. They exist because they are common patterns the
curriculum teaches, and giving the AI them as first-class tools avoids
forcing it to re-derive the recipe every call.

### Layer and Frame

| Tool | Args |
|---|---|
| `layer.add` | `name`, `position?`, `opacity?`, `blend_mode?` |
| `layer.remove` | `layer_id` |
| `layer.set_visibility` | `layer_id`, `visible` |
| `layer.set_opacity` | `layer_id`, `opacity` |
| `layer.list` | — |
| `frame.add` | `position?`, `duration_ms?`, `clone_from?` |
| `frame.remove` | `frame_id` |
| `frame.set_duration` | `frame_id`, `duration_ms` |
| `frame.list` | — |

### Palette

| Tool | Args |
|---|---|
| `palette.load` | `palette_id` (from registry) or inline colours |
| `palette.set_color` | `index`, `rgba` |
| `palette.add_color` | `rgba`, returns new index |
| `palette.suggest_ramp` | base `color_idx`, `steps`, `style: cool_shadow / warm_shadow / neutral` — returns suggested ramp using hue-shifting principles from the curriculum |

`palette.suggest_ramp` is the curriculum reified as a tool. Given a base
hue, it produces a hue-shifted ramp following the rules from the
hue-shifting lesson. The AI calls it instead of guessing.

### Validation

| Tool | Args | Returns |
|---|---|---|
| `validate.run` | `canvas_id?`, `rules: [{fn, args}]` | structured results per rule |
| `validate.list_fns` | — | registry with signatures (same data as `pincel://validations/index`) |
| `validate.suggest_fixes` | `canvas_id?`, `failed_rule` | hint based on failure metrics |

`validate.run` returns the same `ValidationResult` shape used by the
editor: passed/failed, pixel coordinates of issues, metrics. The AI uses
these to plan corrective commands.

### Export

| Tool | Args | Returns |
|---|---|---|
| `export.aseprite` | `canvas_id?`, `path?` | path (stdio) or base64 (http) |
| `export.png` | `canvas_id?`, `frame?`, `scale?`, `path?` | path or base64 |
| `export.gif` | `canvas_id?`, `path?` | for animated canvases |
| `export.ait` | `canvas_id?`, `path?` | Amigo Engine runtime format |
| `export.spritesheet` | `canvas_id?`, `layout`, `path?` | tile/frame sheet |

In stdio mode with a writable workspace, exports write to disk and return
the path. In HTTP mode, they return base64-encoded bytes by default; an
optional `upload_url` arg supports PUT-to-storage flows.

### Curriculum

| Tool | Args | Returns |
|---|---|---|
| `curriculum.search` | `query`, `limit?` | matching lessons with snippets |
| `curriculum.get_principles` | `topic` | distilled principles for a topic |
| `curriculum.recommend_prereqs` | `goal` (e.g. "animate walk cycle") | ordered list of lesson IDs |
| `curriculum.refresh` | — | re-fetches manifest from CDN |

`curriculum.recommend_prereqs` is particularly useful for AI planning:
given a high-level goal, returns the chain of techniques needed, in order.

## Workflow Examples

### Generate a Slime Sprite

Conceptual call sequence the AI would issue:

1. `curriculum.get_principles(topic="first sprite")` → silhouette first,
   then base colour, then shading, then highlights
2. `palette.load(palette_id="endesga-32")` → returns palette
3. `canvas.new(width=16, height=16, palette_id="endesga-32")` → canvas_id
4. `draw.ellipse(center=[8,10], radii=[6,4], color_idx=mid_green, filled=true)`
   → silhouette
5. `canvas.preview()` → AI verifies shape
6. `palette.suggest_ramp(base=mid_green, steps=3, style=cool_shadow)`
   → ramp indices
7. `draw.shaded_sphere(...)` or manual `draw.pixels` with ramp
8. `validate.run(rules=[{fn: "has_hue_shift"}, {fn: "count_colors_used", op: "le", value: 8}])`
9. If failed: read metrics, plan fix, repeat
10. `export.aseprite(path="assets/enemies/slime.aseprite")`

### Generate an Animated Idle

1. `curriculum.recommend_prereqs(goal="idle animation")` → walk through
   lessons
2. `canvas.open(path="assets/enemies/slime.aseprite")` → existing sprite
3. `frame.add(clone_from=0, duration_ms=200)` → frame 1
4. `frame.add(clone_from=0, duration_ms=200)` → frame 2
5. `draw.shift_pixels(frame=1, dy=-1, region=top_half)` → squash motion
6. `canvas.preview(frame=0)`, `canvas.preview(frame=1)`, etc. → verify
7. `export.aseprite(path="assets/enemies/slime.aseprite")`

### Tile Set Generation

1. `canvas.new(width=16, height=16, palette_id="...")`
2. Draw tile contents using primitives
3. `validate.run(rules=[{fn: "tile_seamless", args: {edges: ["all"]}}])`
4. If edges don't match, `draw.tile_seamless(edge: "right")` to mirror
5. Iterate
6. `export.spritesheet(layout="4x4", path="assets/tiles/grass.png")`

## Quality Gates

The MCP supports **automatic validation policies** per session:

```rust
pub struct ValidationPolicy {
    pub auto_run_after_export: bool,
    pub block_export_on_failure: bool,
    pub default_rules: Vec<ValidationRule>,
}
```

Configured via a `policy.set` tool call early in the session. Default
policy enforces no banding, palette adherence, and minimum readability for
sprites under 32x32. The AI can lower the bar for prototype-quality work
via `policy.set({block_export_on_failure: false})`.

## Distribution

### NPM Package

`@amigo-labs/pincel-mcp` distributes the binary via `npx`:

```bash
npx @amigo-labs/pincel-mcp
```

Internally, the package downloads the platform-appropriate Rust binary
on first run (similar to `esbuild`, `swc`). Claude Desktop config:

```json
{
  "mcpServers": {
    "pincel": {
      "command": "npx",
      "args": ["@amigo-labs/pincel-mcp"]
    }
  }
}
```

### Hosted

`https://mcp.pincel.app` runs the same code on Cloudflare Workers.
Connection via the MCP HTTP transport:

```json
{
  "mcpServers": {
    "pincel": {
      "url": "https://mcp.pincel.app",
      "auth": "none"
    }
  }
}
```

The hosted version has no per-user state beyond a session — canvases
are session-scoped, exports return base64. No user accounts at v1.

### MCP Registry

Submit to the public MCP registry so the Anthropic app picker surfaces
Pincel when users ask for pixel-art help. Listing description emphasises
"create pixel art assets", not tutoring.

## Cargo Code Use Cases

The primary integration target is Claude Code in an Amigo Engine repo.
Workflow:

1. `.claude/config.toml` lists `pincel-mcp` as a stdio server
2. Developer asks Claude Code: "Add four enemy variants for the forest
   biome, 16x16, using the engine's standard palette"
3. Claude reads `curriculum.get_principles(topic="enemy design")`
4. Claude reads project's `assets/_palettes/forest.toml` from disk
5. Claude calls Pincel tools to create four canvases, draws them, runs
   validations, exports to `assets/enemies/`
6. Claude commits the `.aseprite` files

The validation step is what makes this trustworthy: the AI cannot ship
banded, off-palette, or unreadable sprites without explicitly disabling
the policy. Visible quality bar.

## Security and Sandboxing

The MCP server in stdio mode has access to the local filesystem within
the workspace root (default: CWD). Configurable via `--workspace <path>`.
Operations attempting to read or write outside the workspace are rejected
with `E007: path outside workspace`.

In HTTP mode, no filesystem access. Exports return inline data.

The server does not execute lesson-side code — lessons are pure data
references (TOML, Markdown). Validation functions are compiled Rust in
the `core` crate, not user-supplied code.

## Performance

Targets for stdio mode on a mid-range developer machine:

- Tool call round-trip: under 10 ms for canvas ops, under 50 ms for
  validation, under 200 ms for export
- Memory per session: under 50 MB with 10 active canvases up to 128x128
- Canvas operations scale linearly with affected pixel count

HTTP/Workers mode has roughly 50–100 ms additional latency from network
and edge cold start.

## Telemetry

Off by default. Opt-in via config:

```toml
[mcp.telemetry]
enabled = false
endpoint = "https://telemetry.pincel.app/mcp"
```

When enabled, anonymised tool-call counts and validation pass rates
inform curriculum and tool improvements. No canvas content, no file
paths, no user identifiers.

## Open Questions

1. **Multimodal canvas preview**: how does the AI receive `canvas.preview`
   output? Inline base64 image in the tool result, or a separate
   "resource" the AI then fetches? Spec'd as inline for now; revisit if
   bandwidth becomes a concern.
2. **Streaming long operations**: large canvases or batch exports may
   exceed reasonable tool timeouts. Add streaming progress events?
3. **Multi-canvas operations**: should there be tools like
   `canvas.composite_into(target, source, region)` for combining sprites?
   Common workflow but adds complexity; defer to v2.
4. **AI-authored lessons**: if Claude creates a particularly good sprite,
   should there be a one-shot tool to package the command stream as a
   draft lesson contribution? Compelling but premature.
5. **Cost of hosted MCP**: Cloudflare Workers pricing scales with
   requests. At what point does usage warrant rate limits or auth?
6. **Aseprite binary write**: Pincel's `.aseprite` writer needs to be
   feature-complete enough that AI-produced files open cleanly in
   Aseprite proper. Round-trip CI tests.

## Implementation Phases

### Phase 1 — Minimum Viable Pixel Production
- Stdio transport
- `canvas.new`, `canvas.preview`, `canvas.info`
- `draw.pixel`, `draw.pixels`, `draw.line`, `draw.rect`, `draw.fill`
- `palette.load`, `palette.list`
- `validate.run`, `validate.list_fns` with 5 initial validation fns
- `export.aseprite`, `export.png`
- NPM distribution for Claude Desktop / Claude Code

### Phase 2 — Curriculum Integration
- `pincel://lessons/*` resources
- `pincel://principles/*` resources
- `curriculum.search`, `curriculum.get_principles`
- CDN client with caching

### Phase 3 — High-Level Tools
- `draw.shaded_sphere`, `draw.outline`, `draw.tile_seamless`
- `palette.suggest_ramp`
- `validate.suggest_fixes`
- Validation policy enforcement

### Phase 4 — Animation
- Frame tools
- `export.gif`, `export.spritesheet`
- `draw.shift_pixels`

### Phase 5 — Hosted Deployment
- HTTP transport
- Cloudflare Workers deployment
- MCP Registry submission
- `mcp.pincel.app` domain

### Phase 6 — Engine Integration
- `.ait` export tested end-to-end with Amigo Engine
- Claude Code recipes documented for Amigo Engine repos
- Example: full asset set generation for Threadwalker biome

## References

- `canvas.md` — the underlying data model and command vocabulary
- `lessons.md` — the curriculum exposed as resources and principles
- `cli.md` — `pincel-mcp` binary follows CLI conventions for config and
  errors
- Model Context Protocol specification (external)
