//! Raster import surface (Fineliner parity): open a PNG as a new document
//! or drop one onto the current document as a new layer.

use pincel_core::{Cel, ColorMode, FrameIndex, PixelBuffer, import_png};
use wasm_bindgen::prelude::*;

use crate::Document;

#[wasm_bindgen]
impl Document {
    /// Create a new RGBA document from PNG bytes: one image layer named
    /// `"Layer 1"` holding the decoded pixels at frame `0`, one 100 ms
    /// frame. Any PNG colour type / bit depth is accepted.
    #[wasm_bindgen(js_name = openPng)]
    pub fn open_png(bytes: &[u8]) -> Result<Document, String> {
        let img = import_png(bytes).map_err(|e| format!("failed to open PNG: {e}"))?;
        let mut doc = Document::new(img.width, img.height)?;
        let mut buf = PixelBuffer::empty(img.width, img.height, ColorMode::Rgba);
        buf.data = img.rgba;
        doc.cels
            .insert(Cel::image(crate::DEFAULT_LAYER_ID, FrameIndex::new(0), buf));
        Ok(doc)
    }

    /// Decode PNG bytes and add them as a new image layer named `name` on
    /// top of the stack, at the current frame, anchored at the canvas
    /// origin and clipped to the canvas. Returns the new layer id (which
    /// also becomes the active paint target). One undo step removes the
    /// layer and its pixels (the Paste command, spec §5.4).
    #[wasm_bindgen(js_name = importPngAsLayer)]
    pub fn import_png_as_layer(&mut self, bytes: &[u8], name: &str) -> Result<u32, String> {
        self.paste_image(bytes, name, 0, 0, false, None)
            .map_err(|e| format!("failed to import PNG: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PNG bytes for a 2×1 image with the given two `0xRRGGBBAA` pixels,
    /// produced through the document's own exporter.
    fn png_2x1(left: u32, right: u32) -> Vec<u8> {
        let mut doc = Document::new(2, 1).expect("dims");
        doc.apply_tool("pencil", 0, 0, left).expect("paint");
        doc.apply_tool("pencil", 1, 0, right).expect("paint");
        doc.export_png(0).expect("export").into_vec()
    }

    #[test]
    fn open_png_builds_a_paintable_document_with_the_pixels() {
        let bytes = png_2x1(0xFF0000FF, 0x0000FF80);
        let mut doc = Document::open_png(&bytes).expect("open");
        assert_eq!((doc.width(), doc.height()), (2, 1));
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick"), 0xFF0000FF);
        assert_eq!(doc.pick_color(0, 1, 0).expect("pick"), 0x0000FF80);
        doc.apply_tool("pencil", 1, 0, 0x00FF00FF)
            .expect("paint on it");
        assert_eq!(doc.pick_color(0, 1, 0).expect("pick"), 0x00FF00FF);
        assert!(Document::open_png(&[1, 2, 3]).is_err());
    }

    #[test]
    fn import_png_as_layer_adds_a_clipped_layer_on_top_in_one_undo_step() {
        let mut doc = Document::new(1, 1).expect("dims");
        let bytes = png_2x1(0x090909FF, 0x010101FF);
        let id = doc.import_png_as_layer(&bytes, "imported").expect("import");
        assert_eq!(doc.layer_count(), 2);
        assert_eq!(doc.layer_name(id), "imported");
        assert_eq!(doc.paint_target_layer_id(), id);
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick"), 0x090909FF);
        assert_eq!(doc.layer_id_at(1).expect("id"), id, "on top of the stack");
        assert_eq!(doc.undo_depth(), 1, "one undo step");
        assert!(doc.undo(), "undo the import");
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick") & 0xFF, 0);
        assert!(!doc.undo(), "nothing left to undo");
    }
}
