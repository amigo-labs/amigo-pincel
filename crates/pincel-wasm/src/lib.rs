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
    AsepriteReadOutput, Bus, Cel, CelMap, ColorMode, ComposeRequest, DrawLine, DrawRectangle,
    Frame, FrameIndex, Layer, LayerId, LayerKind, PixelBuffer, Rect, Rgba, SetPixel, Sprite,
    compose, read_aseprite, write_aseprite,
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
        use pincel_core::CelData;
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
}
