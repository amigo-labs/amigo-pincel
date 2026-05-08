//! WebAssembly bindings for Pincel.
//!
//! This crate is the `wasm-bindgen` / `wasm-pack` target that exposes the
//! `pincel-core` document model to JavaScript. The boundary contract lives
//! in `docs/specs/pincel.md` §9.3 (`Document.new`, `applyTool`, `compose`,
//! `drainEvents`) and the public-package surface in §17.5
//! (`Pincel.create`, `openFile`, `saveAseprite`, `on('change' | 'save')`).
//!
//! Phase 1 / CLAUDE.md M6 lands the surface incrementally. This skeleton
//! covers `Document::new`, opening / saving Aseprite byte buffers, and
//! basic dimension getters. `applyTool`, `compose`, and event drains
//! follow in subsequent M6 sub-tasks.
//!
//! Errors cross the boundary as `Result<_, String>`; `wasm-bindgen` maps
//! `String` Errs to a thrown JS exception. This keeps the surface
//! testable on the host target — `JsError::new` panics outside of
//! `wasm32-unknown-unknown` because it imports JS-side machinery.

use pincel_core::{
    AsepriteReadOutput, CelMap, ColorMode, Frame, Sprite, read_aseprite, write_aseprite,
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
}
