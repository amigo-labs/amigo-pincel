//! Shaped-selection surface (spec §13.1, Fineliner parity): ellipse,
//! polygon / lasso and magic-wand selections with replace / add /
//! subtract / intersect modes, plus select-all, invert, expand and
//! contract. The UI reads the coverage back through `selectionMask` to
//! draw marching ants around any shape.
//!
//! Every mutator emits one `selection-changed` event carrying the new
//! bounding box (all zeros when the selection became empty). Selection
//! remains transient state outside the undo stack, like the rect
//! selection it extends.

use pincel_core::{Rect, SelectionMask, SelectionMode, composite_canvas, layer_canvas};
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

#[wasm_bindgen]
impl Document {
    /// Combine a rectangle into the selection. `mode` is `replace`, `add`,
    /// `subtract` or `intersect`.
    #[wasm_bindgen(js_name = selectRect)]
    pub fn select_rect(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        mode: &str,
    ) -> Result<(), String> {
        let mode = parse_mode(mode)?;
        let (w, h) = (self.sprite.width, self.sprite.height);
        let shape = SelectionMask::rect(w, h, Rect::new(x, y, width, height));
        self.combine_selection(shape, mode);
        Ok(())
    }

    /// Combine the ellipse inscribed in the given rect into the selection.
    #[wasm_bindgen(js_name = selectEllipse)]
    pub fn select_ellipse(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        mode: &str,
    ) -> Result<(), String> {
        let mode = parse_mode(mode)?;
        let (w, h) = (self.sprite.width, self.sprite.height);
        let shape = SelectionMask::ellipse(w, h, Rect::new(x, y, width, height));
        self.combine_selection(shape, mode);
        Ok(())
    }

    /// Combine a filled polygon into the selection. `points` is a flat
    /// `[x0, y0, x1, y1, …]` list in sprite coordinates (lasso and
    /// polygonal lasso both end here); fewer than three points is a no-op
    /// for `add` / `subtract` and clears for `replace` / `intersect`.
    #[wasm_bindgen(js_name = selectPolygon)]
    pub fn select_polygon(&mut self, points: &[i32], mode: &str) -> Result<(), String> {
        let mode = parse_mode(mode)?;
        let pts: Vec<(i32, i32)> = points
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| (p[0], p[1]))
            .collect();
        let (w, h) = (self.sprite.width, self.sprite.height);
        let shape = SelectionMask::polygon(w, h, &pts);
        self.combine_selection(shape, mode);
        Ok(())
    }

    /// Magic wand at sprite `(x, y)`: pixels within `tolerance` (0–255,
    /// per channel) of the clicked colour, 4-connected when `contiguous`.
    /// `sample` is `layer` (the active layer's cel at the current frame)
    /// or `composite` (what is on screen).
    #[wasm_bindgen(js_name = selectWand)]
    pub fn select_wand(
        &mut self,
        x: i32,
        y: i32,
        tolerance: u8,
        contiguous: bool,
        sample: &str,
        mode: &str,
    ) -> Result<(), String> {
        let mode = parse_mode(mode)?;
        let buffer = match sample {
            "composite" => composite_canvas(&self.sprite, &self.cels, self.current_frame)
                .map_err(|e| format!("failed to compose: {e}"))?,
            "layer" => layer_canvas(
                &self.sprite,
                &self.cels,
                self.paint_target_layer()?,
                self.current_frame,
            ),
            other => return Err(format!("unknown wand sample source `{other}`")),
        };
        let shape = SelectionMask::wand(&buffer, (x, y), tolerance, contiguous);
        self.combine_selection(shape, mode);
        Ok(())
    }

    /// Select the whole canvas.
    #[wasm_bindgen(js_name = selectAll)]
    pub fn select_all(&mut self) {
        let (w, h) = (self.sprite.width, self.sprite.height);
        self.sprite.set_selection(Rect::new(0, 0, w, h));
        self.push_selection_event();
    }

    /// Invert the selection over the canvas (everything when nothing was
    /// selected).
    #[wasm_bindgen(js_name = invertSelection)]
    pub fn invert_selection(&mut self) {
        let (w, h) = (self.sprite.width, self.sprite.height);
        let mut mask = self
            .sprite
            .selection_as_mask()
            .unwrap_or_else(|| SelectionMask::empty(w, h));
        mask.invert();
        self.sprite.set_selection_mask(mask);
        self.push_selection_event();
    }

    /// Grow the selection by `radius` pixels. No-op without a selection.
    #[wasm_bindgen(js_name = expandSelection)]
    pub fn expand_selection(&mut self, radius: u32) {
        if let Some(mask) = self.sprite.selection_as_mask() {
            self.sprite.set_selection_mask(mask.expand(radius));
            self.push_selection_event();
        }
    }

    /// Shrink the selection by `radius` pixels (may clear it).
    #[wasm_bindgen(js_name = contractSelection)]
    pub fn contract_selection(&mut self, radius: u32) {
        if let Some(mask) = self.sprite.selection_as_mask() {
            self.sprite.set_selection_mask(mask.contract(radius));
            self.push_selection_event();
        }
    }

    /// `true` when the active selection is a plain rectangle (or there is
    /// none) — the UI can then draw the cheap rect marquee.
    #[wasm_bindgen(getter, js_name = selectionIsRect)]
    pub fn selection_is_rect(&self) -> bool {
        self.sprite.selection_mask.is_none()
    }

    /// Canvas-sized coverage bytes (`0` / `255`, row-major) of the active
    /// selection, or an empty array when nothing is selected. A rect
    /// selection is materialised on demand.
    #[wasm_bindgen(js_name = selectionMask)]
    pub fn selection_mask(&self) -> Vec<u8> {
        self.sprite
            .selection_as_mask()
            .map(|m| m.data().to_vec())
            .unwrap_or_default()
    }

    /// Replace the selection with canvas-sized coverage bytes, as
    /// `selectionMask` returns them (non-zero = selected). An empty array or
    /// an all-zero mask clears the selection. Restores a selection the UI
    /// saved earlier, e.g. for the Magic Wand link (pixel-selection-move
    /// AC-25). Errors when the length isn't `width × height`.
    #[wasm_bindgen(js_name = setSelectionMask)]
    pub fn set_selection_mask(&mut self, mask: &[u8]) -> Result<(), String> {
        let (w, h) = (self.sprite.width, self.sprite.height);
        if mask.is_empty() {
            self.sprite.clear_selection();
        } else {
            let data = mask.iter().map(|&v| if v == 0 { 0 } else { 255 }).collect();
            let mask = SelectionMask::from_raw(w, h, data)
                .ok_or_else(|| format!("selection mask must be {w}×{h} bytes"))?;
            self.sprite.set_selection_mask(mask);
        }
        self.push_selection_event();
        Ok(())
    }
}

impl Document {
    /// Merge `shape` into the current selection per `mode` and announce it.
    fn combine_selection(&mut self, shape: SelectionMask, mode: SelectionMode) {
        self.sprite.combine_selection(shape, mode);
        self.push_selection_event();
    }

    /// Emit `selection-changed` with the current bounds.
    pub(crate) fn push_selection_event(&mut self) {
        let ev = match self.sprite.selection {
            Some(r) => Event::selection_changed(r.x, r.y, r.width, r.height),
            None => Event::selection_changed(0, 0, 0, 0),
        };
        self.events.push(ev);
    }
}

fn parse_mode(name: &str) -> Result<SelectionMode, String> {
    SelectionMode::from_name(name).ok_or_else(|| format!("unknown selection mode `{name}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(doc: &Document) -> usize {
        doc.selection_mask().iter().filter(|&&v| v != 0).count()
    }

    #[test]
    fn set_selection_mask_restores_a_shaped_selection() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.select_ellipse(0, 0, 4, 4, "replace").expect("select");
        let saved = doc.selection_mask();
        doc.select_rect(0, 0, 1, 1, "replace").expect("select");
        doc.drain_events();

        doc.set_selection_mask(&saved).expect("restore");
        assert_eq!(doc.selection_mask(), saved);
        assert!(!doc.selection_is_rect());
        assert_eq!((doc.selection_width(), doc.selection_height()), (4, 4));
        assert!(
            doc.drain_events()
                .iter()
                .any(|e| e.kind() == "selection-changed" && e.width() == 4)
        );
    }

    #[test]
    fn set_selection_mask_with_an_empty_mask_clears_the_selection() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.select_rect(0, 0, 2, 2, "replace").expect("select");
        doc.set_selection_mask(&[]).expect("clear");
        assert!(!doc.has_selection());
        doc.select_rect(0, 0, 2, 2, "replace").expect("select");
        doc.set_selection_mask(&[0; 16]).expect("clear");
        assert!(!doc.has_selection());
    }

    #[test]
    fn set_selection_mask_of_the_wrong_size_returns_err() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.select_rect(0, 0, 2, 2, "replace").expect("select");
        assert!(doc.set_selection_mask(&[255; 15]).is_err());
        assert_eq!((doc.selection_width(), doc.selection_height()), (2, 2));
    }

    #[test]
    fn select_ellipse_sets_bounds_and_a_shaped_mask() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.select_ellipse(0, 0, 4, 4, "replace").expect("select");
        assert!(doc.has_selection());
        assert!(!doc.selection_is_rect());
        assert_eq!((doc.selection_x(), doc.selection_y()), (0, 0));
        assert_eq!((doc.selection_width(), doc.selection_height()), (4, 4));
        assert_eq!(count(&doc), 12, "4×4 ellipse drops the four corners");
        let ev = doc.drain_events();
        assert!(
            ev.iter()
                .any(|e| e.kind() == "selection-changed" && e.width() == 4)
        );
    }

    #[test]
    fn modes_combine_and_a_rectangular_result_collapses_to_a_rect() {
        let mut doc = Document::new(4, 1).expect("dims");
        doc.select_rect(0, 0, 2, 1, "replace").expect("select");
        doc.select_rect(1, 0, 2, 1, "add").expect("add");
        assert_eq!(count(&doc), 3);
        assert!(doc.selection_is_rect());
        doc.select_rect(1, 0, 1, 1, "subtract").expect("sub");
        assert_eq!(count(&doc), 2);
        assert!(!doc.selection_is_rect(), "0 and 2 selected with a hole");
        doc.select_rect(2, 0, 2, 1, "intersect").expect("intersect");
        assert_eq!(count(&doc), 1);
        assert_eq!(doc.selection_x(), 2);
        assert!(doc.select_rect(0, 0, 1, 1, "xor").is_err());
    }

    #[test]
    fn subtract_or_intersect_with_nothing_selected_stays_empty() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.select_rect(0, 0, 2, 2, "subtract").expect("sub");
        assert!(!doc.has_selection());
        doc.select_rect(0, 0, 2, 2, "intersect").expect("intersect");
        assert!(!doc.has_selection());
    }

    #[test]
    fn select_polygon_lasso_triangle() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.select_polygon(&[0, 0, 3, 0, 0, 3], "replace")
            .expect("poly");
        assert!(doc.has_selection());
        assert!(!doc.selection_is_rect());
        let mask = doc.selection_mask();
        assert_eq!(mask[0], 255);
        assert_eq!(mask[15], 0);
    }

    #[test]
    fn select_wand_on_layer_and_composite() {
        let mut doc = Document::new(3, 1).expect("dims");
        doc.apply_tool("pencil", 0, 0, 0xFF0000FF).expect("paint");
        doc.apply_tool("pencil", 2, 0, 0xFF0000FF).expect("paint");
        doc.end_stroke();
        doc.select_wand(0, 0, 0, true, "layer", "replace")
            .expect("wand");
        assert_eq!(count(&doc), 1);
        doc.select_wand(0, 0, 0, false, "composite", "replace")
            .expect("wand");
        assert_eq!(count(&doc), 2);
        // Transparent seed selects the transparent run.
        doc.select_wand(1, 0, 0, true, "layer", "replace")
            .expect("wand");
        assert_eq!(count(&doc), 1);
        assert_eq!(doc.selection_x(), 1);
        assert!(doc.select_wand(0, 0, 0, true, "screen", "replace").is_err());
    }

    #[test]
    fn select_all_invert_expand_contract() {
        let mut doc = Document::new(5, 5).expect("dims");
        doc.select_all();
        assert_eq!(count(&doc), 25);
        assert!(doc.selection_is_rect());
        doc.invert_selection();
        assert!(!doc.has_selection());
        doc.select_rect(2, 2, 1, 1, "replace").expect("select");
        doc.expand_selection(1);
        assert_eq!(count(&doc), 9);
        doc.contract_selection(1);
        assert_eq!(count(&doc), 1);
        doc.invert_selection();
        assert_eq!(count(&doc), 24);
        doc.contract_selection(3);
        assert!(!doc.has_selection());
    }

    #[test]
    fn invert_without_selection_selects_the_whole_canvas() {
        let mut doc = Document::new(3, 2).expect("dims");
        assert!(!doc.has_selection());
        doc.invert_selection();
        assert_eq!(count(&doc), 6);
    }

    #[test]
    fn delete_and_move_honour_the_shaped_mask() {
        let mut doc = Document::new(3, 1).expect("dims");
        for x in 0..3 {
            doc.apply_tool("pencil", x, 0, 0x00FF00FF).expect("paint");
        }
        doc.end_stroke();
        // Select pixels 0 and 2 (hole at 1) via subtract.
        doc.select_rect(0, 0, 3, 1, "replace").expect("select");
        doc.select_rect(1, 0, 1, 1, "subtract").expect("sub");
        assert!(doc.delete_selection().expect("delete"));
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick") & 0xFF, 0);
        assert_eq!(
            doc.pick_color(0, 1, 0).expect("pick"),
            0x00FF00FF,
            "hole survives"
        );
        assert_eq!(doc.pick_color(0, 2, 0).expect("pick") & 0xFF, 0);
        assert!(doc.undo());
        // Move the shaped selection right by one: pixel 1 is not selected
        // so it stays; pixel 0 lands on 1, pixel 2 falls off the canvas.
        doc.apply_move_selection(1, 0).expect("move");
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick") & 0xFF, 0);
        assert_eq!(doc.pick_color(0, 1, 0).expect("pick"), 0x00FF00FF);
        assert!(!doc.selection_is_rect());
        let mask = doc.selection_mask();
        assert_eq!(&mask[..], &[0, 255, 0], "mask moved with the pixels");
    }

    #[test]
    fn effects_honour_the_shaped_mask() {
        let mut doc = Document::new(3, 1).expect("dims");
        for x in 0..3 {
            doc.apply_tool("pencil", x, 0, 0xFF0000FF).expect("paint");
        }
        doc.end_stroke();
        doc.select_rect(0, 0, 3, 1, "replace").expect("select");
        doc.select_rect(1, 0, 1, 1, "subtract").expect("sub");
        doc.apply_effect("invert", &[]).expect("invert");
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick"), 0x00FFFFFF);
        assert_eq!(doc.pick_color(0, 1, 0).expect("pick"), 0xFF0000FF);
        assert_eq!(doc.pick_color(0, 2, 0).expect("pick"), 0x00FFFFFF);
    }
}
