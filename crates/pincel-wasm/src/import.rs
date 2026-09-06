//! Raster import surface (Fineliner parity): open a PNG as a new document
//! or drop one onto the current document as a new layer.

use pincel_core::{Cel, ColorMode, FrameIndex, LayerId, PixelBuffer, ReplaceCelPixels, import_png};
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

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
    /// also becomes the active paint target). One undo step restores the
    /// layer's blank cel, a second removes the layer.
    #[wasm_bindgen(js_name = importPngAsLayer)]
    pub fn import_png_as_layer(&mut self, bytes: &[u8], name: &str) -> Result<u32, String> {
        let img = import_png(bytes).map_err(|e| format!("failed to import PNG: {e}"))?;
        let new_id = self.add_layer(name)?;
        let layer = LayerId::new(new_id);
        let frame = self.current_frame;
        self.ensure_paint_cel(layer, frame);
        let (w, h) = (self.sprite.width, self.sprite.height);
        let mut pixels = vec![0u8; w as usize * h as usize * 4];
        let copy_w = img.width.min(w) as usize;
        for y in 0..img.height.min(h) as usize {
            let src = y * img.width as usize * 4;
            let dst = y * w as usize * 4;
            pixels[dst..dst + copy_w * 4].copy_from_slice(&img.rgba[src..src + copy_w * 4]);
        }
        let cmd = ReplaceCelPixels::new(layer, frame, pixels);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to import PNG: {e}"))?;
        self.bus.seal();
        self.active_layer = Some(layer);
        self.events.push(Event::dirty_canvas());
        Ok(new_id)
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
    fn import_png_as_layer_adds_a_clipped_layer_on_top() {
        let mut doc = Document::new(1, 1).expect("dims");
        let bytes = png_2x1(0x090909FF, 0x010101FF);
        let id = doc.import_png_as_layer(&bytes, "imported").expect("import");
        assert_eq!(doc.layer_count(), 2);
        assert_eq!(doc.layer_name(id), "imported");
        assert_eq!(doc.paint_target_layer_id(), id);
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick"), 0x090909FF);
        assert!(doc.undo(), "undo the pixels");
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick") & 0xFF, 0);
        assert!(doc.undo(), "undo the layer");
        assert_eq!(doc.layer_count(), 1);
    }
}
