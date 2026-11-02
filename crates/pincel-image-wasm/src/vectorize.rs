//! SVG export (`docs/specs/vectorize-dialog.md`): a `pincel-trace` session
//! over the document's composite, traced with the flat option vector of
//! [`pincel_trace::TraceOptions::from_wire`].

use pincel_image_core::compose;
use pincel_trace::{TraceOptions, TraceSession, to_svg};
use wasm_bindgen::prelude::*;

use crate::with_bus;

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

impl Vectorizer {
    fn run(&mut self, mode: u8, params: &[f64]) -> Result<VectorizeResult, String> {
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
impl Vectorizer {
    /// Traces with `mode` (0 colours, 1 monochrome, 2 pixel-exact) and the
    /// flat `params` vector; errors carry the core's message.
    pub fn trace(&mut self, mode: u8, params: &[f64]) -> Result<VectorizeResult, JsError> {
        self.run(mode, params).map_err(|e| JsError::new(&e))
    }
}

/// The composite of the visible layers as a [`Vectorizer`].
fn session_for(handle: u32) -> Result<Result<Vectorizer, String>, JsError> {
    with_bus(handle, |bus| {
        let img = compose(bus.document.layers());
        let (w, h) = (img.width(), img.height());
        Ok(TraceSession::new(&img.into_raw(), w, h)
            .map(|session| Vectorizer { session })
            .map_err(|e| e.to_string()))
    })
}

/// A [`Vectorizer`] over the document's composite of its visible layers.
#[wasm_bindgen]
pub fn vectorizer(handle: u32) -> Result<Vectorizer, JsError> {
    session_for(handle)?.map_err(|e| JsError::new(&e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{close_document, create_document};

    const COLOURS: u8 = 0;
    const DEFAULTS: [f64; 9] = [6.0, -1.0, 0.0, 0.0, 0.0, 2.0, 1.0, 1.0, 0.2];

    #[test]
    fn vectorizer_traces_the_composite() {
        let handle = create_document(5, 3).expect("document");
        let mut v = session_for(handle).expect("handle").expect("session");
        let r = v.run(COLOURS, &DEFAULTS).expect("trace");
        assert!(r.svg().contains(r#"viewBox="0 0 5 3""#), "{}", r.svg());
        assert_eq!(r.threshold(), -1);
        assert!(v.run(9, &DEFAULTS).is_err());
        close_document(handle);
    }
}
