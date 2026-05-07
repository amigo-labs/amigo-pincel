# Pincel — Pixel-Art Editor for Game Asset Creation

> **Status:** Draft, Specification v0.1
> **Type:** Living Document
> **Last updated:** 2026-05-07
> **Owner:** Daniel Rück / amigo-labs

---

## 1. Goals & Non-Goals

### 1.1 Goals

Pincel is a pixel-art editor at the feature level of Aseprite, focused on **game asset creation**. It ships as:

- A **standalone Progressive Web App** (PWA, installable, offline-capable)
- A **native desktop app** via Tauri (sharing the same UI codebase)

The two builds share one Svelte UI and one Rust core. The native build adds OS file-system integration via Tauri commands; the PWA uses File System Access API with download fallback.

### 1.2 Engine Interop Strategy

Pincel does **not** integrate as a library or plugin into `amigo-engine`. Instead, interop is achieved via the Aseprite file format:

- Pincel reads and writes `.aseprite` files
- `amigo_assets` already imports Aseprite files and supports hot-reloading
- The user opens a file in Pincel, edits it, saves it; the running engine instance reloads automatically

This keeps both projects fully decoupled. Pincel works for Amigo Engine users *and* for anyone else using the Aseprite ecosystem (Godot, LDtk, Unity, custom engines).

### 1.3 Non-Goals (Phase 1)

- No proprietary file format. `.aseprite` is the only source-of-truth format.
- No Lua scripting / Aseprite extension API compatibility.
- No plugin system.
- No collaborative / multi-user editing.
- No 3D, no vector graphics, no high-bit-depth (HDR) painting.
- No atlas / sprite-sheet packing — `amigo_assets` does this; Pincel only exports flat PNG sheets when explicitly requested.

---

## 2. Architecture Overview

### 2.1 Workspace Structure

```
amigo-pincel/
├── crates/
│   ├── pincel-core/             Rust library: document model, tools, codecs, compose()
│   ├── aseprite-writer/         Rust library: standalone, publishable, MIT/Apache-2.0
│   └── pincel-wasm/             Rust crate: wasm-bindgen layer, cdylib
├── ui/                          Svelte 5 + Vite + Tailwind 4 + shadcn-svelte
│   ├── src/
│   │   ├── lib/
│   │   │   ├── core/            wasm import + typed API wrappers
│   │   │   ├── render/          WebGPU + Canvas2D adapters
│   │   │   ├── tools/           Tool input handlers (mouse/touch → core)
│   │   │   ├── components/      Panels, dialogs, toolbar, timeline
│   │   │   └── stores/          Svelte 5 runes for app state
│   │   └── routes/
│   ├── static/
│   ├── service-worker.ts
│   └── manifest.webmanifest
├── src-tauri/                   Tauri 2 native shell, uses pincel-core directly
│   ├── src/
│   └── tauri.conf.json
├── docs/
│   └── specs/
│       └── pincel.md            ← this document
├── Cargo.toml                   Workspace root
└── README.md
```

The UI build is consumed by both PWA and Tauri. Tauri loads the same Vite output via its built-in dev server / bundled assets. Native-only features (true file-system access, OS dialogs, recent-files registry) are exposed as Tauri commands that the UI feature-detects at runtime.

### 2.2 High-Level Diagram

```
┌──────────────────────────────────────────────────────────────┐
│                      UI Layer (Svelte 5)                     │
│                                                              │
│  Toolbar │ Canvas Viewport │ Layers │ Timeline │ Palette │   │
│          │                 │ Tilesets │ Slices │             │
│          │                 │                                 │
│          ▼                                                   │
│  ┌───────────────┐         ┌──────────────────┐              │
│  │ Render Adapter│◄────────│ WASM API (typed) │              │
│  │ WebGPU/Canvas2│         └──────────────────┘              │
│  └───────────────┘                  │                        │
└─────────────────────────────────────┼────────────────────────┘
                                      ▼
┌──────────────────────────────────────────────────────────────┐
│                   pincel-core (Rust)                         │
│                                                              │
│  Document ── Layers (Image│Tilemap│Group)                    │
│           ── Frames ── Cels                                  │
│           ── Palette ── Tilesets ── Slices ── Tags           │
│                                                              │
│  Tools (Pencil, Bucket, Line, …) ──► Commands                │
│                                          │                   │
│  Command Bus ◄─────────────── Undo/Redo Stack                │
│                                                              │
│  compose(viewport, zoom) ──► RGBA Buffer                     │
│                                                              │
│  Codec ── aseprite-loader (read) ── aseprite-writer (write)  │
│        ── png export                                         │
└──────────────────────────────────────────────────────────────┘
```

### 2.3 Crate Responsibilities

**`pincel-core`** — pure Rust, `no_std`-compatible where possible (currently `std`-only, but no platform-specific I/O). Knows nothing about files-on-disk or rendering APIs. Codec functions take `Read`/`Write` traits. Render output is a `Vec<u8>` RGBA buffer plus metadata (dirty rect, viewport transform).

**`aseprite-writer`** — independent crate, designed to be published to crates.io alongside (and complementing) `aseprite-loader` / `asefile`. Implements the Aseprite v1.3 format spec for writing. Round-trip tests against `aseprite-loader` ensure compliance.

**`pincel-wasm`** — thin wrapper, exposes `pincel-core` to JS via `wasm-bindgen`. Owns the document state in Rust memory. JS calls methods, receives RGBA buffer views (zero-copy where possible) and event notifications.

**`ui/`** — Svelte 5 application. Imports `pincel-wasm` package built via `wasm-pack`. Uses Svelte 5 runes (`$state`, `$derived`, `$effect`) for reactive state. The render adapter translates document buffers into WebGPU textures or Canvas2D `ImageData` depending on capability detection.

**`src-tauri/`** — Tauri 2 shell. Depends on `pincel-core` directly (no WASM round-trip for native I/O). Exposes Tauri commands for native FS, OS dialogs, recent files, and "open with…" association.

---

## 3. Document Model

The Pincel document model is **isomorphic to the Aseprite v1.3 file format**. Every concept maps cleanly to a chunk type in `.aseprite`. This is a deliberate constraint: it guarantees round-trip fidelity and avoids a translation layer.

### 3.1 Top-Level Types

```rust
pub struct Sprite {
    pub width: u32,
    pub height: u32,
    pub color_mode: ColorMode,
    pub layers: Vec<Layer>,           // Z-order: index 0 is bottom
    pub frames: Vec<Frame>,           // sequential
    pub palette: Palette,
    pub tilesets: Vec<Tileset>,
    pub tags: Vec<Tag>,                // animation tags
    pub slices: Vec<Slice>,
    pub metadata: Metadata,
}

pub enum ColorMode {
    Rgba,                              // 32 bpp
    Indexed { transparent_index: u8 }, // 8 bpp + palette
    Grayscale,                         // 16 bpp (V+A) — Phase 2
}
```

### 3.2 Layers

```rust
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub kind: LayerKind,
    pub visible: bool,
    pub editable: bool,
    pub blend_mode: BlendMode,
    pub opacity: u8,                   // 0..=255
    pub parent: Option<LayerId>,       // for group nesting
}

pub enum LayerKind {
    Image,
    Tilemap { tileset_id: TilesetId },
    Group,
}
```

Cels are stored separately, keyed by `(LayerId, FrameIndex)`:

```rust
pub struct Cel {
    pub layer: LayerId,
    pub frame: FrameIndex,
    pub position: (i32, i32),          // top-left in sprite coords
    pub opacity: u8,
    pub data: CelData,
}

pub enum CelData {
    Image(PixelBuffer),                // RGBA8 or indexed
    Tilemap {
        grid_w: u32,
        grid_h: u32,
        tiles: Vec<TileRef>,
    },
    Linked(FrameIndex),                // points to another cel in same layer
}

pub struct TileRef {
    pub tile_id: u32,                  // index into Tileset
    pub flip_x: bool,
    pub flip_y: bool,
    pub rotate_90: bool,
}
```

`Linked` cels are an Aseprite optimization: identical content across frames is stored once. Pincel must preserve linkage on read and may optionally create links on write (Phase 1: write expanded only; Phase 2: link-on-write).

### 3.3 Frames

```rust
pub struct Frame {
    pub duration_ms: u16,              // per-frame, Aseprite native
}
```

Frame-level data is intentionally minimal. Per-frame cel data lives in the `Cel` map. Slice keys (per-frame slice geometry) are stored on the `Slice` itself, not the frame.

### 3.4 Tilesets

```rust
pub struct Tileset {
    pub id: TilesetId,
    pub name: String,
    pub tile_size: (u32, u32),         // e.g. (16, 16)
    pub tiles: Vec<TileImage>,         // tile 0 is the empty tile
    pub base_index: i32,               // typically 1; tile 0 = empty
    pub external_file: Option<PathRef>,// Aseprite supports shared tilesets
}

pub struct TileImage {
    pub pixels: PixelBuffer,
}
```

Aseprite tile 0 is conventionally the "empty" tile. Pincel keeps this convention to maintain compatibility.

### 3.5 Slices

Slices are named rectangles overlaid on the sprite, optionally per-frame-keyed.

```rust
pub struct Slice {
    pub id: SliceId,
    pub name: String,
    pub color: Rgba,                   // visual color in editor
    pub keys: Vec<SliceKey>,           // sorted by frame
}

pub struct SliceKey {
    pub frame: FrameIndex,             // first frame this key applies to
    pub bounds: Rect,
    pub center: Option<Rect>,          // 9-patch inner rect (optional)
    pub pivot: Option<(i32, i32)>,     // pivot point (optional)
}
```

A slice key applies from its `frame` until the next key's `frame`. This is the Aseprite semantics; do not reinvent it.

### 3.6 Tags

```rust
pub struct Tag {
    pub name: String,
    pub from: FrameIndex,
    pub to: FrameIndex,
    pub direction: TagDirection,
    pub color: Rgba,
    pub repeats: u16,                  // 0 = infinite
}

pub enum TagDirection {
    Forward,
    Reverse,
    Pingpong,
    PingpongReverse,
}
```

Tags are essential for `amigo_animation`'s state-machine consumption. The state machine identifies states by tag name; ensure tag names are validated as identifier-like in the editor (warn on whitespace/special chars).

### 3.7 Palette

```rust
pub struct Palette {
    pub colors: Vec<PaletteEntry>,     // up to 256 for indexed mode
}

pub struct PaletteEntry {
    pub rgba: Rgba,
    pub name: Option<String>,          // Aseprite supports named entries
}
```

---

## 4. Render Pipeline

### 4.1 The `compose()` Contract

`pincel-core` provides a single composition entry point:

```rust
pub fn compose(
    sprite: &Sprite,
    cels: &CelMap,
    request: &ComposeRequest,
) -> ComposeResult;

pub struct ComposeRequest {
    pub frame: FrameIndex,
    pub viewport: Rect,                // in sprite coordinates
    pub zoom: u32,                     // integer zoom, 1..=64
    pub onion_skin: Option<OnionSkin>,
    pub include_layers: LayerFilter,   // visible-only by default
    pub overlays: Overlays,            // grid, slices, selection marquee
    pub dirty_hint: Option<Rect>,      // for incremental repaint
}

pub struct ComposeResult {
    pub pixels: Vec<u8>,               // RGBA8, len = vp.w*vp.h*4*zoom²
    pub width: u32,                    // viewport.w * zoom
    pub height: u32,
    pub generation: u64,               // monotonic, for the UI to detect staleness
}
```

The function is **pure** with respect to inputs and **must not allocate per-pixel**. It uses pre-allocated scratch buffers stored on the document for compositing. Layer compositing applies blend modes in z-order, then the result is upscaled by zoom (nearest-neighbor) and overlays are drawn on top.

### 4.2 Compositing Strategy

- Convert indexed pixels to RGBA at composite time (palette lookup)
- Apply per-cel opacity and blend mode bottom-up
- Group layers fold their children into a temporary buffer, then blend that buffer into the parent
- Tilemap cels expand to image data via tileset lookup, with flip/rotate flags applied during expansion
- Onion skin renders previous frame at user-defined alpha (default 0.3) tinted red, next frame tinted blue

### 4.3 Dirty-Rect Optimization (Phase 1.5, not blocking MVP)

The document tracks dirty rectangles per-cel. `compose()` accepts a `dirty_hint` and returns only the changed region. The UI render adapter then uploads a sub-rect to the GPU texture instead of the full frame. Skip in initial MVP; revisit when 512×512+ canvases feel sluggish.

### 4.4 UI Render Adapter

The render adapter lives in TypeScript under `ui/src/lib/render/`. Two backends:

**WebGPU backend (`render/webgpu.ts`)**
- Capability detection: `'gpu' in navigator && await navigator.gpu.requestAdapter() !== null`
- Single full-screen quad pipeline, sampling a 2D texture
- Texture upload via `device.queue.writeTexture()` with the RGBA buffer view exposed by `pincel-wasm`
- For pixel-perfect rendering, core composes at the exact zoom (nearest-neighbor in CPU) and GPU just blits. This avoids subpixel sampling artifacts.

**Canvas2D backend (`render/canvas2d.ts`)**
- Used as fallback or when WebGPU is unavailable (Safari pre-GA, restrictive enterprise)
- `ImageData` from the RGBA buffer, drawn via `putImageData`
- Pan/zoom managed by core (same compose contract); Canvas2D does no scaling

A common interface abstracts both:

```typescript
interface CanvasRenderer {
  resize(w: number, h: number): void;
  draw(rgba: Uint8ClampedArray, w: number, h: number): void;
  destroy(): void;
}
```

The adapter chooses the backend at startup; users can force Canvas2D via a setting (debug toggle).

---

## 5. Tool System

### 5.1 Tool Trait

```rust
pub trait Tool {
    fn id(&self) -> ToolId;
    fn cursor(&self, ctx: &ToolContext) -> CursorShape;

    fn on_press(&mut self, pt: PixelPoint, mods: Mods, ctx: &mut ToolContext)
        -> Option<Command>;
    fn on_drag(&mut self, pt: PixelPoint, mods: Mods, ctx: &mut ToolContext)
        -> Option<Command>;
    fn on_release(&mut self, pt: PixelPoint, mods: Mods, ctx: &mut ToolContext)
        -> Option<Command>;
}
```

Tools never mutate the document directly. They emit `Command`s. The command bus applies them and pushes onto the undo stack.

### 5.2 Phase 1 Tool Set

| Tool | Behavior |
|------|----------|
| Pencil | Single-pixel line, brush-size variants 1–8, hold Shift = constrain to 0/45/90° |
| Eraser | Clears to transparent (RGBA) or transparent-index (Indexed) |
| Bucket | Paint-fill, contiguous (default) or all-matching, tolerance 0 in MVP |
| Line | Bresenham line between press and release |
| Rectangle | Outline + filled variants, hold Shift = square |
| Ellipse | Midpoint algorithm, outline + filled, hold Shift = circle |
| Eyedropper | Sets foreground color from canvas pixel; hold Alt with any tool for temporary eyedropper |
| Move | Pans canvas with space-drag; moves selection content if active |
| Selection (Rect) | Marquee selection with marching-ants overlay |
| Tilemap Stamp | In tilemap layer: places selected tile from tileset at grid cell |
| Slice Tool | Drags out a new slice rect, drag corners to resize, double-click for properties (name, 9-patch center, pivot) |

### 5.3 Tileset Editor Sub-Mode

When the active layer is a Tilemap, the UI exposes a Tileset Panel showing all tiles in the layer's tileset. Clicking a tile enters "edit tile" sub-mode: the canvas shows that single tile at high zoom, normal image tools (Pencil, Eraser, etc.) operate on it. Returning to the tilemap view auto-updates all instances of that tile in the layer.

Auto-tile-detection (Aseprite's "Auto" mode where painting on the tilemap layer at tile resolution either reuses or creates tiles) is **deferred to Phase 2**. Phase 1 supports manual tile placement and explicit tile editing.

---

## 6. Command / Undo System

### 6.1 Command Pattern

```rust
pub trait Command: Send {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap);
    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap);
    fn merge(&mut self, next: &Self) -> bool { false }
}
```

`merge` returns true if the next command of the same kind can be coalesced (e.g., consecutive pencil strokes within the same press-drag-release sequence become one undo entry).

### 6.2 History

Linear undo stack with a configurable cap (default 100 entries). No branching history in MVP. On document save, the history is **not** cleared — the user can undo across saves until app close. Optional: persist history into the `.aseprite` user-data chunk for cross-session undo (Phase 2).

### 6.3 Diff Granularity

Image-cel commands store **dirty-rect deltas** (the bounding box of changed pixels plus before/after pixel data for that rect), not full cel snapshots. This keeps memory bounded for large canvases. Tilemap commands store affected `(grid_x, grid_y) → (old_tile, new_tile)` lists.

---

## 7. File I/O

### 7.1 Aseprite Read

Use **`aseprite-loader`** (`docs.rs/aseprite-loader`) as the primary parser. Rationale: zero-copy, MIT/Apache, actively maintained, supports format v1.3 including tilemaps and slices.

A thin adapter in `pincel-core::codec::aseprite_read` translates parser output into Pincel's `Sprite` model. Unsupported chunks (custom user-data extensions, future Aseprite versions) are preserved as opaque blobs and round-tripped on save (`unknown_chunks: Vec<RawChunk>` on each carrier type).

### 7.2 Aseprite Write

Implemented in the **`aseprite-writer`** crate (see Section 8). `pincel-core::codec::aseprite_write` produces a byte stream consumable by `Write`.

### 7.3 PNG Export

Two modes:

- **Single frame**: current frame composited and saved as PNG
- **Sprite sheet**: all frames laid out in a grid (rows/cols configurable, packed left-to-right top-to-bottom by default)

Sprite-sheet export emits an optional sidecar JSON describing frame durations, tags, and slice rects. Format follows Aseprite's `--data` JSON output for ecosystem compatibility (LDtk, Phaser, etc. consume this format).

### 7.4 Engine Build Pipeline

Pincel does **not** export to `.ait` directly. The engine's existing `amigo_assets` pipeline reads `.aseprite` files and produces engine-internal formats. Pincel's only contract with the engine is: produce a valid `.aseprite` file at a known location.

---

## 8. Crate: `aseprite-writer`

A standalone, publishable Rust crate. Lives in the Pincel workspace but has no Pincel dependencies — it exposes the Aseprite types directly.

### 8.1 Goals

- Implement Aseprite v1.3 file format **writing**
- Spec source: <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>
- License: MIT OR Apache-2.0 (matches `aseprite-loader`)
- Round-trip tested against `aseprite-loader`: read file → re-write → read again → assert equivalent

### 8.2 API Sketch

```rust
pub struct AseFile { /* matches loader's data model */ }

pub fn write<W: Write>(file: &AseFile, out: &mut W) -> Result<(), WriteError>;

pub struct WriteOptions {
    pub compression: Compression,      // Zlib (default), None
    pub link_identical_cels: bool,     // optimize file size (Phase 2)
}
```

### 8.3 Required Chunks (Phase 1)

| Chunk | Hex | Purpose |
|-------|-----|---------|
| Old Palette | 0x0004 | Compatibility (legacy palette) |
| Layer | 0x2004 | Layer metadata |
| Cel | 0x2005 | Image and tilemap cel data |
| Color Profile | 0x2007 | sRGB declaration |
| External Files | 0x2008 | Reference external tilesets |
| Tags | 0x2018 | Animation tags |
| Palette | 0x2019 | Modern palette with names + alpha |
| Slice | 0x2022 | Slice rectangles + 9-patch + pivot |
| Tileset | 0x2023 | Tile pixel data |
| User Data | 0x2020 | Round-trip preservation |

Compression: Zlib for cel pixel data (matches Aseprite default). Use the `flate2` crate.

### 8.4 Test Strategy

- Fixture-based: a `tests/fixtures/` directory with diverse `.aseprite` files (RGBA, indexed, with tilemaps, with slices, with linked cels, with all blend modes)
- For each fixture: load with `aseprite-loader`, write with `aseprite-writer`, load again, assert structural equality
- Property-based fuzzing (via `proptest`) on randomly-generated documents for serialization robustness
- A separate test verifies the actual Aseprite application can open Pincel-written files (manual gate, run before each release)

---

## 9. UI Architecture

### 9.1 Stack

- **Svelte 5** with runes (`$state`, `$derived`, `$effect`) for reactivity
- **Vite** as bundler
- **Tailwind 4** for styling
- **shadcn-svelte** for components (copied into project, not a runtime dependency)
- **wasm-pack** to build `pincel-wasm` as an npm-importable package

### 9.2 Layout

```
┌────────────────────────────────────────────────────────────────┐
│  Top Bar: File │ Edit │ Sprite │ Layer │ Frame │ View │ Help   │
├──────┬──────────────────────────────────────────────┬──────────┤
│ Tool │                                              │  Layers  │
│ bar  │                                              ├──────────┤
│      │            Canvas Viewport                   │ Tilesets │
│      │                                              ├──────────┤
│      │           (WebGPU/Canvas2D)                  │  Slices  │
│      │                                              ├──────────┤
│      │                                              │ Palette  │
├──────┴──────────────────────────────────────────────┴──────────┤
│  Timeline: frames + tags + onion-skin controls                 │
├────────────────────────────────────────────────────────────────┤
│  Status: zoom % │ cursor coords │ active color │ doc size      │
└────────────────────────────────────────────────────────────────┘
```

Right-side panels are reorderable, collapsible, and dock-tabbable. Layout state persists per-document type.

### 9.3 WASM Boundary Contract

**State stays in Rust.** The UI never holds a serialized document. It calls thin methods:

```typescript
import init, { Document } from 'pincel-wasm';

const doc = Document.new(64, 64, ColorMode.Rgba);
doc.applyTool(toolId, { x, y, button, mods, phase });
const frame = doc.compose(viewport, zoom, options);
//  ^ frame.pixels is a Uint8ClampedArray view into WASM memory (zero-copy)
```

The UI pulls events via:

```typescript
const events = doc.drainEvents();
// events: layer-changed, palette-changed, undo-pushed, dirty-rect, …
```

`drainEvents()` is called every frame (RAF-driven). Stores update from events; views re-render via Svelte reactivity.

### 9.4 Input Handling

Mouse and pen input is captured on the canvas element with the full `PointerEvent` API (pressure, tilt, pointer-type). Touch input is supported with two-finger pan/zoom. Keyboard shortcuts are defined in a single `shortcuts.ts` map that mirrors Aseprite's defaults where reasonable.

---

## 10. PWA-Specific

### 10.1 Service Worker

`vite-plugin-pwa` with the `injectManifest` strategy. The service worker precaches the WASM bundle, UI assets, and Tailwind CSS. Documents are NOT cached by the service worker — they live in IndexedDB.

### 10.2 File System

- **Modern browsers (Chrome, Edge, Opera)**: File System Access API (`showOpenFilePicker`, `showSaveFilePicker`). User grants per-file or per-directory access; Pincel can save in place.
- **Firefox / Safari**: download fallback. Open via `<input type="file">`, save via `Blob` + anchor download.
- **Capability detection**: feature-flag the UI; show "Save" vs "Save As (download)" labels accordingly.

### 10.3 Persistence

- **IndexedDB**: autosave snapshots every 30 seconds, recent-files registry, settings/preferences
- Each document gets a stable UUID; autosave snapshots are keyed by `(doc_uuid, timestamp)`
- On open, Pincel checks for unsaved changes from a previous session and offers recovery

### 10.4 Threading

Initial MVP: single-threaded WASM. `compose()` runs on the main thread. Acceptable up to ~512×512 canvases at zoom ≤ 16.

Phase 2: move heavy operations (full-canvas compose, large-area paint-fill, filters) to a Web Worker via `comlink`. Requires `SharedArrayBuffer` for zero-copy texture upload, which requires COOP/COEP headers on the hosting. Document this in deployment notes.

---

## 11. Tauri-Specific

### 11.1 Stack

Tauri 2, WebView2 / WKWebView / WebKitGTK depending on platform. The bundled UI is the same Vite output as the PWA build; Tauri loads it from `dist/`.

### 11.2 Native Commands

Defined in `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
async fn open_file(path: String) -> Result<DocumentDto, String>;

#[tauri::command]
async fn save_file(path: String, doc: DocumentDto) -> Result<(), String>;

#[tauri::command]
async fn recent_files() -> Vec<RecentFile>;

#[tauri::command]
async fn show_open_dialog() -> Option<String>;

#[tauri::command]
async fn show_save_dialog(suggested_name: String) -> Option<String>;
```

The native build uses these instead of the File System Access API. `pincel-core` is a direct Cargo dependency of `src-tauri/`, so commands can call codec functions without WASM round-trip.

### 11.3 OS Integration

- File-association: register `.aseprite` and `.ase` (with user opt-in on first launch)
- Recent-files in OS jump lists (Windows) / dock menu (macOS)
- Native menu bar with shortcuts matching the in-app Top Bar

### 11.4 Capability Detection in UI

```typescript
const isTauri = '__TAURI__' in window;
const fs = isTauri ? tauriFsAdapter : webFsAdapter;
```

Single FS interface, two adapters. UI never branches on platform beyond this.

---

## 12. Engine Integration Path

No code-level integration. Workflow:

1. Developer runs `cargo run` on their Amigo Engine game
2. Developer opens `assets/sprites/player.aseprite` in Pincel
3. Edits, saves
4. `amigo_assets` hot-reload watcher detects file change
5. Engine reloads the sprite, animation state machine picks up new tag data, tilemap layer updates, slices update collision rects

This requires no Pincel awareness in the engine. The contract is the file format.

If hot-reload coupling needs to be tighter in the future (e.g., live-editing while the game runs in a window next to Pincel), the engine could expose a local WebSocket that Pincel pushes change-notifications to. Out of scope for Phase 1.

---

## 13. Phase 2 Scope

Documented here so they're not forgotten and so Phase 1 designs accommodate them.

### 13.1 Selection Tools

- Lasso (free-form polygon selection)
- Magic Wand (contiguous-color selection with tolerance)
- Selection arithmetic (add, subtract, intersect via modifiers)

### 13.2 Auto-Tilemap Mode

Painting on a tilemap layer at tile-grid resolution either reuses an existing matching tile or creates a new one. Detection: hash each (tile_w × tile_h) area after a stroke; compare against tileset; auto-link or auto-create.

### 13.3 Custom Brushes

User defines a brush from a selection rect. Brush is stored on the document (Aseprite user-data chunk) or globally (preferences). Pencil tool uses brush instead of single-pixel stamp.

### 13.4 Filters

- Outline (configurable thickness, inside/outside/both)
- Drop Shadow
- Hue/Saturation/Lightness shift
- Color replace

All implemented as commands that produce a new cel state, undoable.

### 13.5 Scripting

If scripting is added later, follow the **amigo-downloader plugin pattern**:

- **rquickjs** (QuickJS-NG bindings) as the embedded JS engine
- **SWC** for TypeScript transpilation at plugin-load time
- Plugins are TypeScript files declaring an entry point — for Pincel, this would be e.g. `transform(sprite, options) → sprite` for batch processing, or `export(sprite) → bytes` for custom export formats
- Heavy work stays in Rust; plugins are orchestration only — they call host APIs to do the actual pixel manipulation, mirroring how downloader plugins call host APIs for HTTP/parsing
- Host-API surface is small and explicit: `getLayer`, `getCel`, `setPixel`, `addLayer`, `addFrame`, etc. No raw memory access.

This is **explicitly not Aseprite-Lua-script-compatible**. Aseprite scripts use a Lua API; reimplementing that surface in JS is not a goal. Pincel scripting is a fresh, smaller API designed for the patterns Pincel users actually need (batch export, palette manipulation, asset-pipeline hooks).

Decision deferred until a concrete need arises. No scripting infrastructure ships in Phase 1.

### 13.6 Animation Enhancements

- Per-tag repeat counts UI
- Pingpong direction visualization on timeline
- Cel linking on write (file size optimization)
- Inbetweening / tweening helpers (Phase 3 maybe — not classic pixel-art workflow)

### 13.7 Collaborative / Cloud

Out of scope indefinitely. Local-first by design.

---

## 14. Open Questions

1. **Indexed-mode rendering performance**: composing indexed → RGBA per frame is acceptable up to N×N. What is N before we need a palette-LUT-on-GPU path? (Benchmark in Phase 1.)
2. **Linked-cel write strategy**: do we always expand on write (simpler, larger files) or detect identical cels and link (smaller files, more complex writer)? Defer; see Phase 2.
3. **PWA deployment target**: GitHub Pages? A subdomain on amigo-labs infrastructure? Affects COOP/COEP for SharedArrayBuffer.
4. **Tauri auto-update**: Tauri 2's update plugin requires a signed-update server. Do we run one, or push releases via GitHub-Releases-only and require manual update?
5. **Touch/pen UX on tablets**: iPad with Apple Pencil is a real target for pixel art. PWA-on-iPad gives us pen pressure via `PointerEvent`. Tauri-on-iPad doesn't exist (Tauri mobile is Android-first; iOS support is alpha). Decide whether iPad is "PWA only" officially.

---

## 15. Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-05-07 | `.aseprite` as source-of-truth format | Engine already imports it; ecosystem interop; no proprietary lock-in |
| 2026-05-07 | No engine code coupling | Hot-reload via file format is sufficient and cleaner |
| 2026-05-07 | UI: Svelte 5 + Tailwind 4 + shadcn-svelte | Component coverage without lock-in; matches amigo-downloader stack |
| 2026-05-07 | Render: WebGPU + Canvas2D fallback in TypeScript | Avoids wgpu-rs WASM bloat; native browser APIs; small adapter surface |
| 2026-05-07 | `aseprite-writer` as separate publishable crate | Gap in Rust ecosystem; useful beyond Pincel |
| 2026-05-07 | Tilemap and Slice support in Phase 1 | Critical for game-asset workflow with `amigo_tilemap` and 9-patch / collision use-cases |
| 2026-05-07 | Scripting deferred; if needed later, downloader pattern (rquickjs + SWC + TS) | Consistent with amigo-labs tooling; out-of-scope for Phase 1 |
| 2026-05-07 | Document state lives exclusively in Rust memory; canvas / WebGPU is render-target only | Avoids the architectural failure mode that forced Piskel into a multi-year rewrite (Piskel #1245). Browser canvas anti-fingerprinting (Brave today, others tomorrow) corrupts pixel readbacks; in-memory state is immune. |
| 2026-05-07 | `pincel-wasm` ships as npm-importable package with documented public API from Phase 1 | Embedding is a recurring Piskel community ask (#1229, #1246). Designing the boundary in early avoids retrofit. |

---

## 16. Phase 1 Milestones

| # | Milestone | Exit Criterion |
|---|-----------|----------------|
| M1 | `pincel-core` document model + commands + undo/redo, no I/O | Unit-tested command application + revert for image cels; CI green |
| M2 | `aseprite-loader` integration, read fixture set | All test fixtures load into Pincel model without loss |
| M3 | `aseprite-writer` MVP (image layers, palette, frames, tags) | Round-trip test passes for image-only documents |
| M4 | `pincel-wasm` + minimal Svelte UI: open file, view, pencil tool, save | Demo: open `.aseprite`, paint, save, reopen in Aseprite — file is valid |
| M5 | Full tool set (Section 5.2 except Tilemap Stamp + Slice Tool) | Each tool has interaction tests; UX feels native |
| M6 | Tilemap support: read/write, Tilemap layer, Stamp tool, Tileset Panel | Round-trip test for tilemap fixtures; `amigo_assets` loads Pincel-saved tilemap correctly |
| M7 | Slice support: read/write, Slice tool, 9-patch + pivot UI | Round-trip test; engine can read Pincel slices as collision rects |
| M8 | PWA polish: service worker, IndexedDB autosave, recent files | Lighthouse PWA score ≥ 90 |
| M9 | Tauri build: native FS, OS dialogs, menu bar, file association | Installable on Windows + macOS + Linux; opens `.aseprite` files via OS |
| M10 | Performance pass: dirty-rect compose, profiling | 256×256 sprite at zoom 32 maintains 60 fps on M1 / mid-tier Windows |

Estimated Phase 1 effort: 3–4 months solo with focused work; longer with the `aseprite-writer` work being more thorough than minimal.

---

## 17. Lessons from Prior Art (Piskel)

Piskel is the closest peer to Pincel: web-based, open-source, sprite-and-animation-focused. It has 12.4k GitHub stars, ran for ~14 years, and entered a multi-year modernization effort starting 2026. Reading its issue tracker is a gift — both for what to avoid and for what users have begged for over a decade.

### 17.1 Architectural Anti-Pattern: Canvas as Source of Truth

Piskel issue **#1245** ("Move Away from Canvas as Data Source") documents that Piskel uses `<canvas>` elements as the authoritative pixel store: layer compositing runs through canvas APIs, file imports go via canvas-draw-then-readback, exports re-render from canvas. This worked for 14 years until browser anti-fingerprinting started "nudging" canvas pixel values. Brave broke first; other browsers will follow. Piskel's stated solution direction is exactly the architecture Pincel ships with on day one:

- Pixel state in pure memory (`Vec<u8>`, owned by `pincel-core`)
- All compositing manual, in Rust
- Canvas / WebGPU treated as render-target only, never a data source

This is encoded in the `compose()` contract (Section 4) and is non-negotiable. **Never read pixels back from the canvas.** If a feature seems to require it, it's the wrong design.

### 17.2 Stack Validation

Piskel issue **#1246** ("Piskel modernization") describes the migration the Piskel team is currently running:

- Build system → Vite
- JS → TypeScript
- IIFEs / namespaces → ES modules
- E2E → Playwright
- Goal: ship as an importable npm library

This is precisely the Pincel stack as specified in Sections 9 and 11. Pincel starts where Piskel hopes to arrive after multi-year migration. No design changes needed; this is validation.

### 17.3 Recurring Feature Requests → Pincel Mappings

These have been requested for years in Piskel's tracker. Pincel should accommodate them by design, not as afterthoughts.

| Piskel Issue | Request | Pincel Approach |
|--------------|---------|-----------------|
| #1264 | Rotate / flip selection | Phase 2 selection ops; Aseprite has it, free with format compatibility |
| #1243 | Text tool | Phase 2; bitmap-font rendering, optional |
| #1234 | Bucket fill across multiple frames | First-class `FrameScope { Current, AllFrames, TagRange, Selection }` modifier on every applicable tool / command |
| #1232 | Better downscaling / resize quality | Configurable resampling: Nearest, Scale2x/3x/4x, hqx, RotSprite |
| #1229 | npm-package / embeddable | Section 15 decision; `pincel-wasm` is a documented public API from Phase 1 |
| #681 | JSON export | Section 7.3; sprite-sheet sidecar JSON in Aseprite `--data` format for ecosystem compat |
| #661 | CLI args (open file from terminal) | Tauri build supports this natively; UI receives `initial-file` event on launch |
| #636 | Pixel grid display option | Standard view toggle; on by default at zoom ≥ 8 |
| #562 | Bigger canvases | No artificial limit; document model is bounded only by available memory |
| #403 | Align to center | Sprite-level transform commands (rotate, flip, align, trim) |
| #402 | Tiling / seamless mode | Phase 2; Aseprite-style canvas wrap-around for seamless authoring |
| #658 | Better camera / pan ergonomics | Standard: space-drag, middle-click-drag, mouse-wheel-zoom-to-cursor |
| #762 | Better UI space usage | No persistent top-banner / account chrome; panels collapsible and dock-tabbable (Section 9.2) |

### 17.4 Mobile / Tablet Support

Piskel README states: *"There is no support for mobile."* This is a self-imposed limitation rooted in event-handling assumptions made in 2012. The pixel-art community on iPad with Apple Pencil is large and underserved.

Pincel's input layer uses the modern `PointerEvent` API end-to-end, capturing pen pressure, tilt, and pointer-type from day one (Section 9.4). Two-finger pan/zoom is part of the standard input handling, not a tablet special case. Pincel's PWA build is the reference target for iPad — Tauri-on-iPad is not a supported path (Tauri mobile is Android-first, iOS support is alpha).

This means Pincel should design its tool palette and shortcut layout to be **usable without keyboard modifiers**. Long-press for context menus, on-screen modifier keys (the Aseprite mobile pattern), pinch-to-zoom. Validate this in M5 (full tool set milestone).

### 17.5 Embeddability as a Phase-1 Concern

Repeated Piskel requests for "Piskel as a React component" (#1229) and "Piskel as a JS library" (#1246 stated goal) show this is a real recurring need. Game engines, level editors, and asset-pipeline tools want an embedded pixel editor, not a separate app.

`pincel-wasm` ships with a stable, documented public API from Phase 1 (M4 acceptance):

```typescript
// Public package surface
import { Pincel, ColorMode, ToolId } from '@amigo-labs/pincel';

const pincel = await Pincel.create({
  width: 64,
  height: 64,
  colorMode: ColorMode.Rgba,
  // ... options
});

pincel.mount(htmlElementForCanvas);
pincel.openFile(asepriteBytes);
const bytes = await pincel.saveAseprite();

pincel.on('change', () => {/* … */});
pincel.on('save', (bytes) => {/* … */});
```

The Pincel app itself is then a thin shell on top of this same package. This is a deliberate constraint: if the public API is sufficient to build the app, it's sufficient for embedders.

---

*End of specification v0.1. This is a living document — amend in place via PR; significant decisions go in Section 15.*
