//! WebAssembly bindings for Pincel.
//!
//! This crate is the `wasm-bindgen` / `wasm-pack` target that exposes the
//! `pincel-core` document model to JavaScript. The boundary contract lives
//! in `docs/specs/pincel.md` §9.3 (`Document.new`, `applyTool`, `compose`,
//! `drainEvents`) and the public-package surface in §17.5
//! (`Pincel.create`, `openFile`, `saveAseprite`, `on('change' | 'save')`).
//!
//! Phase 1 / CLAUDE.md M6 lands the surface incrementally. The current
//! cut covers `Document::new` (with a default-layer / cel bootstrap),
//! opening / saving Aseprite byte buffers, full-canvas
//! `Document::compose`, basic dimension getters, `applyTool` with a
//! Pencil routed through the command bus, JS-facing `undo` / `redo` /
//! `undoDepth` / `redoDepth`, and `drainEvents` driven by a bounded
//! ring buffer (M6.4).
//!
//! Errors cross the boundary as `Result<_, String>`; `wasm-bindgen` maps
//! `String` Errs to a thrown JS exception. This keeps the surface
//! testable on the host target — `JsError::new` panics outside of
//! `wasm32-unknown-unknown` because it imports JS-side machinery.

mod events;

pub use events::Event;

use events::EventQueue;
use pincel_core::{
    AddLayer, AddTileset, AsepriteReadOutput, Bus, Cel, CelData, CelMap, ColorMode, ComposeRequest,
    DrawEllipse, DrawLine, DrawRectangle, FillRegion, Frame, FrameIndex, Layer, LayerId, LayerKind,
    MoveSelectionContent, PixelBuffer, PlaceTile, Rect, Rgba, SetPixel, SetTilePixel, Sprite,
    TileRef, Tileset, TilesetId, compose, read_aseprite, write_aseprite,
};
use wasm_bindgen::prelude::*;

/// Identifier of the default layer that [`Document::new`] seeds.
const DEFAULT_LAYER_ID: LayerId = LayerId::new(0);

/// Owned Pincel document — the [`Sprite`] plus its detached cel store —
/// exposed as a JS class.
///
/// State lives entirely in Rust memory (spec §15, "canvas-as-source-of-
/// truth" anti-pattern). JS holds an opaque handle and pulls renders /
/// byte buffers across the boundary.
#[wasm_bindgen]
pub struct Document {
    sprite: Sprite,
    cels: CelMap,
    bus: Bus,
    events: EventQueue,
}

#[wasm_bindgen]
impl Document {
    /// Create an empty RGBA document with the given canvas dimensions.
    ///
    /// The fresh document is bootstrapped so it is paintable out of the
    /// box: one image layer named `"Layer 1"` (id `0`) and one
    /// transparent image cel sized to the canvas at frame `0`. The
    /// single 100 ms frame makes the round-trip through
    /// `aseprite-writer` / `aseprite-loader` produce a parseable file.
    ///
    /// Returns `Err(String)` when the sprite builder rejects the input
    /// — today the only failure mode is a zero `width` or `height`. The
    /// error string comes from `pincel_core`'s `Display` impl and is
    /// not part of the public JS contract.
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32) -> Result<Document, String> {
        let sprite = Sprite::builder(width, height)
            .color_mode(ColorMode::Rgba)
            .add_layer(Layer::image(DEFAULT_LAYER_ID, "Layer 1"))
            .add_frame(Frame::new(100))
            .build()
            .map_err(|e| format!("failed to build sprite: {e}"))?;
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            DEFAULT_LAYER_ID,
            FrameIndex::new(0),
            PixelBuffer::empty(width, height, ColorMode::Rgba),
        ));
        Ok(Self {
            sprite,
            cels,
            bus: Bus::new(),
            events: EventQueue::new(),
        })
    }

    /// Parse an Aseprite v1.3 byte stream into a [`Document`].
    ///
    /// Mirrors `pincel_core::read_aseprite`. Surfaced to JS as
    /// `Document.openAseprite(bytes)`.
    #[wasm_bindgen(js_name = openAseprite)]
    pub fn open_aseprite(bytes: &[u8]) -> Result<Document, String> {
        let AsepriteReadOutput { sprite, cels } =
            read_aseprite(bytes).map_err(|e| format!("failed to open Aseprite: {e}"))?;
        Ok(Self {
            sprite,
            cels,
            bus: Bus::new(),
            events: EventQueue::new(),
        })
    }

    /// Serialize this document to an Aseprite v1.3 byte vector.
    ///
    /// The returned `Box<[u8]>` is materialized as a freshly-allocated
    /// `Uint8Array` on the JS side. Round-trip parity with
    /// `aseprite-loader` is covered by the
    /// `pincel-core::codec::aseprite_codec` integration suite.
    #[wasm_bindgen(js_name = saveAseprite)]
    pub fn save_aseprite(&self) -> Result<Box<[u8]>, String> {
        let mut buf = Vec::new();
        write_aseprite(&self.sprite, &self.cels, &mut buf)
            .map_err(|e| format!("failed to save Aseprite: {e}"))?;
        Ok(buf.into_boxed_slice())
    }

    /// Canvas width in pixels.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.sprite.width
    }

    /// Canvas height in pixels.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.sprite.height
    }

    /// Number of layers in the document, including invisible ones.
    #[wasm_bindgen(getter, js_name = layerCount)]
    pub fn layer_count(&self) -> u32 {
        self.sprite.layers.len() as u32
    }

    /// Numeric id of the layer at z-index `index` (`0` = bottom-most).
    /// Errors when `index` is out of range.
    #[wasm_bindgen(js_name = layerIdAt)]
    pub fn layer_id_at(&self, index: u32) -> Result<u32, String> {
        self.sprite
            .layers
            .get(index as usize)
            .map(|l| l.id.0)
            .ok_or_else(|| format!("layer index {index} out of range"))
    }

    /// Display name of the named layer, or an empty string when
    /// `layer_id` is unknown. Pair with [`Document::layer_kind`] to
    /// detect missing layers.
    #[wasm_bindgen(js_name = layerName)]
    pub fn layer_name(&self, layer_id: u32) -> String {
        self.sprite
            .layer(LayerId::new(layer_id))
            .map(|l| l.name.clone())
            .unwrap_or_default()
    }

    /// Discriminant for the kind of layer at `layer_id`:
    /// `"image"`, `"tilemap"`, `"group"`, or `""` when unknown. Lets
    /// JS code decide whether a layer accepts pixel paint vs. tile
    /// placement.
    #[wasm_bindgen(js_name = layerKind)]
    pub fn layer_kind(&self, layer_id: u32) -> String {
        match self.sprite.layer(LayerId::new(layer_id)).map(|l| &l.kind) {
            Some(LayerKind::Image) => "image".to_string(),
            Some(LayerKind::Tilemap { .. }) => "tilemap".to_string(),
            Some(LayerKind::Group) => "group".to_string(),
            None => String::new(),
        }
    }

    /// Tileset id bound to the named tilemap layer. Returns `0` when
    /// `layer_id` is unknown or the layer is not a tilemap — callers
    /// should pair with [`Document::layer_kind`] to disambiguate the
    /// "tileset 0" case from "not a tilemap".
    #[wasm_bindgen(js_name = layerTilesetId)]
    pub fn layer_tileset_id(&self, layer_id: u32) -> u32 {
        match self.sprite.layer(LayerId::new(layer_id)).map(|l| &l.kind) {
            Some(LayerKind::Tilemap { tileset_id }) => tileset_id.0,
            _ => 0,
        }
    }

    /// Number of frames in the document.
    #[wasm_bindgen(getter, js_name = frameCount)]
    pub fn frame_count(&self) -> u32 {
        self.sprite.frames.len() as u32
    }

    /// Composite the requested frame at the given integer zoom over the
    /// full sprite canvas, with the default `Visible` layer filter and
    /// no overlays / onion skin.
    ///
    /// Mirrors `pincel_core::compose` with [`ComposeRequest::full`] —
    /// viewport, layer-filter, onion-skin, and overlay knobs land in a
    /// follow-up sub-task once the UI surfaces a need.
    ///
    /// `frame` is a 0-based frame index. `zoom` must be `1..=64`.
    /// Output dimensions are `width * zoom` × `height * zoom` and the
    /// pixel buffer is row-major non-premultiplied RGBA8.
    pub fn compose(&self, frame: u32, zoom: u32) -> Result<ComposeFrame, String> {
        let mut request = ComposeRequest::full(
            FrameIndex::new(frame),
            self.sprite.width,
            self.sprite.height,
        );
        request.zoom = zoom;
        let result = compose(&self.sprite, &self.cels, &request)
            .map_err(|e| format!("failed to compose: {e}"))?;
        Ok(ComposeFrame {
            width: result.width,
            height: result.height,
            pixels: result.pixels,
        })
    }

    /// Apply a tool action at sprite coordinates `(x, y)` with the
    /// given non-premultiplied RGBA color, routed through the command
    /// bus.
    ///
    /// `color` is `0xRRGGBBAA` (red in the high byte, alpha in the
    /// low byte). Supported `tool_id`s:
    ///
    /// - `"pencil"` — writes `color` to the target pixel.
    /// - `"eraser"` — writes fully transparent RGBA(0,0,0,0) to the
    ///   target pixel; the `color` argument is ignored (spec §5.2:
    ///   "Clears to transparent (RGBA) or transparent-index
    ///   (Indexed)").
    ///
    /// Both tools emit a [`SetPixel`] on the active layer / frame:
    /// today the active layer is the lowest-z `LayerKind::Image`
    /// layer (the bootstrapped `"Layer 1"` for fresh documents) and
    /// the active frame is `0`. Group / tilemap layers in an opened
    /// document are skipped so the user never paints into a layer
    /// that cannot accept pixels. Errors propagate from the
    /// underlying command (out-of-bounds pixel, missing cel, …).
    ///
    /// Spec §9.3 calls for a richer options struct (`button`, `mods`,
    /// `phase`, brush size). Positional args today; an options struct
    /// lands when more tools ship.
    #[wasm_bindgen(js_name = applyTool)]
    pub fn apply_tool(&mut self, tool_id: &str, x: i32, y: i32, color: u32) -> Result<(), String> {
        let rgba = match tool_id {
            "pencil" => Rgba {
                r: ((color >> 24) & 0xff) as u8,
                g: ((color >> 16) & 0xff) as u8,
                b: ((color >> 8) & 0xff) as u8,
                a: (color & 0xff) as u8,
            },
            "eraser" => Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
            _ => return Err(format!("unknown tool: {tool_id}")),
        };
        let layer = self
            .sprite
            .layers
            .iter()
            .find(|l| matches!(l.kind, LayerKind::Image))
            .ok_or_else(|| "document has no paintable image layer".to_string())?
            .id;
        let frame = FrameIndex::new(0);
        let cmd = SetPixel::new(layer, frame, x, y, rgba);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to apply {tool_id}: {e}"))?;
        self.events
            .push(Event::dirty_rect(layer.0, frame.0, x, y, 1, 1));
        Ok(())
    }

    /// Rasterize a 1-pixel-wide Bresenham line between sprite-space
    /// `(x0, y0)` and `(x1, y1)` with the given non-premultiplied RGBA
    /// color, routed through the command bus as a single
    /// [`DrawLine`](pincel_core::DrawLine).
    ///
    /// `color` is packed as `0xRRGGBBAA` (matching
    /// [`Self::apply_tool`]). The line targets the same active
    /// layer / frame as the pencil — today the lowest-z
    /// `LayerKind::Image` layer and frame `0`. Pixels that fall
    /// outside the target cel are skipped silently per the natural
    /// drawing-tool clipping semantics; only a missing image layer
    /// or a tilemap-only document surfaces as an error here.
    ///
    /// The emitted `dirty-rect` event covers the line's axis-aligned
    /// bounding box in sprite space. UI consumers that need pixel-
    /// exact dirty regions can replay Bresenham themselves.
    #[wasm_bindgen(js_name = applyLine)]
    pub fn apply_line(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: u32,
    ) -> Result<(), String> {
        let rgba = Rgba {
            r: ((color >> 24) & 0xff) as u8,
            g: ((color >> 16) & 0xff) as u8,
            b: ((color >> 8) & 0xff) as u8,
            a: (color & 0xff) as u8,
        };
        let layer = self
            .sprite
            .layers
            .iter()
            .find(|l| matches!(l.kind, LayerKind::Image))
            .ok_or_else(|| "document has no paintable image layer".to_string())?
            .id;
        let frame = FrameIndex::new(0);
        let cmd = DrawLine::new(layer, frame, x0, y0, x1, y1, rgba);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to apply line: {e}"))?;
        let bbox = endpoint_bbox(x0, y0, x1, y1);
        self.events.push(Event::dirty_rect(
            layer.0, frame.0, bbox.0, bbox.1, bbox.2, bbox.3,
        ));
        Ok(())
    }

    /// Rasterize an axis-aligned rectangle between sprite-space corners
    /// `(x0, y0)` and `(x1, y1)` with the given non-premultiplied RGBA
    /// color, routed through the command bus as a single
    /// [`DrawRectangle`](pincel_core::DrawRectangle).
    ///
    /// `fill == false` writes the 1-pixel border; `fill == true` writes
    /// every pixel in the interior (border included). `color` is packed
    /// as `0xRRGGBBAA` (matching [`Self::apply_tool`]). The command
    /// targets the same active layer / frame as the pencil — today the
    /// lowest-z `LayerKind::Image` layer and frame `0`. Pixels outside
    /// the target cel are skipped silently per the natural drawing-tool
    /// clipping semantics; only a missing image layer surfaces as an
    /// error here. Endpoint order does not matter — the underlying
    /// command normalizes to min / max corners before rasterizing.
    ///
    /// The emitted `dirty-rect` event covers the rectangle's
    /// axis-aligned bounding box in sprite space.
    #[wasm_bindgen(js_name = applyRectangle)]
    pub fn apply_rectangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: u32,
        fill: bool,
    ) -> Result<(), String> {
        let rgba = Rgba {
            r: ((color >> 24) & 0xff) as u8,
            g: ((color >> 16) & 0xff) as u8,
            b: ((color >> 8) & 0xff) as u8,
            a: (color & 0xff) as u8,
        };
        let layer = self
            .sprite
            .layers
            .iter()
            .find(|l| matches!(l.kind, LayerKind::Image))
            .ok_or_else(|| "document has no paintable image layer".to_string())?
            .id;
        let frame = FrameIndex::new(0);
        let cmd = DrawRectangle::new(layer, frame, (x0, y0), (x1, y1), fill, rgba);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to apply rectangle: {e}"))?;
        let bbox = endpoint_bbox(x0, y0, x1, y1);
        self.events.push(Event::dirty_rect(
            layer.0, frame.0, bbox.0, bbox.1, bbox.2, bbox.3,
        ));
        Ok(())
    }

    /// Rasterize the ellipse inscribed in the bbox of sprite-space
    /// corners `(x0, y0)` and `(x1, y1)` with the given non-premultiplied
    /// RGBA color, routed through the command bus as a single
    /// [`DrawEllipse`](pincel_core::DrawEllipse).
    ///
    /// `fill == false` walks the rim; `fill == true` emits the full
    /// disk (rim + interior). `color` is packed as `0xRRGGBBAA`
    /// (matching [`Self::apply_tool`]). The command targets the same
    /// active layer / frame as the pencil — today the lowest-z
    /// `LayerKind::Image` layer and frame `0`. Pixels outside the
    /// target cel are skipped silently per the natural drawing-tool
    /// clipping semantics; only a missing image layer surfaces as an
    /// error here. Endpoint order does not matter — the underlying
    /// command normalizes to min / max corners before rasterizing.
    ///
    /// The emitted `dirty-rect` event covers the bbox in sprite space.
    #[wasm_bindgen(js_name = applyEllipse)]
    pub fn apply_ellipse(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: u32,
        fill: bool,
    ) -> Result<(), String> {
        let rgba = Rgba {
            r: ((color >> 24) & 0xff) as u8,
            g: ((color >> 16) & 0xff) as u8,
            b: ((color >> 8) & 0xff) as u8,
            a: (color & 0xff) as u8,
        };
        let layer = self
            .sprite
            .layers
            .iter()
            .find(|l| matches!(l.kind, LayerKind::Image))
            .ok_or_else(|| "document has no paintable image layer".to_string())?
            .id;
        let frame = FrameIndex::new(0);
        let cmd = DrawEllipse::new(layer, frame, (x0, y0), (x1, y1), fill, rgba);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to apply ellipse: {e}"))?;
        let bbox = endpoint_bbox(x0, y0, x1, y1);
        self.events.push(Event::dirty_rect(
            layer.0, frame.0, bbox.0, bbox.1, bbox.2, bbox.3,
        ));
        Ok(())
    }

    /// Flood-fill the contiguous region of pixels matching the seed pixel
    /// at sprite-space `(x, y)` with the given non-premultiplied RGBA
    /// color, routed through the command bus as a single
    /// [`FillRegion`](pincel_core::FillRegion).
    ///
    /// The fill is 4-connected with tolerance 0 — only pixels that match
    /// the seed's exact RGBA are replaced. `color` is packed as
    /// `0xRRGGBBAA` (matching [`Self::apply_tool`]). The command targets
    /// the same active layer / frame as the pencil — today the lowest-z
    /// `LayerKind::Image` layer and frame `0`. Painting the seed color
    /// over itself, or seeding outside the target cel, leaves the pixel
    /// buffer unchanged but the command still joins the bus and emits
    /// `dirty-canvas` for undo-symmetry with the other paint tools.
    ///
    /// The emitted `dirty-canvas` event reflects that a bucket fill can
    /// affect any subset of the cel — the UI's RAF loop coalesces the
    /// event into a single recompose.
    #[wasm_bindgen(js_name = applyBucket)]
    pub fn apply_bucket(&mut self, x: i32, y: i32, color: u32) -> Result<(), String> {
        let rgba = Rgba {
            r: ((color >> 24) & 0xff) as u8,
            g: ((color >> 16) & 0xff) as u8,
            b: ((color >> 8) & 0xff) as u8,
            a: (color & 0xff) as u8,
        };
        let layer = self
            .sprite
            .layers
            .iter()
            .find(|l| matches!(l.kind, LayerKind::Image))
            .ok_or_else(|| "document has no paintable image layer".to_string())?
            .id;
        let frame = FrameIndex::new(0);
        let cmd = FillRegion::new(layer, frame, x, y, rgba);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to apply bucket: {e}"))?;
        self.events.push(Event::dirty_canvas());
        Ok(())
    }

    /// Sample the composited color at sprite coordinates `(x, y)` on the
    /// given frame. Returns the packed non-premultiplied RGBA8 value as
    /// `0xRRGGBBAA` (matching [`Self::apply_tool`]).
    ///
    /// Implements spec §5.2 — "Eyedropper: Sets foreground color from
    /// canvas pixel". The sample comes from `compose()` with the default
    /// `Visible` layer filter, so what the user sees is what they pick:
    /// hidden layers do not contribute, and transparent pixels yield
    /// `0x00000000`. Coordinates outside the sprite canvas are not
    /// rejected — they fall outside every cel's intersection and yield
    /// transparent, matching the natural read-only semantics.
    ///
    /// Errors propagate from `compose()`: unknown frame index,
    /// unsupported color mode (indexed / grayscale), etc.
    #[wasm_bindgen(js_name = pickColor)]
    pub fn pick_color(&self, frame: u32, x: i32, y: i32) -> Result<u32, String> {
        let mut request = ComposeRequest::full(
            FrameIndex::new(frame),
            self.sprite.width,
            self.sprite.height,
        );
        request.viewport = Rect::new(x, y, 1, 1);
        let result = compose(&self.sprite, &self.cels, &request)
            .map_err(|e| format!("failed to pick color: {e}"))?;
        debug_assert_eq!(result.pixels.len(), 4);
        Ok(u32::from_be_bytes([
            result.pixels[0],
            result.pixels[1],
            result.pixels[2],
            result.pixels[3],
        ]))
    }

    /// Revert the most recent command. Returns `true` if a command was
    /// undone, `false` when the undo stack was empty.
    ///
    /// On a successful undo a `dirty-canvas` event is enqueued so the
    /// UI re-renders. The WASM layer cannot yet attribute the reverted
    /// change to a single cel — per-command dirty rects land in M12
    /// (perf pass).
    pub fn undo(&mut self) -> bool {
        let undone = self.bus.undo(&mut self.sprite, &mut self.cels);
        if undone {
            self.events.push(Event::dirty_canvas());
        }
        undone
    }

    /// Re-apply the most recently undone command. Returns `true` if a
    /// command was redone. Errors propagate from the underlying
    /// command (e.g. a redo whose target cel was deleted).
    ///
    /// On a successful redo a `dirty-canvas` event is enqueued, same
    /// rationale as [`Document::undo`].
    pub fn redo(&mut self) -> Result<bool, String> {
        let redone = self
            .bus
            .redo(&mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to redo: {e}"))?;
        if redone {
            self.events.push(Event::dirty_canvas());
        }
        Ok(redone)
    }

    /// Number of commands available to undo.
    #[wasm_bindgen(getter, js_name = undoDepth)]
    pub fn undo_depth(&self) -> u32 {
        self.bus.undo_depth() as u32
    }

    /// Number of commands available to redo.
    #[wasm_bindgen(getter, js_name = redoDepth)]
    pub fn redo_depth(&self) -> u32 {
        self.bus.redo_depth() as u32
    }

    /// Drain queued events and return them in FIFO order. The internal
    /// buffer is cleared on every call.
    ///
    /// The UI is expected to call this once per RAF tick (spec §9.3).
    /// The buffer is bounded with drop-oldest semantics; a UI that
    /// stops draining (e.g. a backgrounded tab) cannot grow it without
    /// limit.
    #[wasm_bindgen(js_name = drainEvents)]
    pub fn drain_events(&mut self) -> Vec<Event> {
        self.events.drain()
    }

    /// Replace the active marquee selection with the given sprite-space
    /// rect. An empty rect (`width == 0` or `height == 0`) clears the
    /// selection instead of storing a degenerate marquee — matches
    /// [`pincel_core::Sprite::set_selection`] and the Aseprite
    /// "zero-width drag = no selection" affordance.
    ///
    /// Always enqueues a `selection-changed` event, even when the new
    /// rect matches the previous one — the UI's RAF coalescer collapses
    /// the duplicate, and the symmetric "every write emits" contract
    /// matches the other paint methods. Selection state is not part of
    /// the undo stack in the M7.8 slice (see spec §5.2 / STATUS.md).
    #[wasm_bindgen(js_name = setSelection)]
    pub fn set_selection(&mut self, x: i32, y: i32, width: u32, height: u32) {
        let rect = Rect::new(x, y, width, height);
        self.sprite.set_selection(rect);
        let event = match self.sprite.selection {
            Some(r) => Event::selection_changed(r.x, r.y, r.width, r.height),
            None => Event::selection_changed(0, 0, 0, 0),
        };
        self.events.push(event);
    }

    /// Drop the active marquee selection. No-op when no selection is
    /// active; always enqueues a `selection-changed` event with zeroed
    /// numeric fields so the UI can repaint without per-call state
    /// tracking.
    #[wasm_bindgen(js_name = clearSelection)]
    pub fn clear_selection(&mut self) {
        self.sprite.clear_selection();
        self.events.push(Event::selection_changed(0, 0, 0, 0));
    }

    /// `true` when a non-empty marquee selection is active. See
    /// [`pincel_core::Sprite::has_selection`].
    #[wasm_bindgen(getter, js_name = hasSelection)]
    pub fn has_selection(&self) -> bool {
        self.sprite.has_selection()
    }

    /// Sprite-space `x` of the active selection, or `0` when no
    /// selection is active. Pair with [`Self::has_selection`] to
    /// distinguish "selection at (0, 0)" from "no selection".
    #[wasm_bindgen(getter, js_name = selectionX)]
    pub fn selection_x(&self) -> i32 {
        self.sprite.selection.map_or(0, |r| r.x)
    }

    /// Sprite-space `y` of the active selection, or `0` when no
    /// selection is active.
    #[wasm_bindgen(getter, js_name = selectionY)]
    pub fn selection_y(&self) -> i32 {
        self.sprite.selection.map_or(0, |r| r.y)
    }

    /// Width of the active selection in pixels, or `0` when no
    /// selection is active.
    #[wasm_bindgen(getter, js_name = selectionWidth)]
    pub fn selection_width(&self) -> u32 {
        self.sprite.selection.map_or(0, |r| r.width)
    }

    /// Height of the active selection in pixels, or `0` when no
    /// selection is active.
    #[wasm_bindgen(getter, js_name = selectionHeight)]
    pub fn selection_height(&self) -> u32 {
        self.sprite.selection.map_or(0, |r| r.height)
    }

    /// Translate the pixels inside the active marquee selection by
    /// sprite-space `(delta_x, delta_y)`. Routed through the command
    /// bus as a single
    /// [`MoveSelectionContent`](pincel_core::MoveSelectionContent) so
    /// undo / redo restore both pixels and the selection rect.
    ///
    /// The command targets the same active layer / frame as the other
    /// paint surfaces — today the lowest-z `LayerKind::Image` layer and
    /// frame `0`. Source pixels are cleared to transparent, copied to
    /// the translated cel-local position (pixels whose destination
    /// falls outside the cel buffer are dropped — Phase 1 does not
    /// auto-grow), and `Sprite::selection` is updated to the
    /// translated rect.
    ///
    /// Emits both a `dirty-canvas` event (the move can affect any
    /// subset of the cel) and a `selection-changed` event with the
    /// translated rect so the UI repaints the marching ants at the
    /// new position. A `(0, 0)` delta is accepted and joins the
    /// undo bus so the UI can commit a no-move drag uniformly.
    /// Errors propagate from the command bus — missing selection,
    /// missing image layer, unsupported color mode, etc.
    #[wasm_bindgen(js_name = applyMoveSelection)]
    pub fn apply_move_selection(&mut self, delta_x: i32, delta_y: i32) -> Result<(), String> {
        let layer = self
            .sprite
            .layers
            .iter()
            .find(|l| matches!(l.kind, LayerKind::Image))
            .ok_or_else(|| "document has no paintable image layer".to_string())?
            .id;
        let frame = FrameIndex::new(0);
        let cmd = MoveSelectionContent::new(layer, frame, delta_x, delta_y);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to move selection: {e}"))?;
        self.events.push(Event::dirty_canvas());
        let event = match self.sprite.selection {
            Some(r) => Event::selection_changed(r.x, r.y, r.width, r.height),
            None => Event::selection_changed(0, 0, 0, 0),
        };
        self.events.push(event);
        Ok(())
    }

    // ---- M8.6: tilemap surface ----------------------------------------

    /// Number of tilesets in the document.
    #[wasm_bindgen(getter, js_name = tilesetCount)]
    pub fn tileset_count(&self) -> u32 {
        self.sprite.tilesets.len() as u32
    }

    /// Numeric id of the tileset at the given position in
    /// `Sprite::tilesets` (`0..tilesetCount`). Tileset ids are assigned
    /// by [`Document::add_tileset`] and survive the codec round-trip, so
    /// they may be non-contiguous after an `openAseprite`.
    ///
    /// Errors when `index` is out of range.
    #[wasm_bindgen(js_name = tilesetIdAt)]
    pub fn tileset_id_at(&self, index: u32) -> Result<u32, String> {
        self.sprite
            .tilesets
            .get(index as usize)
            .map(|t| t.id.0)
            .ok_or_else(|| format!("tileset index {index} out of range"))
    }

    /// Tile width of the named tileset, or `0` if `tileset_id` is
    /// unknown.
    #[wasm_bindgen(js_name = tilesetTileWidth)]
    pub fn tileset_tile_width(&self, tileset_id: u32) -> u32 {
        self.sprite
            .tileset(TilesetId::new(tileset_id))
            .map(|t| t.tile_size.0)
            .unwrap_or(0)
    }

    /// Tile height of the named tileset, or `0` if `tileset_id` is
    /// unknown.
    #[wasm_bindgen(js_name = tilesetTileHeight)]
    pub fn tileset_tile_height(&self, tileset_id: u32) -> u32 {
        self.sprite
            .tileset(TilesetId::new(tileset_id))
            .map(|t| t.tile_size.1)
            .unwrap_or(0)
    }

    /// Number of tile images stored in the named tileset. By Aseprite
    /// convention tile id `0` is the empty / transparent tile and is
    /// usually not stored, so a freshly created tileset starts at `0`.
    /// Returns `0` if `tileset_id` is unknown.
    #[wasm_bindgen(js_name = tilesetTileCount)]
    pub fn tileset_tile_count(&self, tileset_id: u32) -> u32 {
        self.sprite
            .tileset(TilesetId::new(tileset_id))
            .map(|t| t.tile_count() as u32)
            .unwrap_or(0)
    }

    /// Tileset display name. Returns an empty string when `tileset_id`
    /// is unknown — JS code should pair this with a
    /// [`Document::tileset_tile_width`] / `tileset_tile_height` lookup
    /// that returns `0` to detect missing tilesets.
    #[wasm_bindgen(js_name = tilesetName)]
    pub fn tileset_name(&self, tileset_id: u32) -> String {
        self.sprite
            .tileset(TilesetId::new(tileset_id))
            .map(|t| t.name.clone())
            .unwrap_or_default()
    }

    /// RGBA pixel bytes for `tile_id` inside the named tileset.
    ///
    /// The returned buffer is `tile_w * tile_h * 4` bytes of
    /// non-premultiplied RGBA8 in row-major order — the same layout
    /// [`Document::compose`] produces, so it feeds directly into a JS
    /// `ImageData` for canvas painting.
    ///
    /// Errors when `tileset_id` is unknown, when `tile_id` is past the
    /// end of the tileset's stored tiles, or when the tile is not RGBA
    /// (indexed tiles are Phase 2 and need palette resolution before
    /// they cross the boundary). Aseprite convention reserves tile id
    /// `0` as the implicit empty / transparent tile and may not store
    /// it explicitly — callers iterating thumbnails should drive their
    /// loop off [`Document::tileset_tile_count`].
    #[wasm_bindgen(js_name = tilePixels)]
    pub fn tile_pixels(&self, tileset_id: u32, tile_id: u32) -> Result<Vec<u8>, String> {
        let tileset = self
            .sprite
            .tileset(TilesetId::new(tileset_id))
            .ok_or_else(|| format!("unknown tileset id {tileset_id}"))?;
        let tile = tileset
            .tile(tile_id)
            .ok_or_else(|| format!("tile id {tile_id} out of range for tileset {tileset_id}"))?;
        if tile.pixels.color_mode != ColorMode::Rgba {
            return Err(format!(
                "tileset {tileset_id} is not RGBA (got {:?})",
                tile.pixels.color_mode
            ));
        }
        Ok(tile.pixels.data.clone())
    }

    /// Add a new tileset to the document. The id is chosen as
    /// `max(existing ids) + 1` (or `0` when no tilesets exist) so it is
    /// stable across runs and monotonic. Returns the assigned id.
    ///
    /// Routes through the [`AddTileset`] command so the operation joins
    /// the undo / redo bus. Tile-image content starts empty (Aseprite
    /// convention: tile id `0` is the implicit empty tile); per-tile
    /// edits land alongside the Tileset Editor in M8.7.
    ///
    /// Errors when `tile_w` or `tile_h` is zero, when the existing id
    /// space is exhausted (an existing tileset already uses `u32::MAX`),
    /// or when the command bus rejects the insert.
    #[wasm_bindgen(js_name = addTileset)]
    pub fn add_tileset(&mut self, name: &str, tile_w: u32, tile_h: u32) -> Result<u32, String> {
        if tile_w == 0 || tile_h == 0 {
            return Err(format!(
                "tile size must be non-zero (got {tile_w}x{tile_h})"
            ));
        }
        let new_id = match self.sprite.tilesets.iter().map(|t| t.id.0).max() {
            None => 0,
            // Detect id-space exhaustion explicitly so the caller sees a
            // clear error rather than a generic duplicate-id from the
            // command bus (`saturating_add(1)` on `u32::MAX` would
            // otherwise collide with the existing tileset).
            Some(u32::MAX) => return Err("tileset id space exhausted".to_string()),
            Some(m) => m + 1,
        };
        let tileset = Tileset::new(TilesetId::new(new_id), name, (tile_w, tile_h));
        let cmd = AddTileset::new(tileset);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to add tileset: {e}"))?;
        Ok(new_id)
    }

    /// Place tile `tile_id` at grid cell `(grid_x, grid_y)` of the
    /// tilemap cel on `(layer, frame)`.
    ///
    /// Flip / rotate flags are not yet surfaced; tiles are placed in
    /// canonical orientation. The UI can wire them up once M8.7 needs
    /// them.
    ///
    /// Emits a `dirty-rect` event covering the single grid cell in
    /// sprite space so the renderer can repaint just the affected
    /// tile rather than the full canvas. Errors propagate from the
    /// underlying [`PlaceTile`] command (missing cel, non-tilemap cel,
    /// out-of-bounds grid coords).
    #[wasm_bindgen(js_name = placeTile)]
    pub fn place_tile(
        &mut self,
        layer: u32,
        frame: u32,
        grid_x: u32,
        grid_y: u32,
        tile_id: u32,
    ) -> Result<(), String> {
        let layer_id = LayerId::new(layer);
        let frame_idx = FrameIndex::new(frame);
        // Resolve the parent layer's tile size before mutating so the
        // dirty-rect we emit always reflects the on-disk geometry, not
        // a post-mutation fiction.
        let tile_size = self.resolve_tile_size(layer_id)?;
        let cmd = PlaceTile::new(layer_id, frame_idx, grid_x, grid_y, TileRef::new(tile_id));
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to place tile: {e}"))?;
        let dirty_x = (grid_x as i64) * i64::from(tile_size.0);
        let dirty_y = (grid_y as i64) * i64::from(tile_size.1);
        // Clamp to i32 — sprite-space coordinates are i32 in the event
        // schema. An overflow here only happens for astronomically
        // large grids that can't be rendered anyway.
        let dirty_x = i32::try_from(dirty_x).unwrap_or(i32::MAX);
        let dirty_y = i32::try_from(dirty_y).unwrap_or(i32::MAX);
        self.events.push(Event::dirty_rect(
            layer,
            frame,
            dirty_x,
            dirty_y,
            tile_size.0,
            tile_size.1,
        ));
        Ok(())
    }

    /// Look up the `tile_size` of the tileset bound to `layer`. Used
    /// by [`Document::place_tile`] to size the emitted dirty-rect.
    fn resolve_tile_size(&self, layer: LayerId) -> Result<(u32, u32), String> {
        let layer_ref = self
            .sprite
            .layer(layer)
            .ok_or_else(|| format!("unknown layer id {layer:?}"))?;
        let tileset_id = match layer_ref.kind {
            LayerKind::Tilemap { tileset_id } => tileset_id,
            _ => return Err(format!("layer {} is not a tilemap layer", layer.0)),
        };
        self.sprite
            .tileset(tileset_id)
            .map(|t| t.tile_size)
            .ok_or_else(|| {
                format!(
                    "layer {} references unknown tileset {}",
                    layer.0, tileset_id.0
                )
            })
    }

    // ---- M8.7c: tilemap-layer creation -------------------------------

    /// Add a new tilemap layer bound to `tileset_id` and seed it with
    /// an empty tilemap cel on every existing frame, sized to the
    /// canvas (`grid_w = ceil(width / tile_w)`,
    /// `grid_h = ceil(height / tile_h)`). Returns the new layer's id.
    ///
    /// The layer is added on top of the existing stack via the
    /// [`AddLayer`] command so the operation joins the undo bus.
    /// Per-frame cels are inserted directly into the [`CelMap`]
    /// after the layer lands — they are not undoable in this slice,
    /// which is consistent with how [`Document::new`] bootstraps its
    /// default image cel.
    ///
    /// Errors when the tileset id is unknown, when the existing
    /// layer-id space is exhausted, or when the underlying command
    /// rejects the insert.
    #[wasm_bindgen(js_name = addTilemapLayer)]
    pub fn add_tilemap_layer(&mut self, name: &str, tileset_id: u32) -> Result<u32, String> {
        let ts = self
            .sprite
            .tileset(TilesetId::new(tileset_id))
            .ok_or_else(|| format!("unknown tileset id {tileset_id}"))?;
        let (tile_w, tile_h) = ts.tile_size;
        if tile_w == 0 || tile_h == 0 {
            return Err(format!(
                "tileset {tileset_id} has invalid tile size {tile_w}x{tile_h}"
            ));
        }
        let new_layer_id = match self.sprite.layers.iter().map(|l| l.id.0).max() {
            None => 0,
            Some(u32::MAX) => return Err("layer id space exhausted".to_string()),
            Some(m) => m + 1,
        };
        let grid_w = self.sprite.width.div_ceil(tile_w);
        let grid_h = self.sprite.height.div_ceil(tile_h);
        let frame_count = self.sprite.frames.len();
        let layer = Layer::tilemap(LayerId::new(new_layer_id), name, TilesetId::new(tileset_id));
        let cmd = AddLayer::on_top(layer);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to add tilemap layer: {e}"))?;
        let layer_id = LayerId::new(new_layer_id);
        let tile_count = (grid_w as usize) * (grid_h as usize);
        for f in 0..frame_count {
            let frame = FrameIndex::new(f as u32);
            self.cels.insert(Cel {
                layer: layer_id,
                frame,
                position: (0, 0),
                opacity: 255,
                data: CelData::Tilemap {
                    grid_w,
                    grid_h,
                    tiles: vec![TileRef::EMPTY; tile_count],
                },
            });
        }
        self.events.push(Event::dirty_canvas());
        Ok(new_layer_id)
    }

    // ---- M8.7d: per-tile pixel write ---------------------------------

    /// Write a single pixel into `tileset.tiles[tile_id].pixels`.
    /// Routed through the [`SetTilePixel`] command so the edit joins
    /// the undo bus.
    ///
    /// `color` is packed as `0xRRGGBBAA`. Coordinates are tile-local
    /// (`0..tile_w` × `0..tile_h`). Emits a `dirty-canvas` event so
    /// the UI repaints — per-tile dirty events land alongside the
    /// dirty-rect refinement in M12.
    ///
    /// Errors when the tileset id is unknown, the tile id is past the
    /// stored range, or the coordinates fall outside the tile.
    #[wasm_bindgen(js_name = setTilePixel)]
    pub fn set_tile_pixel(
        &mut self,
        tileset_id: u32,
        tile_id: u32,
        x: u32,
        y: u32,
        color: u32,
    ) -> Result<(), String> {
        let rgba = Rgba {
            r: ((color >> 24) & 0xff) as u8,
            g: ((color >> 16) & 0xff) as u8,
            b: ((color >> 8) & 0xff) as u8,
            a: (color & 0xff) as u8,
        };
        let cmd = SetTilePixel::new(TilesetId::new(tileset_id), tile_id, x, y, rgba);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to set tile pixel: {e}"))?;
        self.events.push(Event::dirty_canvas());
        Ok(())
    }

    /// Append an empty (transparent) tile image to the named tileset
    /// and return the new tile id. The tile is sized to the tileset's
    /// `tile_size` and starts fully transparent RGBA so a fresh tile
    /// can be edited via [`Document::set_tile_pixel`] immediately
    /// without first crossing the boundary again to learn the size.
    ///
    /// Not undoable in this slice — tile-image insertion is treated
    /// like [`Document::new`]'s bootstrap cels: the editing commands
    /// targeting the tile join the bus, the container does not.
    /// Errors when the tileset id is unknown.
    #[wasm_bindgen(js_name = addTile)]
    pub fn add_tile(&mut self, tileset_id: u32) -> Result<u32, String> {
        let tileset = self
            .sprite
            .tilesets
            .iter_mut()
            .find(|t| t.id.0 == tileset_id)
            .ok_or_else(|| format!("unknown tileset id {tileset_id}"))?;
        let (tile_w, tile_h) = tileset.tile_size;
        let new_id = tileset.tiles.len() as u32;
        tileset.tiles.push(pincel_core::TileImage {
            pixels: PixelBuffer::empty(tile_w, tile_h, ColorMode::Rgba),
        });
        self.events.push(Event::dirty_canvas());
        Ok(new_id)
    }
}

/// Axis-aligned bounding box of two sprite-space points (a line segment
/// or rectangle defined by opposite corners). Returns
/// `(x, y, width, height)` with `width >= 1` and `height >= 1`.
///
/// For endpoint pairs whose span exceeds `u32::MAX` (e.g. `i32::MIN` to
/// `i32::MAX`), the width / height are saturated to `u32::MAX` so the
/// emitted event still satisfies the documented `>= 1` invariant.
fn endpoint_bbox(x0: i32, y0: i32, x1: i32, y1: i32) -> (i32, i32, u32, u32) {
    let min_x = x0.min(x1);
    let min_y = y0.min(y1);
    let max_x = x0.max(x1);
    let max_y = y0.max(y1);
    let w = u32::try_from(i64::from(max_x) - i64::from(min_x) + 1).unwrap_or(u32::MAX);
    let h = u32::try_from(i64::from(max_y) - i64::from(min_y) + 1).unwrap_or(u32::MAX);
    (min_x, min_y, w, h)
}

/// A single composited frame returned to JS.
///
/// `pixels` is `width * height * 4` non-premultiplied RGBA8 bytes in
/// row-major order. Today the `pixels` getter copies the buffer into a
/// fresh `Uint8Array`; spec §9.3 calls for a zero-copy
/// `Uint8ClampedArray` view of WASM memory, which lands once the
/// `js-sys` integration is wired up (M6 follow-up).
#[wasm_bindgen]
pub struct ComposeFrame {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[wasm_bindgen]
impl ComposeFrame {
    /// Output buffer width in pixels (`viewport.width * zoom`).
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Output buffer height in pixels (`viewport.height * zoom`).
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Fresh `Uint8Array` copy of the RGBA8 pixel buffer.
    #[wasm_bindgen(getter)]
    pub fn pixels(&self) -> Box<[u8]> {
        self.pixels.clone().into_boxed_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_bootstraps_one_layer_one_frame() {
        let doc = Document::new(64, 48).expect("non-zero dims build");
        assert_eq!(doc.width(), 64);
        assert_eq!(doc.height(), 48);
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.frame_count(), 1);
    }

    #[test]
    fn new_rejects_zero_width() {
        assert!(Document::new(0, 16).is_err());
    }

    #[test]
    fn new_rejects_zero_height() {
        assert!(Document::new(16, 0).is_err());
    }

    #[test]
    fn save_then_open_roundtrips_a_fresh_document() {
        let doc = Document::new(8, 8).expect("dims");
        let bytes = doc.save_aseprite().expect("save ok");
        let reopened = Document::open_aseprite(&bytes).expect("open ok");
        assert_eq!(reopened.width(), 8);
        assert_eq!(reopened.height(), 8);
        assert_eq!(reopened.layer_count(), 1);
        assert_eq!(reopened.frame_count(), 1);
    }

    #[test]
    fn open_aseprite_rejects_garbage_bytes() {
        let result = Document::open_aseprite(&[0, 1, 2, 3]);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("garbage bytes should not parse as a valid Aseprite file"),
        };
        assert!(!err.is_empty());
    }

    #[test]
    fn compose_fresh_document_yields_transparent_canvas() {
        let doc = Document::new(4, 3).expect("dims");
        let frame = doc.compose(0, 1).expect("compose ok");
        assert_eq!(frame.width(), 4);
        assert_eq!(frame.height(), 3);
        let pixels = frame.pixels();
        assert_eq!(pixels.len(), 4 * 3 * 4);
        assert!(pixels.iter().all(|&b| b == 0));
    }

    #[test]
    fn compose_honors_integer_zoom() {
        let doc = Document::new(2, 2).expect("dims");
        let frame = doc.compose(0, 4).expect("compose ok");
        assert_eq!(frame.width(), 8);
        assert_eq!(frame.height(), 8);
        assert_eq!(frame.pixels().len(), 8 * 8 * 4);
    }

    #[test]
    fn compose_rejects_unknown_frame() {
        let doc = Document::new(2, 2).expect("dims");
        assert!(doc.compose(7, 1).is_err());
    }

    #[test]
    fn compose_rejects_zoom_zero() {
        let doc = Document::new(2, 2).expect("dims");
        assert!(doc.compose(0, 0).is_err());
    }

    #[test]
    fn compose_rejects_zoom_above_max() {
        let doc = Document::new(2, 2).expect("dims");
        assert!(doc.compose(0, 65).is_err());
    }

    fn pixel_at(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let off = ((y * width + x) * 4) as usize;
        [
            pixels[off],
            pixels[off + 1],
            pixels[off + 2],
            pixels[off + 3],
        ]
    }

    #[test]
    fn apply_tool_pencil_writes_pixel_into_default_cel() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_tool("pencil", 1, 2, 0xff0000ff)
            .expect("pencil ok");
        let frame = doc.compose(0, 1).expect("compose ok");
        let pixels = frame.pixels();
        assert_eq!(pixel_at(&pixels, 4, 1, 2), [255, 0, 0, 255]);
        assert_eq!(doc.bus.undo_depth(), 1);
    }

    #[test]
    fn apply_tool_pencil_runs_through_bus_for_undo() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_tool("pencil", 0, 0, 0x0a141eff)
            .expect("pencil ok");
        doc.apply_tool("pencil", 1, 0, 0x28323cff)
            .expect("pencil ok");
        assert_eq!(doc.bus.undo_depth(), 2);
        assert!(doc.bus.undo(&mut doc.sprite, &mut doc.cels));
        let frame = doc.compose(0, 1).expect("compose ok");
        let pixels = frame.pixels();
        assert_eq!(pixel_at(&pixels, 4, 1, 0), [0, 0, 0, 0]);
        assert_eq!(pixel_at(&pixels, 4, 0, 0), [10, 20, 30, 255]);
    }

    #[test]
    fn apply_tool_rejects_unknown_tool() {
        let mut doc = Document::new(2, 2).expect("dims");
        let err = doc.apply_tool("paintbrush", 0, 0, 0x000000ff).unwrap_err();
        assert!(err.contains("paintbrush"));
    }

    #[test]
    fn apply_tool_pencil_rejects_out_of_bounds_pixel() {
        let mut doc = Document::new(2, 2).expect("dims");
        assert!(doc.apply_tool("pencil", 10, 10, 0x000000ff).is_err());
    }

    #[test]
    fn apply_tool_eraser_clears_a_previously_painted_pixel() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_tool("pencil", 2, 1, 0xff0000ff)
            .expect("pencil ok");
        doc.apply_tool("eraser", 2, 1, 0x00000000)
            .expect("eraser ok");
        let frame = doc.compose(0, 1).expect("compose ok");
        let pixels = frame.pixels();
        assert_eq!(pixel_at(&pixels, 4, 2, 1), [0, 0, 0, 0]);
        // The eraser is its own command, so it joins the bus.
        assert_eq!(doc.bus.undo_depth(), 2);
    }

    #[test]
    fn apply_tool_eraser_ignores_the_color_argument() {
        // The eraser always writes transparent regardless of `color`,
        // so a non-zero color argument must not surface in the cel.
        let mut doc = Document::new(2, 2).expect("dims");
        doc.apply_tool("eraser", 0, 0, 0xff00ffff)
            .expect("eraser ok");
        let frame = doc.compose(0, 1).expect("compose ok");
        assert_eq!(pixel_at(&frame.pixels(), 2, 0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn apply_tool_eraser_emits_dirty_rect() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_tool("eraser", 1, 2, 0x00000000)
            .expect("eraser ok");
        let events = doc.drain_events();
        assert_eq!(events.len(), 1);
        let ev = events[0];
        assert_eq!(ev.kind(), "dirty-rect");
        assert_eq!(ev.x(), 1);
        assert_eq!(ev.y(), 2);
        assert_eq!(ev.width(), 1);
        assert_eq!(ev.height(), 1);
    }

    #[test]
    fn apply_tool_eraser_rejects_out_of_bounds_pixel() {
        let mut doc = Document::new(2, 2).expect("dims");
        assert!(doc.apply_tool("eraser", 10, 10, 0x00000000).is_err());
    }

    #[test]
    fn pick_color_returns_painted_pixel() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_tool("pencil", 2, 1, 0x80aaffff)
            .expect("pencil ok");
        let color = doc.pick_color(0, 2, 1).expect("pick ok");
        assert_eq!(color, 0x80aaffff);
    }

    #[test]
    fn pick_color_returns_zero_on_transparent_pixel() {
        let doc = Document::new(4, 4).expect("dims");
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick ok"), 0);
    }

    #[test]
    fn pick_color_outside_canvas_returns_transparent() {
        // Out-of-canvas reads are well-defined per spec §4.1 (cels
        // clipped to the viewport intersection); the eyedropper
        // surfaces that as transparent rather than an error.
        let doc = Document::new(2, 2).expect("dims");
        assert_eq!(doc.pick_color(0, -5, -5).expect("pick ok"), 0);
        assert_eq!(doc.pick_color(0, 99, 99).expect("pick ok"), 0);
    }

    #[test]
    fn pick_color_rejects_unknown_frame() {
        let doc = Document::new(2, 2).expect("dims");
        assert!(doc.pick_color(7, 0, 0).is_err());
    }

    #[test]
    fn pick_color_does_not_disturb_command_bus() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_tool("pencil", 0, 0, 0x123456ff)
            .expect("pencil ok");
        let depth_before = doc.bus.undo_depth();
        let _ = doc.pick_color(0, 0, 0).expect("pick ok");
        assert_eq!(doc.bus.undo_depth(), depth_before);
    }

    #[test]
    fn apply_tool_pencil_skips_group_layer_to_find_paintable_image() {
        // `CelData` is already brought in by `super::*` from the
        // top-level import.
        let mut doc = Document::new(2, 2).expect("dims");
        doc.sprite
            .layers
            .insert(0, Layer::group(LayerId::new(99), "folder"));
        doc.apply_tool("pencil", 1, 1, 0xff7f00ff)
            .expect("paints into the image layer behind the group");
        // Compose would reject the group layer (M3 image-only), so read
        // the bootstrapped image cel directly to confirm the paint
        // landed on the right layer.
        let cel = doc
            .cels
            .get(DEFAULT_LAYER_ID, FrameIndex::new(0))
            .expect("default cel still present");
        let CelData::Image(buf) = &cel.data else {
            panic!("expected image cel");
        };
        let off = ((buf.width + 1) * 4) as usize;
        assert_eq!(&buf.data[off..off + 4], &[255, 127, 0, 255]);
    }

    #[test]
    fn apply_tool_pencil_errors_when_no_image_layer_exists() {
        let mut doc = Document::new(2, 2).expect("dims");
        doc.sprite.layers.clear();
        doc.sprite
            .layers
            .push(Layer::group(LayerId::new(7), "folder"));
        let err = doc
            .apply_tool("pencil", 0, 0, 0x000000ff)
            .expect_err("group-only doc has nothing to paint");
        assert!(err.contains("no paintable image layer"));
    }

    #[test]
    fn drain_events_is_empty_on_a_fresh_document() {
        let mut doc = Document::new(4, 4).expect("dims");
        assert!(doc.drain_events().is_empty());
    }

    #[test]
    fn apply_tool_pencil_emits_dirty_rect_for_painted_pixel() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_tool("pencil", 1, 2, 0xff0000ff)
            .expect("pencil ok");
        let events = doc.drain_events();
        assert_eq!(events.len(), 1);
        let ev = events[0];
        assert_eq!(ev.kind(), "dirty-rect");
        assert_eq!(ev.layer(), 0);
        assert_eq!(ev.frame(), 0);
        assert_eq!(ev.x(), 1);
        assert_eq!(ev.y(), 2);
        assert_eq!(ev.width(), 1);
        assert_eq!(ev.height(), 1);
        assert!(doc.drain_events().is_empty());
    }

    #[test]
    fn apply_tool_failure_does_not_emit_an_event() {
        let mut doc = Document::new(2, 2).expect("dims");
        assert!(doc.apply_tool("pencil", 10, 10, 0x000000ff).is_err());
        assert!(doc.drain_events().is_empty());
    }

    #[test]
    fn undo_redo_emit_dirty_canvas_and_track_depth() {
        let mut doc = Document::new(4, 3).expect("dims");
        doc.apply_tool("pencil", 0, 0, 0x123456ff)
            .expect("pencil ok");
        // Drain the paint event so the undo / redo events are isolated.
        let _ = doc.drain_events();

        assert_eq!(doc.undo_depth(), 1);
        assert_eq!(doc.redo_depth(), 0);

        assert!(doc.undo());
        assert_eq!(doc.undo_depth(), 0);
        assert_eq!(doc.redo_depth(), 1);
        let after_undo = doc.drain_events();
        assert_eq!(after_undo.len(), 1);
        assert_eq!(after_undo[0].kind(), "dirty-canvas");

        assert!(doc.redo().expect("redo ok"));
        assert_eq!(doc.undo_depth(), 1);
        assert_eq!(doc.redo_depth(), 0);
        let after_redo = doc.drain_events();
        assert_eq!(after_redo.len(), 1);
        assert_eq!(after_redo[0].kind(), "dirty-canvas");
    }

    #[test]
    fn undo_on_empty_stack_returns_false_and_emits_nothing() {
        let mut doc = Document::new(2, 2).expect("dims");
        assert!(!doc.undo());
        assert!(doc.drain_events().is_empty());
    }

    #[test]
    fn redo_on_empty_stack_returns_false_and_emits_nothing() {
        let mut doc = Document::new(2, 2).expect("dims");
        assert!(!doc.redo().expect("empty redo is ok"));
        assert!(doc.drain_events().is_empty());
    }

    #[test]
    fn apply_line_writes_pixels_along_horizontal_segment() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.apply_line(1, 3, 4, 3, 0xff8800ff).expect("line ok");
        let frame = doc.compose(0, 1).expect("compose ok");
        let pixels = frame.pixels();
        for x in 1..=4 {
            assert_eq!(pixel_at(&pixels, 8, x, 3), [0xff, 0x88, 0x00, 0xff]);
        }
        assert_eq!(pixel_at(&pixels, 8, 0, 3), [0, 0, 0, 0]);
        assert_eq!(pixel_at(&pixels, 8, 5, 3), [0, 0, 0, 0]);
    }

    #[test]
    fn apply_line_joins_the_undo_bus() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.apply_line(0, 0, 7, 7, 0x112233ff).expect("line ok");
        assert_eq!(doc.undo_depth(), 1);
        assert!(doc.bus.undo(&mut doc.sprite, &mut doc.cels));
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        for i in 0..8u32 {
            assert_eq!(pixel_at(&pixels, 8, i, i), [0, 0, 0, 0]);
        }
    }

    #[test]
    fn apply_line_emits_bounding_box_dirty_rect() {
        let mut doc = Document::new(16, 16).expect("dims");
        doc.apply_line(2, 5, 7, 9, 0x00ff00ff).expect("line ok");
        let events = doc.drain_events();
        assert_eq!(events.len(), 1);
        let ev = events[0];
        assert_eq!(ev.kind(), "dirty-rect");
        assert_eq!(ev.x(), 2);
        assert_eq!(ev.y(), 5);
        assert_eq!(ev.width(), 6);
        assert_eq!(ev.height(), 5);
    }

    #[test]
    fn apply_line_with_reversed_endpoints_has_positive_bbox() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.apply_line(6, 6, 1, 1, 0x000000ff).expect("line ok");
        let events = doc.drain_events();
        assert_eq!(events.len(), 1);
        let ev = events[0];
        assert_eq!(ev.x(), 1);
        assert_eq!(ev.y(), 1);
        assert_eq!(ev.width(), 6);
        assert_eq!(ev.height(), 6);
    }

    #[test]
    fn apply_line_errors_when_no_image_layer_exists() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.sprite.layers.clear();
        doc.sprite
            .layers
            .push(Layer::group(LayerId::new(3), "folder"));
        let err = doc
            .apply_line(0, 0, 1, 1, 0x000000ff)
            .expect_err("group-only doc has nothing to paint");
        assert!(err.contains("no paintable image layer"));
    }

    #[test]
    fn apply_line_single_pixel_writes_one_pixel() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_line(2, 1, 2, 1, 0xabcd01ff).expect("line ok");
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        assert_eq!(pixel_at(&pixels, 4, 2, 1), [0xab, 0xcd, 0x01, 0xff]);
    }

    #[test]
    fn endpoint_bbox_is_positive_for_any_endpoint_order() {
        assert_eq!(endpoint_bbox(0, 0, 3, 5), (0, 0, 4, 6));
        assert_eq!(endpoint_bbox(3, 5, 0, 0), (0, 0, 4, 6));
        assert_eq!(endpoint_bbox(2, 2, 2, 2), (2, 2, 1, 1));
        assert_eq!(endpoint_bbox(-3, -2, 1, 2), (-3, -2, 5, 5));
    }

    #[test]
    fn endpoint_bbox_saturates_at_u32_max_for_extreme_endpoints() {
        // Span of `i32::MAX - i32::MIN + 1 == 2^32` overflows `u32`; the
        // saturating cast clamps to `u32::MAX` so the dirty-rect event
        // still satisfies the `width >= 1` / `height >= 1` invariant.
        let (x, y, w, h) = endpoint_bbox(i32::MIN, i32::MIN, i32::MAX, i32::MAX);
        assert_eq!(x, i32::MIN);
        assert_eq!(y, i32::MIN);
        assert_eq!(w, u32::MAX);
        assert_eq!(h, u32::MAX);
    }

    #[test]
    fn apply_rectangle_outline_writes_only_the_border() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.apply_rectangle(1, 1, 4, 4, 0x335577ff, false)
            .expect("rect ok");
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        // Border pixels.
        for x in 1..=4u32 {
            assert_eq!(pixel_at(&pixels, 8, x, 1), [0x33, 0x55, 0x77, 0xff]);
            assert_eq!(pixel_at(&pixels, 8, x, 4), [0x33, 0x55, 0x77, 0xff]);
        }
        for y in 2..=3u32 {
            assert_eq!(pixel_at(&pixels, 8, 1, y), [0x33, 0x55, 0x77, 0xff]);
            assert_eq!(pixel_at(&pixels, 8, 4, y), [0x33, 0x55, 0x77, 0xff]);
        }
        // Interior stays transparent.
        for y in 2..=3u32 {
            for x in 2..=3u32 {
                assert_eq!(pixel_at(&pixels, 8, x, y), [0, 0, 0, 0]);
            }
        }
    }

    #[test]
    fn apply_rectangle_fill_writes_every_pixel_in_the_bbox() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.apply_rectangle(1, 1, 3, 3, 0xff0080ff, true)
            .expect("rect ok");
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        for y in 1..=3u32 {
            for x in 1..=3u32 {
                assert_eq!(pixel_at(&pixels, 8, x, y), [0xff, 0x00, 0x80, 0xff]);
            }
        }
        assert_eq!(pixel_at(&pixels, 8, 0, 0), [0, 0, 0, 0]);
        assert_eq!(pixel_at(&pixels, 8, 4, 4), [0, 0, 0, 0]);
    }

    #[test]
    fn apply_rectangle_joins_the_undo_bus() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.apply_rectangle(0, 0, 7, 7, 0x112233ff, true)
            .expect("rect ok");
        assert_eq!(doc.undo_depth(), 1);
        assert!(doc.bus.undo(&mut doc.sprite, &mut doc.cels));
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(pixel_at(&pixels, 8, x, y), [0, 0, 0, 0]);
            }
        }
    }

    #[test]
    fn apply_rectangle_emits_bounding_box_dirty_rect() {
        let mut doc = Document::new(16, 16).expect("dims");
        doc.apply_rectangle(2, 5, 7, 9, 0x00ff00ff, false)
            .expect("rect ok");
        let events = doc.drain_events();
        assert_eq!(events.len(), 1);
        let ev = events[0];
        assert_eq!(ev.kind(), "dirty-rect");
        assert_eq!(ev.x(), 2);
        assert_eq!(ev.y(), 5);
        assert_eq!(ev.width(), 6);
        assert_eq!(ev.height(), 5);
    }

    #[test]
    fn apply_rectangle_reversed_endpoints_have_positive_bbox() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.apply_rectangle(6, 6, 1, 1, 0x000000ff, true)
            .expect("rect ok");
        let events = doc.drain_events();
        assert_eq!(events.len(), 1);
        let ev = events[0];
        assert_eq!(ev.x(), 1);
        assert_eq!(ev.y(), 1);
        assert_eq!(ev.width(), 6);
        assert_eq!(ev.height(), 6);
    }

    #[test]
    fn apply_rectangle_errors_when_no_image_layer_exists() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.sprite.layers.clear();
        doc.sprite
            .layers
            .push(Layer::group(LayerId::new(3), "folder"));
        let err = doc
            .apply_rectangle(0, 0, 1, 1, 0x000000ff, false)
            .expect_err("group-only doc has nothing to paint");
        assert!(err.contains("no paintable image layer"));
    }

    #[test]
    fn apply_rectangle_single_pixel_writes_one_pixel_when_outline() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_rectangle(2, 1, 2, 1, 0xabcd01ff, false)
            .expect("rect ok");
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        assert_eq!(pixel_at(&pixels, 4, 2, 1), [0xab, 0xcd, 0x01, 0xff]);
    }

    #[test]
    fn apply_ellipse_outline_writes_pixels_on_the_rim() {
        let mut doc = Document::new(11, 11).expect("dims");
        doc.apply_ellipse(0, 0, 10, 10, 0x335577ff, false)
            .expect("ellipse ok");
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        // The 11×11 bbox inscribes a circle whose rim hits the four
        // axis-aligned extremes exactly.
        assert_eq!(pixel_at(&pixels, 11, 5, 0), [0x33, 0x55, 0x77, 0xff]);
        assert_eq!(pixel_at(&pixels, 11, 5, 10), [0x33, 0x55, 0x77, 0xff]);
        assert_eq!(pixel_at(&pixels, 11, 0, 5), [0x33, 0x55, 0x77, 0xff]);
        assert_eq!(pixel_at(&pixels, 11, 10, 5), [0x33, 0x55, 0x77, 0xff]);
        // Center is interior, not on the rim.
        assert_eq!(pixel_at(&pixels, 11, 5, 5), [0, 0, 0, 0]);
        // Bbox corners lie outside an inscribed circle.
        assert_eq!(pixel_at(&pixels, 11, 0, 0), [0, 0, 0, 0]);
        assert_eq!(pixel_at(&pixels, 11, 10, 10), [0, 0, 0, 0]);
    }

    #[test]
    fn apply_ellipse_fill_writes_the_center_and_the_rim() {
        let mut doc = Document::new(11, 11).expect("dims");
        doc.apply_ellipse(0, 0, 10, 10, 0xff0080ff, true)
            .expect("ellipse ok");
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        // Center and axis extremes are filled.
        assert_eq!(pixel_at(&pixels, 11, 5, 5), [0xff, 0x00, 0x80, 0xff]);
        assert_eq!(pixel_at(&pixels, 11, 5, 0), [0xff, 0x00, 0x80, 0xff]);
        assert_eq!(pixel_at(&pixels, 11, 0, 5), [0xff, 0x00, 0x80, 0xff]);
        // Bbox corners stay transparent — they sit outside the circle.
        assert_eq!(pixel_at(&pixels, 11, 0, 0), [0, 0, 0, 0]);
        assert_eq!(pixel_at(&pixels, 11, 10, 10), [0, 0, 0, 0]);
    }

    #[test]
    fn apply_ellipse_joins_the_undo_bus() {
        let mut doc = Document::new(11, 11).expect("dims");
        doc.apply_ellipse(0, 0, 10, 10, 0x112233ff, true)
            .expect("ellipse ok");
        assert_eq!(doc.undo_depth(), 1);
        assert!(doc.bus.undo(&mut doc.sprite, &mut doc.cels));
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        for y in 0..11u32 {
            for x in 0..11u32 {
                assert_eq!(pixel_at(&pixels, 11, x, y), [0, 0, 0, 0]);
            }
        }
    }

    #[test]
    fn apply_ellipse_emits_bounding_box_dirty_rect() {
        let mut doc = Document::new(16, 16).expect("dims");
        doc.apply_ellipse(2, 5, 7, 9, 0x00ff00ff, false)
            .expect("ellipse ok");
        let events = doc.drain_events();
        assert_eq!(events.len(), 1);
        let ev = events[0];
        assert_eq!(ev.kind(), "dirty-rect");
        assert_eq!(ev.x(), 2);
        assert_eq!(ev.y(), 5);
        assert_eq!(ev.width(), 6);
        assert_eq!(ev.height(), 5);
    }

    #[test]
    fn apply_ellipse_reversed_endpoints_have_positive_bbox() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.apply_ellipse(6, 6, 1, 1, 0x000000ff, true)
            .expect("ellipse ok");
        let events = doc.drain_events();
        assert_eq!(events.len(), 1);
        let ev = events[0];
        assert_eq!(ev.x(), 1);
        assert_eq!(ev.y(), 1);
        assert_eq!(ev.width(), 6);
        assert_eq!(ev.height(), 6);
    }

    #[test]
    fn apply_ellipse_errors_when_no_image_layer_exists() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.sprite.layers.clear();
        doc.sprite
            .layers
            .push(Layer::group(LayerId::new(3), "folder"));
        let err = doc
            .apply_ellipse(0, 0, 1, 1, 0x000000ff, false)
            .expect_err("group-only doc has nothing to paint");
        assert!(err.contains("no paintable image layer"));
    }

    #[test]
    fn apply_ellipse_single_pixel_writes_one_pixel() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_ellipse(2, 1, 2, 1, 0xabcd01ff, false)
            .expect("ellipse ok");
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        assert_eq!(pixel_at(&pixels, 4, 2, 1), [0xab, 0xcd, 0x01, 0xff]);
    }

    #[test]
    fn apply_bucket_fills_a_blank_canvas() {
        let mut doc = Document::new(4, 3).expect("dims");
        doc.apply_bucket(0, 0, 0x336699ff).expect("bucket ok");
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        for y in 0..3u32 {
            for x in 0..4u32 {
                assert_eq!(pixel_at(&pixels, 4, x, y), [0x33, 0x66, 0x99, 0xff]);
            }
        }
        assert_eq!(doc.undo_depth(), 1);
    }

    #[test]
    fn apply_bucket_stops_at_color_boundaries() {
        // Paint a vertical line down column 2 first, then bucket-fill from
        // the left half. The right half (including the line) stays
        // unchanged.
        let mut doc = Document::new(4, 3).expect("dims");
        for y in 0..3i32 {
            doc.apply_tool("pencil", 2, y, 0x0000ffff)
                .expect("pencil ok");
        }
        doc.apply_bucket(0, 0, 0xff0000ff).expect("bucket ok");
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        for y in 0..3u32 {
            assert_eq!(pixel_at(&pixels, 4, 0, y), [0xff, 0x00, 0x00, 0xff]);
            assert_eq!(pixel_at(&pixels, 4, 1, y), [0xff, 0x00, 0x00, 0xff]);
            assert_eq!(pixel_at(&pixels, 4, 2, y), [0x00, 0x00, 0xff, 0xff]);
            assert_eq!(pixel_at(&pixels, 4, 3, y), [0, 0, 0, 0]);
        }
    }

    #[test]
    fn apply_bucket_emits_dirty_canvas_event() {
        let mut doc = Document::new(4, 3).expect("dims");
        doc.apply_bucket(0, 0, 0x336699ff).expect("bucket ok");
        let events = doc.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind(), "dirty-canvas");
    }

    #[test]
    fn apply_bucket_joins_the_undo_bus() {
        let mut doc = Document::new(4, 3).expect("dims");
        doc.apply_bucket(0, 0, 0x336699ff).expect("bucket ok");
        assert_eq!(doc.undo_depth(), 1);
        assert!(doc.bus.undo(&mut doc.sprite, &mut doc.cels));
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        for y in 0..3u32 {
            for x in 0..4u32 {
                assert_eq!(pixel_at(&pixels, 4, x, y), [0, 0, 0, 0]);
            }
        }
    }

    #[test]
    fn apply_bucket_errors_when_no_image_layer_exists() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.sprite.layers.clear();
        doc.sprite
            .layers
            .push(Layer::group(LayerId::new(3), "folder"));
        let err = doc
            .apply_bucket(0, 0, 0x000000ff)
            .expect_err("group-only doc has nothing to paint");
        assert!(err.contains("no paintable image layer"));
    }

    #[test]
    fn apply_bucket_outside_canvas_is_a_noop_paint_but_joins_bus() {
        // Out-of-canvas seeds do not raise (matching pickColor's natural
        // semantics) — the underlying FillRegion treats them as no-op.
        // The command still joins the bus for undo symmetry.
        let mut doc = Document::new(2, 2).expect("dims");
        doc.apply_bucket(10, 10, 0xff0000ff).expect("bucket ok");
        assert_eq!(doc.undo_depth(), 1);
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        for y in 0..2u32 {
            for x in 0..2u32 {
                assert_eq!(pixel_at(&pixels, 2, x, y), [0, 0, 0, 0]);
            }
        }
    }

    #[test]
    fn fresh_document_has_no_selection() {
        let doc = Document::new(8, 8).expect("dims");
        assert!(!doc.has_selection());
        assert_eq!(doc.selection_x(), 0);
        assert_eq!(doc.selection_y(), 0);
        assert_eq!(doc.selection_width(), 0);
        assert_eq!(doc.selection_height(), 0);
    }

    #[test]
    fn set_selection_stores_and_exposes_the_rect() {
        let mut doc = Document::new(16, 16).expect("dims");
        doc.set_selection(3, 4, 5, 6);
        assert!(doc.has_selection());
        assert_eq!(doc.selection_x(), 3);
        assert_eq!(doc.selection_y(), 4);
        assert_eq!(doc.selection_width(), 5);
        assert_eq!(doc.selection_height(), 6);
    }

    #[test]
    fn set_selection_emits_selection_changed_with_new_bounds() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.set_selection(1, 2, 3, 4);
        let events = doc.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind(), "selection-changed");
        assert_eq!(events[0].x(), 1);
        assert_eq!(events[0].y(), 2);
        assert_eq!(events[0].width(), 3);
        assert_eq!(events[0].height(), 4);
    }

    #[test]
    fn set_selection_with_empty_rect_clears_and_emits_zeros() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.set_selection(2, 2, 5, 5);
        let _ = doc.drain_events();
        doc.set_selection(2, 2, 0, 5);
        assert!(!doc.has_selection());
        let events = doc.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind(), "selection-changed");
        assert_eq!(events[0].x(), 0);
        assert_eq!(events[0].y(), 0);
        assert_eq!(events[0].width(), 0);
        assert_eq!(events[0].height(), 0);
    }

    #[test]
    fn clear_selection_drops_the_rect_and_emits_zeros() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.set_selection(0, 0, 4, 4);
        let _ = doc.drain_events();
        doc.clear_selection();
        assert!(!doc.has_selection());
        let events = doc.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind(), "selection-changed");
        assert_eq!(events[0].width(), 0);
        assert_eq!(events[0].height(), 0);
    }

    #[test]
    fn clear_selection_emits_event_even_when_no_prior_selection() {
        // Matches the apply_bucket-style "every write emits" contract;
        // the UI's RAF loop coalesces duplicates.
        let mut doc = Document::new(4, 4).expect("dims");
        doc.clear_selection();
        let events = doc.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind(), "selection-changed");
    }

    #[test]
    fn selection_is_not_undoable_in_the_m7_8_slice() {
        // Selection state is intentionally outside the undo stack — see
        // STATUS.md and Sprite::selection docs. A paint between two
        // selection changes confirms undo only rewinds the paint.
        let mut doc = Document::new(8, 8).expect("dims");
        doc.set_selection(1, 1, 4, 4);
        doc.apply_tool("pencil", 0, 0, 0xff0000ff)
            .expect("pencil ok");
        let _ = doc.drain_events();
        assert!(doc.undo());
        assert!(doc.has_selection(), "undo should not affect selection");
        assert_eq!(doc.selection_width(), 4);
    }

    #[test]
    fn set_selection_off_canvas_round_trips_through_getters() {
        // Mirrors the pincel-core behavior: the model does not clip
        // selections to the canvas; the UI / future commands do.
        let mut doc = Document::new(8, 8).expect("dims");
        doc.set_selection(-3, -2, 100, 100);
        assert!(doc.has_selection());
        assert_eq!(doc.selection_x(), -3);
        assert_eq!(doc.selection_y(), -2);
        assert_eq!(doc.selection_width(), 100);
        assert_eq!(doc.selection_height(), 100);
    }

    #[test]
    fn apply_move_selection_translates_pixels_and_selection() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.apply_tool("pencil", 1, 1, 0xff0000ff)
            .expect("pencil ok");
        doc.set_selection(1, 1, 1, 1);
        let _ = doc.drain_events();
        doc.apply_move_selection(3, 2).expect("move ok");
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        assert_eq!(pixel_at(&pixels, 8, 1, 1), [0, 0, 0, 0]);
        assert_eq!(pixel_at(&pixels, 8, 4, 3), [0xff, 0, 0, 0xff]);
        assert_eq!(doc.selection_x(), 4);
        assert_eq!(doc.selection_y(), 3);
    }

    #[test]
    fn apply_move_selection_emits_dirty_canvas_and_selection_events() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.set_selection(2, 2, 2, 2);
        let _ = doc.drain_events();
        doc.apply_move_selection(1, 0).expect("move ok");
        let events = doc.drain_events();
        let kinds: Vec<String> = events.iter().map(|e| e.kind()).collect();
        assert!(kinds.iter().any(|k| k == "dirty-canvas"));
        assert!(kinds.iter().any(|k| k == "selection-changed"));
    }

    #[test]
    fn apply_move_selection_without_selection_errors() {
        let mut doc = Document::new(8, 8).expect("dims");
        let err = doc
            .apply_move_selection(1, 1)
            .expect_err("no selection should error");
        assert!(err.contains("no active selection"));
    }

    #[test]
    fn apply_move_selection_joins_the_undo_bus() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.apply_tool("pencil", 1, 1, 0xff0000ff)
            .expect("pencil ok");
        doc.set_selection(1, 1, 1, 1);
        doc.apply_move_selection(2, 0).expect("move ok");
        assert_eq!(doc.undo_depth(), 2);
        assert!(doc.bus.undo(&mut doc.sprite, &mut doc.cels));
        // Selection rect is restored (selection IS undoable through this
        // command — the command stores prior_selection in its state).
        assert_eq!(doc.selection_x(), 1);
        assert_eq!(doc.selection_y(), 1);
        let pixels = doc.compose(0, 1).expect("compose ok").pixels();
        assert_eq!(pixel_at(&pixels, 8, 1, 1), [0xff, 0, 0, 0xff]);
        assert_eq!(pixel_at(&pixels, 8, 3, 1), [0, 0, 0, 0]);
    }

    #[test]
    fn apply_move_selection_zero_delta_still_joins_bus() {
        // A (0, 0) drag is a valid commit — it still pushes a command
        // onto the bus and emits a selection-changed event so the UI
        // path is uniform with non-zero deltas.
        let mut doc = Document::new(8, 8).expect("dims");
        doc.set_selection(2, 2, 1, 1);
        let _ = doc.drain_events();
        doc.apply_move_selection(0, 0).expect("move ok");
        assert_eq!(doc.undo_depth(), 1);
        assert_eq!(doc.selection_x(), 2);
        assert_eq!(doc.selection_y(), 2);
    }

    #[test]
    fn apply_move_selection_errors_when_no_image_layer_exists() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.set_selection(0, 0, 2, 2);
        doc.sprite.layers.clear();
        doc.sprite
            .layers
            .push(Layer::group(LayerId::new(3), "folder"));
        let err = doc
            .apply_move_selection(1, 0)
            .expect_err("group-only doc has nothing to paint");
        assert!(err.contains("no paintable image layer"));
    }

    // ---- M8.6: tilemap surface tests ----------------------------------

    fn doc_with_tilemap_layer() -> (Document, LayerId, FrameIndex) {
        // 8x8 canvas with a 2x2 tilemap layer (layer id 1) bound to a
        // freshly-added tileset. The tile size matches the M8.5 round-
        // trip fixtures so dirty rects line up at (0,0) - (2,2).
        let mut doc = Document::new(8, 8).expect("dims");
        let ts_id = doc.add_tileset("ts", 2, 2).expect("addTileset");
        let layer_id = LayerId::new(1);
        let layer = Layer::tilemap(layer_id, "tiles", TilesetId::new(ts_id));
        doc.sprite.layers.push(layer);
        let frame = FrameIndex::new(0);
        doc.cels.insert(Cel {
            layer: layer_id,
            frame,
            position: (0, 0),
            opacity: 255,
            data: CelData::Tilemap {
                grid_w: 2,
                grid_h: 2,
                tiles: vec![TileRef::EMPTY; 4],
            },
        });
        // Drain the events the bootstrap produced so the per-test
        // assertions can match the per-method emissions exactly.
        let _ = doc.drain_events();
        (doc, layer_id, frame)
    }

    #[test]
    fn add_tileset_returns_zero_for_first_tileset_and_increments() {
        let mut doc = Document::new(4, 4).expect("dims");
        assert_eq!(doc.tileset_count(), 0);
        let first = doc.add_tileset("a", 8, 8).expect("addTileset a");
        assert_eq!(first, 0);
        let second = doc.add_tileset("b", 16, 16).expect("addTileset b");
        assert_eq!(second, 1);
        assert_eq!(doc.tileset_count(), 2);
    }

    #[test]
    fn add_tileset_rejects_zero_tile_size() {
        let mut doc = Document::new(4, 4).expect("dims");
        let err = doc.add_tileset("zero", 0, 8).unwrap_err();
        assert!(err.contains("tile size"));
    }

    #[test]
    fn add_tileset_joins_undo_bus() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.add_tileset("x", 8, 8).expect("addTileset");
        assert!(doc.undo_depth() >= 1);
        assert!(doc.undo());
        assert_eq!(doc.tileset_count(), 0);
    }

    #[test]
    fn add_tileset_rejects_exhausted_id_space() {
        // Inject a tileset with id `u32::MAX` directly so the next
        // `add_tileset` call has nowhere to go. The detection runs
        // before the command bus executes, so the call surfaces a
        // clearer error than a generic duplicate-id collision.
        let mut doc = Document::new(4, 4).expect("dims");
        doc.sprite
            .tilesets
            .push(Tileset::new(TilesetId::new(u32::MAX), "max", (1, 1)));
        let err = doc.add_tileset("overflow", 1, 1).unwrap_err();
        assert!(
            err.contains("id space exhausted"),
            "expected exhaustion error, got: {err}"
        );
    }

    #[test]
    fn tileset_getters_round_trip_dimensions_and_name() {
        let mut doc = Document::new(4, 4).expect("dims");
        let id = doc.add_tileset("ground", 16, 16).expect("addTileset");
        assert_eq!(doc.tileset_count(), 1);
        assert_eq!(doc.tileset_id_at(0).unwrap(), id);
        assert_eq!(doc.tileset_name(id), "ground");
        assert_eq!(doc.tileset_tile_width(id), 16);
        assert_eq!(doc.tileset_tile_height(id), 16);
        assert_eq!(doc.tileset_tile_count(id), 0);
    }

    #[test]
    fn tileset_getters_return_defaults_for_unknown_id() {
        let doc = Document::new(4, 4).expect("dims");
        assert_eq!(doc.tileset_tile_width(99), 0);
        assert_eq!(doc.tileset_tile_height(99), 0);
        assert_eq!(doc.tileset_tile_count(99), 0);
        assert_eq!(doc.tileset_name(99), "");
        assert!(doc.tileset_id_at(0).is_err());
    }

    #[test]
    fn place_tile_routes_through_command_bus_and_emits_dirty_rect() {
        let (mut doc, layer_id, frame_idx) = doc_with_tilemap_layer();
        doc.place_tile(layer_id.0, frame_idx.0, 1, 0, 1)
            .expect("placeTile");
        // Cel updated.
        let cel = doc
            .cels
            .get(layer_id, frame_idx)
            .expect("tilemap cel present");
        let CelData::Tilemap { tiles, .. } = &cel.data else {
            panic!("expected tilemap cel");
        };
        assert_eq!(tiles[1].tile_id, 1);
        // Dirty-rect covers the single cell at (1,0) * (2,2) = (2,0,2,2).
        let events = doc.drain_events();
        assert_eq!(events.len(), 1, "expected exactly one dirty event");
        assert_eq!(events[0].kind(), "dirty-rect");
        assert_eq!(events[0].layer(), layer_id.0);
        assert_eq!(events[0].frame(), frame_idx.0);
        assert_eq!(events[0].x(), 2);
        assert_eq!(events[0].y(), 0);
        assert_eq!(events[0].width(), 2);
        assert_eq!(events[0].height(), 2);
    }

    #[test]
    fn place_tile_joins_undo_bus() {
        let (mut doc, layer_id, frame_idx) = doc_with_tilemap_layer();
        let depth_before = doc.undo_depth();
        doc.place_tile(layer_id.0, frame_idx.0, 0, 0, 1)
            .expect("placeTile");
        assert_eq!(doc.undo_depth(), depth_before + 1);
        assert!(doc.undo());
        let cel = doc.cels.get(layer_id, frame_idx).unwrap();
        let CelData::Tilemap { tiles, .. } = &cel.data else {
            panic!("expected tilemap cel");
        };
        // Undo restored the prior TileRef::EMPTY.
        assert_eq!(tiles[0].tile_id, 0);
    }

    #[test]
    fn place_tile_rejects_non_tilemap_layer() {
        let mut doc = Document::new(4, 4).expect("dims");
        // Layer 0 is the bootstrap image layer.
        let err = doc.place_tile(0, 0, 0, 0, 1).unwrap_err();
        assert!(
            err.contains("not a tilemap layer"),
            "expected layer-kind error, got: {err}"
        );
    }

    #[test]
    fn place_tile_rejects_out_of_grid_coords() {
        let (mut doc, layer_id, frame_idx) = doc_with_tilemap_layer();
        let err = doc
            .place_tile(layer_id.0, frame_idx.0, 5, 5, 1)
            .unwrap_err();
        assert!(
            err.contains("place tile") || err.contains("coord") || err.contains("bounds"),
            "expected coord-bounds error, got: {err}"
        );
    }

    #[test]
    fn tile_pixels_returns_rgba_buffer_for_stored_tile() {
        use pincel_core::TileImage;
        let mut doc = Document::new(4, 4).expect("dims");
        let ts_id = doc.add_tileset("t", 2, 2).expect("addTileset");
        // Seed two distinct tiles directly on the underlying tileset —
        // the public surface doesn't yet expose per-tile pixel writes
        // (that lands in M8.7d).
        let tileset = doc
            .sprite
            .tilesets
            .iter_mut()
            .find(|t| t.id.0 == ts_id)
            .expect("freshly added tileset is reachable");
        let mut tile0 = PixelBuffer::empty(2, 2, ColorMode::Rgba);
        tile0.data.copy_from_slice(&[0u8; 16]);
        let mut tile1 = PixelBuffer::empty(2, 2, ColorMode::Rgba);
        tile1.data.copy_from_slice(&[
            0xFF, 0x00, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF,
        ]);
        tileset.tiles.push(TileImage { pixels: tile0 });
        tileset.tiles.push(TileImage { pixels: tile1 });

        let bytes0 = doc.tile_pixels(ts_id, 0).expect("tile 0 pixels");
        assert_eq!(bytes0.len(), 2 * 2 * 4);
        assert!(bytes0.iter().all(|b| *b == 0));

        let bytes1 = doc.tile_pixels(ts_id, 1).expect("tile 1 pixels");
        assert_eq!(bytes1[0..4], [0xFF, 0x00, 0x00, 0xFF]);
        assert_eq!(bytes1[12..16], [0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn tile_pixels_rejects_unknown_tileset() {
        let doc = Document::new(4, 4).expect("dims");
        let err = doc.tile_pixels(99, 0).unwrap_err();
        assert!(err.contains("unknown tileset"), "got: {err}");
    }

    #[test]
    fn tile_pixels_rejects_tile_id_past_end() {
        let mut doc = Document::new(4, 4).expect("dims");
        let ts_id = doc.add_tileset("empty", 8, 8).expect("addTileset");
        // Freshly added tileset stores zero tiles (Aseprite convention:
        // tile 0 is implicit empty and not stored), so any tile_id is
        // past the end.
        let err = doc.tile_pixels(ts_id, 0).unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn add_tilemap_layer_creates_layer_and_seeds_cel_sized_to_canvas() {
        let mut doc = Document::new(8, 6).expect("dims");
        let ts_id = doc.add_tileset("g", 4, 4).expect("addTileset");
        let _ = doc.drain_events();
        let new_layer = doc
            .add_tilemap_layer("tiles", ts_id)
            .expect("addTilemapLayer");
        // Layer registered with correct kind.
        assert_eq!(doc.layer_kind(new_layer), "tilemap");
        assert_eq!(doc.layer_name(new_layer), "tiles");
        assert_eq!(doc.layer_tileset_id(new_layer), ts_id);
        // Cel seeded for frame 0 with grid = ceil(8/4) x ceil(6/4) = 2x2.
        let cel = doc
            .cels
            .get(LayerId::new(new_layer), FrameIndex::new(0))
            .expect("tilemap cel seeded");
        let CelData::Tilemap {
            grid_w,
            grid_h,
            tiles,
        } = &cel.data
        else {
            panic!("expected tilemap cel");
        };
        assert_eq!((*grid_w, *grid_h), (2, 2));
        assert_eq!(tiles.len(), 4);
        assert!(tiles.iter().all(|t| t.tile_id == 0));
        // Emits dirty-canvas so the UI repaints.
        let events = doc.drain_events();
        assert!(events.iter().any(|e| e.kind() == "dirty-canvas"));
    }

    #[test]
    fn add_tilemap_layer_rejects_unknown_tileset() {
        let mut doc = Document::new(4, 4).expect("dims");
        let err = doc.add_tilemap_layer("x", 99).unwrap_err();
        assert!(err.contains("unknown tileset"), "got: {err}");
    }

    #[test]
    fn add_tilemap_layer_joins_undo_bus() {
        let mut doc = Document::new(4, 4).expect("dims");
        let ts_id = doc.add_tileset("g", 4, 4).expect("addTileset");
        let depth = doc.undo_depth();
        let _ = doc.add_tilemap_layer("tiles", ts_id).expect("addTilemapLayer");
        assert_eq!(doc.undo_depth(), depth + 1);
        assert!(doc.undo());
        // After undo the tilemap layer is gone; only the bootstrap
        // image layer remains.
        assert_eq!(doc.layer_count(), 1);
    }

    #[test]
    fn set_tile_pixel_writes_and_emits_dirty_canvas() {
        let mut doc = Document::new(4, 4).expect("dims");
        let ts_id = doc.add_tileset("t", 2, 2).expect("addTileset");
        let tile_id = doc.add_tile(ts_id).expect("addTile");
        let _ = doc.drain_events();
        doc.set_tile_pixel(ts_id, tile_id, 1, 1, 0xFF00_00FFu32)
            .expect("setTilePixel");
        let bytes = doc.tile_pixels(ts_id, tile_id).expect("tilePixels");
        // (1,1) offset in a 2x2 RGBA buffer = (1*2 + 1) * 4 = 12.
        assert_eq!(bytes[12..16], [0xFF, 0x00, 0x00, 0xFF]);
        let events = doc.drain_events();
        assert!(events.iter().any(|e| e.kind() == "dirty-canvas"));
    }

    #[test]
    fn set_tile_pixel_joins_undo_bus() {
        let mut doc = Document::new(4, 4).expect("dims");
        let ts_id = doc.add_tileset("t", 2, 2).expect("addTileset");
        let tile_id = doc.add_tile(ts_id).expect("addTile");
        let depth = doc.undo_depth();
        doc.set_tile_pixel(ts_id, tile_id, 0, 0, 0xFFFF_FFFFu32)
            .expect("setTilePixel");
        assert_eq!(doc.undo_depth(), depth + 1);
        assert!(doc.undo());
        let bytes = doc.tile_pixels(ts_id, tile_id).expect("tilePixels");
        assert!(bytes.iter().all(|b| *b == 0), "undo restored transparent");
    }

    #[test]
    fn layer_kind_distinguishes_image_tilemap_unknown() {
        let mut doc = Document::new(4, 4).expect("dims");
        let ts_id = doc.add_tileset("t", 2, 2).expect("addTileset");
        let tilemap_id = doc.add_tilemap_layer("tm", ts_id).expect("addTilemapLayer");
        // Bootstrap image layer is id 0.
        assert_eq!(doc.layer_kind(0), "image");
        assert_eq!(doc.layer_kind(tilemap_id), "tilemap");
        assert_eq!(doc.layer_kind(99), "");
    }

    #[test]
    fn add_tile_appends_transparent_tile() {
        let mut doc = Document::new(4, 4).expect("dims");
        let ts_id = doc.add_tileset("t", 2, 2).expect("addTileset");
        assert_eq!(doc.tileset_tile_count(ts_id), 0);
        let tile_id = doc.add_tile(ts_id).expect("addTile");
        assert_eq!(tile_id, 0);
        assert_eq!(doc.tileset_tile_count(ts_id), 1);
        let bytes = doc.tile_pixels(ts_id, tile_id).expect("tilePixels");
        assert_eq!(bytes.len(), 2 * 2 * 4);
        assert!(bytes.iter().all(|b| *b == 0));
    }
}
