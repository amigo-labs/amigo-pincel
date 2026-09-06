//! Transform surface (Fineliner parity): flip / rotate the active cel or
//! the whole canvas, resize the canvas around an anchor, crop to the
//! selection, and scale the image.
//!
//! Orientation, anchor and interpolation names are the snake_case wire
//! names of the core enums (`flip_horizontal`, `rotate_90_cw`, `center`,
//! `bilinear`, …). Canvas-level commands can change the sprite size; JS
//! must re-read `width` / `height` afterwards. They also clear the
//! selection, so a `selection-changed` event accompanies `dirty-canvas`.

use pincel_core::{
    Anchor, Interpolation, Orientation, Rect, ReframeCanvas, ScaleImage, TransformCanvas,
    TransformCel,
};
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

#[wasm_bindgen]
impl Document {
    /// Flip or rotate the active layer's cel at the current frame. With a
    /// marquee selection only the selected pixels turn (about the
    /// selection's centre). `orientation` is one of `flip_horizontal`,
    /// `flip_vertical`, `rotate_90_cw`, `rotate_90_ccw`, `rotate_180`.
    /// One undo entry; emits a dirty event.
    #[wasm_bindgen(js_name = transformLayer)]
    pub fn transform_layer(&mut self, orientation: &str) -> Result<(), String> {
        let op = parse_orientation(orientation)?;
        let layer = self.paint_target_layer()?;
        let frame = self.current_frame;
        self.ensure_paint_cel(layer, frame);
        let mut cmd = TransformCel::new(layer, frame, op);
        if let Some(sel) = self.sprite.selection.filter(|r| !r.is_empty()) {
            cmd = cmd.within(sel);
        }
        self.run_sealed(cmd.into(), "failed to transform layer")?;
        if let Some(ev) = Event::from_dirty(self.bus.last_dirty_region()) {
            self.events.push(ev);
        }
        Ok(())
    }

    /// Flip or rotate the whole sprite (every layer and frame, slices
    /// included). Quarter turns swap `width` / `height`. Clears the
    /// selection. Errors when the document has a tilemap layer.
    #[wasm_bindgen(js_name = transformCanvas)]
    pub fn transform_canvas(&mut self, orientation: &str) -> Result<(), String> {
        let op = parse_orientation(orientation)?;
        self.run_sealed(
            TransformCanvas::new(op).into(),
            "failed to transform canvas",
        )?;
        self.push_canvas_changed();
        Ok(())
    }

    /// Resize the canvas to `width × height`, pinning the existing content
    /// at `anchor` (`top_left`, `top`, `top_right`, `left`, `center`,
    /// `right`, `bottom_left`, `bottom`, `bottom_right`). Content outside
    /// the new frame is dropped (undo restores it). Clears the selection.
    #[wasm_bindgen(js_name = resizeCanvas)]
    pub fn resize_canvas(&mut self, width: u32, height: u32, anchor: &str) -> Result<(), String> {
        let anchor =
            Anchor::from_name(anchor).ok_or_else(|| format!("unknown anchor `{anchor}`"))?;
        let cmd =
            ReframeCanvas::resize(self.sprite.width, self.sprite.height, width, height, anchor);
        self.run_sealed(cmd.into(), "failed to resize canvas")?;
        self.push_canvas_changed();
        Ok(())
    }

    /// Crop the canvas to the marquee selection (intersected with the
    /// canvas). Returns `Ok(false)` when there is no usable selection.
    #[wasm_bindgen(js_name = cropToSelection)]
    pub fn crop_to_selection(&mut self) -> Result<bool, String> {
        let Some(sel) = self.sprite.selection.filter(|r| !r.is_empty()) else {
            return Ok(false);
        };
        let rect = sel.intersect(Rect::new(0, 0, self.sprite.width, self.sprite.height));
        if rect.is_empty() {
            return Ok(false);
        }
        self.run_sealed(ReframeCanvas::new(rect).into(), "failed to crop")?;
        self.push_canvas_changed();
        Ok(true)
    }

    /// Resample the whole sprite to `width × height` with `interpolation`
    /// (`nearest`, `bilinear`, `bicubic`). Clears the selection. Errors
    /// when the document has a tilemap layer.
    #[wasm_bindgen(js_name = scaleImage)]
    pub fn scale_image(
        &mut self,
        width: u32,
        height: u32,
        interpolation: &str,
    ) -> Result<(), String> {
        let filter = Interpolation::from_name(interpolation)
            .ok_or_else(|| format!("unknown interpolation `{interpolation}`"))?;
        self.run_sealed(
            ScaleImage::new(width, height, filter).into(),
            "failed to scale image",
        )?;
        self.push_canvas_changed();
        Ok(())
    }
}

impl Document {
    /// Execute `cmd` as its own undo entry (sealed on both sides).
    fn run_sealed(&mut self, cmd: pincel_core::AnyCommand, what: &str) -> Result<(), String> {
        self.bus.seal();
        let result = self
            .bus
            .execute(cmd, &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("{what}: {e}"));
        self.bus.seal();
        result
    }

    /// After a canvas-level command: full repaint plus the (cleared)
    /// selection.
    fn push_canvas_changed(&mut self) {
        self.events.push(Event::dirty_canvas());
        self.events.push(Event::selection_changed(0, 0, 0, 0));
    }
}

fn parse_orientation(name: &str) -> Result<Orientation, String> {
    Orientation::from_name(name).ok_or_else(|| format!("unknown orientation `{name}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(doc: &Document, x: i32, y: i32) -> u32 {
        doc.pick_color(0, x, y).expect("pick")
    }

    #[test]
    fn transform_layer_flips_and_respects_selection() {
        let mut doc = Document::new(4, 1).expect("dims");
        doc.apply_tool("pencil", 0, 0, 0xFF0000FF).expect("paint");
        doc.end_stroke();
        doc.transform_layer("flip_horizontal").expect("flip");
        assert_eq!(px(&doc, 3, 0), 0xFF0000FF);
        assert_eq!(px(&doc, 0, 0) & 0xFF, 0);
        doc.set_selection(2, 0, 2, 1);
        doc.transform_layer("flip_horizontal").expect("flip");
        assert_eq!(
            px(&doc, 2, 0),
            0xFF0000FF,
            "flipped inside the selection only"
        );
        assert!(doc.undo());
        assert_eq!(px(&doc, 3, 0), 0xFF0000FF);
        assert!(doc.transform_layer("spin").is_err());
    }

    #[test]
    fn transform_canvas_rotate_swaps_dimensions() {
        let mut doc = Document::new(3, 2).expect("dims");
        doc.apply_tool("pencil", 0, 0, 0x00FF00FF).expect("paint");
        doc.end_stroke();
        doc.transform_canvas("rotate_90_cw").expect("rotate");
        assert_eq!((doc.width(), doc.height()), (2, 3));
        assert_eq!(px(&doc, 1, 0), 0x00FF00FF);
        assert!(doc.undo());
        assert_eq!((doc.width(), doc.height()), (3, 2));
    }

    #[test]
    fn resize_canvas_and_crop_change_dimensions_and_clear_selection() {
        let mut doc = Document::new(2, 2).expect("dims");
        doc.apply_tool("pencil", 1, 1, 0x0000FFFF).expect("paint");
        doc.end_stroke();
        doc.resize_canvas(4, 4, "top_left").expect("resize");
        assert_eq!((doc.width(), doc.height()), (4, 4));
        assert_eq!(px(&doc, 1, 1), 0x0000FFFF);
        assert!(doc.resize_canvas(1, 1, "middle").is_err());
        doc.set_selection(1, 1, 2, 2);
        doc.drain_events();
        assert!(doc.crop_to_selection().expect("crop"));
        assert_eq!((doc.width(), doc.height()), (2, 2));
        assert_eq!(px(&doc, 0, 0), 0x0000FFFF);
        assert!(!doc.has_selection());
        let kinds: Vec<String> = doc.drain_events().iter().map(|e| e.kind()).collect();
        assert!(kinds.contains(&"dirty-canvas".to_owned()));
        assert!(kinds.contains(&"selection-changed".to_owned()));
        assert!(!doc.crop_to_selection().expect("no selection"));
    }

    #[test]
    fn scale_image_resamples() {
        let mut doc = Document::new(2, 2).expect("dims");
        doc.apply_tool("pencil", 0, 0, 0xFF00FFFF).expect("paint");
        doc.end_stroke();
        doc.scale_image(4, 4, "nearest").expect("scale");
        assert_eq!((doc.width(), doc.height()), (4, 4));
        assert_eq!(px(&doc, 1, 1), 0xFF00FFFF);
        assert!(doc.scale_image(4, 4, "lanczos").is_err());
        assert!(doc.undo());
        assert_eq!((doc.width(), doc.height()), (2, 2));
    }
}
