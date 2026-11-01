//! SVG export (`docs/specs/vectorize-dialog.md`): a `pincel-trace` session
//! over one composed frame, traced with the flat option vector of
//! [`pincel_trace::TraceOptions::from_wire`].

use pincel_core::{ComposeRequest, FrameIndex, compose};
use pincel_trace::{TraceOptions, TraceSession, to_svg};
use wasm_bindgen::prelude::*;

use crate::Document;

/// Traces one source image repeatedly; keeps the core's caches between
/// traces so a preview that changes only Potrace options stays cheap.
#[wasm_bindgen]
pub struct Vectorizer {
    session: TraceSession,
}

/// The SVG of one trace and its counts.
#[wasm_bindgen]
pub struct VectorizeResult {
    svg: String,
    layers: u32,
    outlines: u32,
    threshold: i32,
}

#[wasm_bindgen]
impl VectorizeResult {
    /// The SVG document.
    #[wasm_bindgen(getter)]
    pub fn svg(&self) -> String {
        self.svg.clone()
    }

    /// Number of layers (one per colour).
    #[wasm_bindgen(getter)]
    pub fn layers(&self) -> u32 {
        self.layers
    }

    /// Number of outlines over all layers.
    #[wasm_bindgen(getter)]
    pub fn outlines(&self) -> u32 {
        self.outlines
    }

    /// The monochrome threshold used, or −1 outside monochrome mode.
    #[wasm_bindgen(getter)]
    pub fn threshold(&self) -> i32 {
        self.threshold
    }
}

#[wasm_bindgen]
impl Vectorizer {
    /// Traces with `mode` (0 colours, 1 monochrome, 2 pixel-exact) and the
    /// flat `params` vector; errors carry the core's message.
    pub fn trace(&mut self, mode: u8, params: &[f64]) -> Result<VectorizeResult, String> {
        let opts = TraceOptions::from_wire(mode, params).map_err(|e| e.to_string())?;
        let result = self.session.trace(&opts).map_err(|e| e.to_string())?;
        Ok(VectorizeResult {
            svg: to_svg(&result),
            layers: result.layers.len() as u32,
            outlines: result.stats.paths as u32,
            threshold: result.threshold_used.map_or(-1, i32::from),
        })
    }
}

#[wasm_bindgen]
impl Document {
    /// A [`Vectorizer`] over `frame` composed like
    /// [`exportPng`](Document::export_png): zoom 1, visible layers, no onion
    /// skin, no overlays.
    pub fn vectorizer(&self, frame: u32) -> Result<Vectorizer, String> {
        let request = ComposeRequest::full(
            FrameIndex::new(frame),
            self.sprite.width,
            self.sprite.height,
        );
        let mut pixels = Vec::new();
        let composed = compose(&self.sprite, &self.cels, &request, &mut pixels)
            .map_err(|e| format!("failed to compose frame {frame}: {e}"))?;
        let session = TraceSession::new(&pixels, composed.width, composed.height)
            .map_err(|e| e.to_string())?;
        Ok(Vectorizer { session })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIXEL_EXACT: u8 = 2;
    const DEFAULTS: [f64; 9] = [6.0, -1.0, 0.0, 0.0, 0.0, 2.0, 1.0, 1.0, 0.2];

    #[test]
    fn vectorizer_traces_the_visible_frame_pixel_exact() {
        let mut doc = Document::new(4, 3).expect("doc");
        doc.apply_rectangle(1, 1, 1, 1, 0xff00_00ff, true)
            .expect("paint");
        let mut v = doc.vectorizer(0).expect("vectorizer");
        let r = v.trace(PIXEL_EXACT, &DEFAULTS).expect("trace");
        assert_eq!(r.layers(), 1);
        assert_eq!(r.outlines(), 1);
        assert!(r.svg().contains(r#"viewBox="0 0 4 3""#), "{}", r.svg());
        assert!(r.svg().contains("M1 1l1 0l0 1l-1 0z"), "{}", r.svg());
        assert_eq!(r.threshold(), -1);
    }

    #[test]
    fn vectorizer_reports_bad_options() {
        let doc = Document::new(2, 2).expect("doc");
        let mut v = doc.vectorizer(0).expect("vectorizer");
        assert!(v.trace(7, &DEFAULTS).is_err());
        assert!(v.trace(0, &DEFAULTS[..3]).is_err());
        assert!(doc.vectorizer(5).is_err());
    }
}
