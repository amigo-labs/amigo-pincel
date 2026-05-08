//! WebAssembly bindings for Pincel.
//!
//! This crate is the `wasm-bindgen` / `wasm-pack` target that exposes the
//! `pincel-core` document model to JavaScript. The boundary contract lives
//! in `docs/specs/pincel.md` §9.3 (`Document.new`, `applyTool`, `compose`,
//! `drainEvents`) and the public-package surface in §17.5
//! (`Pincel.create`, `openFile`, `saveAseprite`, `on('change' | 'save')`).
//!
//! Phase 1 / CLAUDE.md M6 lands the surface incrementally. This skeleton
//! covers `Document::new`, opening / saving Aseprite byte buffers, the
//! full-canvas `Document::compose` entry point, and basic dimension
//! getters. `applyTool` and event drains follow in subsequent M6
//! sub-tasks.
//!
//! Errors cross the boundary as `Result<_, String>`; `wasm-bindgen` maps
//! `String` Errs to a thrown JS exception. This keeps the surface
//! testable on the host target — `JsError::new` panics outside of
//! `wasm32-unknown-unknown` because it imports JS-side machinery.

use pincel_core::{
    AsepriteReadOutput, CelMap, ColorMode, ComposeRequest, Frame, FrameIndex, Sprite, compose,
    read_aseprite, write_aseprite,
};
use wasm_bindgen::prelude::*;

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
}

#[wasm_bindgen]
impl Document {
    /// Create an empty RGBA document with the given canvas dimensions.
    ///
    /// The fresh document has no layers and a single 100 ms frame so the
    /// round-trip through `aseprite-writer` / `aseprite-loader`
    /// produces a parseable file.
    ///
    /// Returns `Err(String)` when the sprite builder rejects the input
    /// — today the only failure mode is a zero `width` or `height`. The
    /// error string comes from `pincel_core`'s `Display` impl and is
    /// not part of the public JS contract.
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32) -> Result<Document, String> {
        let sprite = Sprite::builder(width, height)
            .color_mode(ColorMode::Rgba)
            .add_frame(Frame::new(100))
            .build()
            .map_err(|e| format!("failed to build sprite: {e}"))?;
        Ok(Self {
            sprite,
            cels: CelMap::new(),
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
        Ok(Self { sprite, cels })
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
    fn new_creates_empty_rgba_document_with_one_frame() {
        let doc = Document::new(64, 48).expect("non-zero dims build");
        assert_eq!(doc.width(), 64);
        assert_eq!(doc.height(), 48);
        assert_eq!(doc.layer_count(), 0);
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
        assert_eq!(reopened.layer_count(), 0);
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
    fn compose_zero_layer_document_yields_transparent_canvas() {
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
}
