---
id: canvas
title: Canvas Core
status: draft
version: 0.1.0
owner: daniel
created: 2026-05-13
last_updated: 2026-05-13
related:
  - lessons
  - replay
  - mcp
  - aseprite-io
---

# Canvas Core

## Overview

The Canvas is Pincel's foundational data structure: an indexed-colour bitmap
with layers, frames, and a palette. All editor operations, replays, validation,
and MCP tool calls go through the same command-based API. This spec defines
the data model, command vocabulary, undo/redo, layer composition, palette
handling, and export contracts.

`packages/core` owns this code. The editor app, the lessons player, the
`lessons-cli`, and the MCP server all consume it.

## Goals

- **Single source of truth for pixel operations**: one Rust implementation
  consumed natively (Tauri, CLI, MCP) and via WASM (PWA).
- **Indexed-colour first**: every pixel references a palette entry. RGBA only
  at the export boundary.
- **Command pattern throughout**: every state change is a serialisable command.
  Undo/redo, replays, and MCP tools share this surface.
- **Aseprite-compatible**: `.aseprite` is the canonical source-of-truth format.
  Lossless round-trip for the subset of features Pincel supports.
- **Deterministic**: the same command stream applied to the same initial state
  always produces the same canvas. Required for replays and tests.

## Non-Goals

- RGBA painting workflows (no soft brushes, no anti-aliased strokes).
- Vector primitives stored alongside pixels.
- Photoshop-style adjustment layers, masks, or blend-mode richness beyond
  what Aseprite supports.
- Real-time multi-user collaboration on a single canvas.

## Data Model

### Canvas

```rust
pub struct Canvas {
    pub id: CanvasId,
    pub width: u32,
    pub height: u32,
    pub palette: Palette,
    pub layers: Vec<Layer>,
    pub frames: Vec<Frame>,
    pub active_layer: LayerId,
    pub active_frame: FrameId,
    pub metadata: CanvasMetadata,
}
```

`CanvasMetadata` carries non-pixel state: tags, slices, user data, source
filename, and a `schema_version`.

### Layer

```rust
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: u8,                  // 0-255
    pub blend_mode: BlendMode,
    pub kind: LayerKind,
}

pub enum LayerKind {
    Image,
    Group { children: Vec<LayerId> },
    Reference { source: ReferenceSource }, // overlay for lessons
}
```

Reference layers are special: they hold a borrowed image (e.g. a lesson
reference) and never participate in saved exports unless explicitly flattened.

### Frame and Cel

```rust
pub struct Frame {
    pub id: FrameId,
    pub duration_ms: u32,
    pub cels: HashMap<LayerId, Cel>,
}

pub struct Cel {
    pub origin: (i32, i32),           // top-left in canvas coords
    pub pixels: IndexedBitmap,        // dense or RLE-compressed
}
```

A cel is one layer's contribution to one frame. Empty cels are omitted.
Origin allows cels smaller than the canvas for memory efficiency.

### Palette

```rust
pub struct Palette {
    pub id: Option<String>,           // "endesga-32", etc., if from registry
    pub colors: Vec<Color>,           // index 0 is typically transparent
    pub transparent_index: u8,
    pub attribution: Option<String>,
}

pub struct Color {
    pub r: u8, pub g: u8, pub b: u8, pub a: u8,
    pub name: Option<String>,
}
```

Maximum 256 palette entries. Index 0 conventionally transparent but
configurable. Palette changes are commands and undoable.

### Indexed Bitmap

Stored as `Vec<u8>` of palette indices, `width * height` long. RLE compression
applied on serialisation, not in-memory. The hot path for editing is direct
index access into a contiguous buffer.

## Coordinate System

- Origin `(0, 0)` is the **top-left** pixel.
- X increases right, Y increases down.
- Negative coordinates are valid for cel origins (cel may extend beyond
  canvas, gets clipped on render).
- All command coordinates are integers. There is no sub-pixel addressing.

## Command Pattern

Every state-mutating operation is a `Command`. Commands are pure data,
serialisable, and reversible.

```rust
pub trait Command: Serialize + DeserializeOwned + Send {
    fn apply(&self, canvas: &mut Canvas) -> Result<CommandEffect>;
    fn inverse(&self, canvas: &Canvas) -> Result<Self> where Self: Sized;
    fn op_name(&self) -> &'static str;
}
```

The `inverse` method produces the command that undoes the operation,
computed against the canvas state *before* application (so a `DrawPixel`'s
inverse captures the previous pixel value).

### Command Effect

`apply` returns a `CommandEffect` describing what changed:

```rust
pub struct CommandEffect {
    pub dirty_region: Option<Rect>,   // for partial redraw
    pub affected_layers: Vec<LayerId>,
    pub affected_frames: Vec<FrameId>,
    pub palette_changed: bool,
    pub structure_changed: bool,      // layer add/remove, etc.
}
```

The UI uses this to scope repaints. The replay system uses it to advance
its render state. The validation system uses it to invalidate cached metrics.

### Command Vocabulary

#### Pixel-level

| Command | Description |
|---|---|
| `DrawPixel` | set a single pixel |
| `DrawPixels` | batch of pixels (single command, single undo step) |
| `DrawLine` | Bresenham line, pixel-perfect option |
| `DrawRect` | filled or outlined rectangle |
| `DrawEllipse` | filled or outlined ellipse, midpoint algorithm |
| `FillArea` | flood-fill from seed, 4- or 8-connected |
| `ReplaceColor` | replace all pixels of index A with B in region |
| `ShiftPixels` | move pixels within a layer by (dx, dy) |
| `ClearRegion` | set region to transparent |

All pixel commands take a target `(LayerId, FrameId)` and operate on the
matching cel, creating it if absent.

#### Selection

| Command | Description |
|---|---|
| `SetSelection` | replace current selection with a mask |
| `AddToSelection` | union |
| `SubtractFromSelection` | difference |
| `InvertSelection` | within canvas bounds |
| `ClearSelection` | |

Selections are stored as bitmasks on the `Canvas`, not per layer. Operations
that take a selection respect it implicitly when present.

#### Layer

| Command | Description |
|---|---|
| `AddLayer` | with kind, position, properties |
| `RemoveLayer` | |
| `RenameLayer` | |
| `MoveLayer` | reorder in stack |
| `SetLayerVisibility` | |
| `SetLayerOpacity` | |
| `SetLayerBlendMode` | |
| `MergeLayers` | down or selected, lossy operation |
| `DuplicateLayer` | |

#### Frame / Animation

| Command | Description |
|---|---|
| `AddFrame` | insert at position, optionally cloning a source frame |
| `RemoveFrame` | |
| `MoveFrame` | reorder |
| `SetFrameDuration` | |
| `DuplicateFrame` | |

#### Palette

| Command | Description |
|---|---|
| `SetPaletteColor` | edit one entry; affects all pixels of that index |
| `AddPaletteColor` | append entry |
| `RemovePaletteColor` | remap affected pixels to a replacement index |
| `ReorderPalette` | with index remap table |
| `LoadPalette` | replace palette wholesale, with optional pixel remap |

#### Metadata

| Command | Description |
|---|---|
| `SetCanvasTag` | named region/frame range |
| `SetSlice` | named rectangle for 9-slice or sprite-sheet hints |

### Command IDs and Ordering

Each command has a monotonic `id: u64` assigned on dispatch. The history is
an ordered `Vec<RecordedCommand>`:

```rust
pub struct RecordedCommand {
    pub id: u64,
    pub timestamp_ms: u64,    // since canvas creation
    pub command: Command,
    pub inverse: Command,     // precomputed for fast undo
    pub effect: CommandEffect,
}
```

This is the **replay format** at the same time (see `lessons.md`). Commands
do not need to be re-derived — they were already pure data when applied.

## Undo / Redo

A linear history with a cursor:

```rust
pub struct History {
    commands: Vec<RecordedCommand>,
    cursor: usize,            // index of next command if redo
    limit: Option<usize>,     // soft cap, oldest dropped when exceeded
}
```

Undo applies `inverse` of `commands[cursor - 1]` and decrements. Redo
applies `command` of `commands[cursor]` and increments. New commands
truncate redo history.

### Compaction

Two strategies for keeping history bounded:

1. **Soft limit**: drop oldest beyond `limit`. Simple, default.
2. **Coalescing**: consecutive `DrawPixel` commands within N ms targeting
   adjacent pixels merge into `DrawPixels`. Reduces noise from drag-strokes
   without losing fidelity.

Coalescing happens at record time, not lazily, so replays stay aligned.

## Layer Composition

Compositing is bottom-up, with alpha blending per layer:

```
out = layer_below
for layer in layers (bottom to top, visible only):
    out = blend(out, layer.cel_for(active_frame), layer.opacity, layer.blend_mode)
```

Blend modes mirror Aseprite's set: `Normal`, `Multiply`, `Screen`, `Overlay`,
`Darken`, `Lighten`, `Addition`, `Subtract`, `Difference`. Indexed-colour
blending is approximate — converts to RGBA for the blend, then maps back
to the nearest palette entry. Lossy by nature; users compose mainly in
`Normal` mode for predictability.

## Rendering Output

The core does not render. It exposes:

```rust
pub fn composite(&self, frame: FrameId, target: &mut RgbaBuffer);
pub fn composite_layer(&self, layer: LayerId, frame: FrameId, target: &mut RgbaBuffer);
pub fn composite_region(&self, frame: FrameId, region: Rect, target: &mut RgbaBuffer);
```

The Svelte app uses these against a `<canvas>` 2D context. The MCP server
uses them for PNG export. The CLI uses them for preview generation.

## Export Targets

| Format | Use | Direction |
|---|---|---|
| `.aseprite` | source of truth | read + write |
| `.png` | sharing, web | write |
| `.gif` | animation preview | write |
| `.ait` | Amigo Engine runtime | write |
| `.replay.json` | stroke replays | write |

`.aseprite` round-trip is the compatibility contract: open, save, diff is
empty for any canvas Pincel originally created. Aseprite features Pincel
does not support (tilemaps, group nesting beyond N levels, etc.) are
preserved as opaque chunks when round-tripping a foreign file.

See `aseprite-io.md` for the binary format details.

## Selection Model

A selection is a single-channel bitmask:

```rust
pub struct Selection {
    pub mask: Bitmap,        // 1 bit per canvas pixel
    pub bounding_box: Rect,  // cached for fast iteration
}
```

Operations honor selections implicitly when present. The selection is
canvas-level, not layer-level, matching Aseprite's UX. A selection survives
command application until explicitly cleared or replaced.

## Performance Targets

- 256x256 canvas, 32 layers, 60 frames: under 4 MB resident.
- Single pixel draw: under 100 µs including command record.
- Full composite of 64x64 / 8 layers / 1 frame: under 2 ms on a mid-range laptop.
- Undo step: under 1 ms for any single command.
- Replay of 10 000 commands: under 100 ms cold, real-time playback warm.

These are budgets, not guarantees. CI runs micro-benchmarks against them.

## Concurrency

The canvas is `!Sync` by design — single-threaded mutation only. Background
work (composite to RGBA, palette analysis, validation) operates on
`Arc<CanvasSnapshot>`, a copy-on-write read-only view taken at a point in
time. Mutations on the live canvas do not affect in-flight snapshots.

## WASM Boundary

The PWA accesses the canvas through a thin wasm-bindgen layer:

- Commands cross the boundary as `serde_wasm_bindgen`-encoded values.
- Pixel buffers are exposed as `Uint8ClampedArray` views into WASM memory
  with explicit lifetime management — JS reads, never writes.
- Render output goes into a JS-owned `Uint8ClampedArray` that the WASM
  side writes into via a pre-shared pointer.

Goal: zero memcpy on the hot composite path.

## Open Questions

1. **Tilemap support**: Aseprite 1.3 added tilemaps as a first-class concept.
   Pincel-out-of-scope for v1 or include as a layer kind? Cost: significant
   command vocabulary additions, but tile-based games are a primary use case.
2. **Group layer recursion depth**: Aseprite allows arbitrary nesting.
   Suggested cap: 8 levels. UI legibility argument vs full compat.
3. **Reference layers in `.aseprite`**: Aseprite has a "reference layer"
   concept already. Map ours to that or keep separate?
4. **Command coalescing window**: 100 ms? 200 ms? Tune empirically.
5. **History limit default**: unlimited with memory pressure dropping, or
   fixed at e.g. 10 000 commands?

## References

- `lessons.md` — uses canvas + command stream for replays and validation
- `replay.md` — replay player built on the command history
- `mcp.md` — exposes canvas operations as MCP tools
- `aseprite-io.md` — binary format read/write
