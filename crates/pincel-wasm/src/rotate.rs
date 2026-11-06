//! Layer ▸ Rotate…: turn the active layer's cel at the current frame (or
//! the selection content) by any angle with nearest neighbour or
//! RotSprite, plus a live preview. See `docs/specs/rotsprite.md`.

use pincel_core::{
    CommandError, ComposeRequest, ROTSPRITE_MAX_SIDE, RotateCel, RotateMethod, compose,
};
use wasm_bindgen::prelude::*;

use crate::events::Event;
use crate::{ComposeFrame, Document};

#[wasm_bindgen]
impl Document {
    /// Largest input width or height RotSprite accepts (512).
    #[wasm_bindgen(js_name = rotspriteMaxSide)]
    pub fn rotsprite_max_side(&self) -> u32 {
        ROTSPRITE_MAX_SIDE
    }

    /// `[width, height]` of what `rotateLayer` would turn — the selection
    /// box, or the bounding box of the cel's visible pixels — or an empty
    /// array when there is nothing to rotate.
    #[wasm_bindgen(js_name = rotateInputSize)]
    pub fn rotate_input_size(&self) -> Vec<u32> {
        let Ok(layer) = self.paint_target_layer() else {
            return Vec::new();
        };
        let cmd = self.rotate_command(layer, 0, RotateMethod::Nearest);
        match cmd.input_rect(&self.cels) {
            Ok(Some(r)) => vec![r.width, r.height],
            _ => Vec::new(),
        }
    }

    /// Rotate the active layer's cel at the current frame clockwise by
    /// `angle` degrees with `method` (`nearest` or `rotsprite`); with a
    /// selection only its content turns. The cel grows to hold the
    /// result. One undo entry. `Ok(false)` when there is nothing visible
    /// to rotate (nothing changes); errors on a locked layer, an unknown
    /// method or a RotSprite input over [`Document::rotsprite_max_side`].
    #[wasm_bindgen(js_name = rotateLayer)]
    pub fn rotate_layer(&mut self, angle: i32, method: &str) -> Result<bool, String> {
        let method = parse_method(method)?;
        let layer = self.edit_target_layer()?;
        let cmd = self.rotate_command(layer, angle, method);
        if !matches!(cmd.input_rect(&self.cels), Ok(Some(_))) {
            return Ok(false);
        }
        self.bus.seal();
        let result = self
            .bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels);
        self.bus.seal();
        match result {
            Ok(()) => {}
            Err(CommandError::EmptyRegion) => return Ok(false),
            Err(e) => return Err(format!("failed to rotate layer: {e}")),
        }
        if let Some(ev) = Event::from_dirty(self.bus.last_dirty_region()) {
            self.events.push(ev);
        }
        Ok(true)
    }

    /// The composite as `rotateLayer(angle, method)` would leave it,
    /// without touching the document or the undo stack. With nothing to
    /// rotate it shows the document unchanged.
    #[wasm_bindgen(js_name = previewRotate)]
    pub fn preview_rotate(
        &self,
        angle: i32,
        method: &str,
        zoom: u32,
    ) -> Result<ComposeFrame, String> {
        let method = parse_method(method)?;
        let layer = self.edit_target_layer()?;
        let mut cmd = self.rotate_command(layer, angle, method);
        let mut sprite = self.sprite.clone();
        let mut cels = self.cels.clone();
        match pincel_core::Command::apply(&mut cmd, &mut sprite, &mut cels) {
            Ok(()) | Err(CommandError::EmptyRegion | CommandError::MissingCel { .. }) => {}
            Err(e) => return Err(format!("failed to preview rotation: {e}")),
        }
        let mut request =
            ComposeRequest::full(self.current_frame, self.sprite.width, self.sprite.height);
        request.zoom = zoom;
        let mut pixels = Vec::new();
        let result = compose(&sprite, &cels, &request, &mut pixels)
            .map_err(|e| format!("failed to compose preview: {e}"))?;
        Ok(ComposeFrame {
            width: result.width,
            height: result.height,
            dirty_x: result.dirty_rect.x,
            dirty_y: result.dirty_rect.y,
            pixels,
        })
    }
}

impl Document {
    /// The rotation of `layer`'s cel at the current frame, restricted to
    /// the selection (rect and shape) when one exists.
    fn rotate_command(
        &self,
        layer: pincel_core::LayerId,
        angle: i32,
        method: RotateMethod,
    ) -> RotateCel {
        let mut cmd = RotateCel::new(layer, self.current_frame, angle, method);
        if let Some(sel) = self.sprite.selection.filter(|r| !r.is_empty()) {
            cmd = cmd.within(sel);
            if let Some(mask) = &self.sprite.selection_mask {
                cmd = cmd.masked(mask.clone());
            }
        }
        cmd
    }
}

fn parse_method(name: &str) -> Result<RotateMethod, String> {
    RotateMethod::from_name(name).ok_or_else(|| format!("unknown rotation method `{name}`"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pincel_core::{FrameIndex, LayerId};

    const RED: u32 = 0xFF00_00FF;
    const BLUE: u32 = 0x0000_FFFF;
    const GREEN: u32 = 0x00FF_00FF;

    fn px(doc: &Document, x: i32, y: i32) -> u32 {
        doc.pick_color(doc.current_frame.0, x, y).expect("pick")
    }

    fn paint(doc: &mut Document, x: i32, y: i32, color: u32) {
        doc.apply_tool("pencil", x, y, color).expect("paint");
        doc.end_stroke();
    }

    /// 6×6 document with a red 3×1 bar at (1..4, 2) on layer 0.
    fn bar_doc() -> Document {
        let mut doc = Document::new(6, 6).expect("dims");
        for x in 1..4 {
            paint(&mut doc, x, 2, RED);
        }
        doc
    }

    #[test]
    fn rotate_layer_turns_only_the_active_cel_in_one_undo_step() {
        let mut doc = bar_doc();
        let other = doc.add_layer("other").expect("layer");
        doc.set_active_layer(other);
        // Two-pixel bars elsewhere, so a stray rotation would show.
        paint(&mut doc, 0, 0, BLUE);
        paint(&mut doc, 1, 0, BLUE);
        let frame = doc.add_frame(100).expect("frame");
        doc.set_current_frame(frame).expect("go");
        paint(&mut doc, 4, 4, BLUE);
        paint(&mut doc, 5, 4, BLUE);
        doc.set_active_layer(0);
        paint(&mut doc, 1, 2, GREEN);
        paint(&mut doc, 2, 2, GREEN);
        doc.set_current_frame(0).expect("back");
        let others: Vec<_> = [(other, 0), (0, frame), (other, frame)]
            .iter()
            .map(|&(l, f)| doc.cels.get(LayerId::new(l), FrameIndex::new(f)).cloned())
            .collect();
        let depth = doc.undo_depth();
        assert_eq!(doc.rotate_input_size(), vec![3, 1]);
        assert_eq!(doc.rotate_layer(90, "rotsprite"), Ok(true));
        assert_eq!(doc.undo_depth(), depth + 1, "one undo entry");
        // The bar turned upright about its centre (2, 2).
        for y in 1..4 {
            assert_eq!(px(&doc, 2, y), RED, "(2,{y})");
        }
        assert_eq!(px(&doc, 1, 2) & 0xFF, 0);
        for (i, &(l, f)) in [(other, 0), (0, frame), (other, frame)].iter().enumerate() {
            let now = doc.cels.get(LayerId::new(l), FrameIndex::new(f)).cloned();
            assert_eq!(now, others[i], "layer {l} frame {f} untouched");
        }
        assert!(doc.undo());
        assert_eq!(px(&doc, 1, 2), RED);
        assert_eq!(px(&doc, 2, 1) & 0xFF, 0);
        assert!(doc.rotate_layer(30, "bilinear").is_err());
    }

    #[test]
    fn rotate_layer_with_a_selection_turns_only_its_content() {
        let mut doc = bar_doc();
        paint(&mut doc, 5, 5, BLUE);
        doc.set_selection(1, 2, 3, 1);
        assert_eq!(doc.rotate_input_size(), vec![3, 1]);
        assert_eq!(doc.rotate_layer(-90, "nearest"), Ok(true));
        assert_eq!(px(&doc, 2, 1), RED);
        assert_eq!(px(&doc, 2, 3), RED);
        assert_eq!(px(&doc, 5, 5), BLUE, "outside the selection");
    }

    #[test]
    fn rotate_layer_on_a_locked_layer_or_without_content_changes_nothing() {
        let mut doc = bar_doc();
        let before = doc.cels.clone();
        let depth = doc.undo_depth();
        doc.set_layer_editable(0, false).expect("lock");
        assert!(doc.rotate_layer(45, "nearest").is_err());
        assert!(doc.preview_rotate(45, "nearest", 1).is_err());
        doc.set_layer_editable(0, true).expect("unlock");
        let depth_unlocked = doc.undo_depth();
        // A selection over transparent pixels: nothing to rotate.
        doc.set_selection(4, 4, 2, 2);
        assert!(doc.rotate_input_size().is_empty());
        assert_eq!(doc.rotate_layer(45, "rotsprite"), Ok(false));
        assert_eq!(doc.cels, before);
        assert_eq!(doc.undo_depth(), depth_unlocked);
        assert!(depth_unlocked >= depth);
        // A fresh layer without a cel.
        let fresh = doc.add_layer("fresh").expect("layer");
        doc.set_active_layer(fresh);
        doc.clear_selection();
        let depth = doc.undo_depth();
        assert!(doc.rotate_input_size().is_empty());
        assert_eq!(doc.rotate_layer(45, "nearest"), Ok(false));
        assert_eq!(doc.undo_depth(), depth);
    }

    #[test]
    fn rotate_layer_refuses_rotsprite_over_the_cap() {
        let mut doc = Document::new(520, 4).expect("dims");
        paint(&mut doc, 0, 0, RED);
        paint(&mut doc, 512, 0, RED);
        assert_eq!(doc.rotate_input_size(), vec![513, 1]);
        assert!(doc.rotsprite_max_side() < 513);
        let before = doc.cels.clone();
        let depth = doc.undo_depth();
        let err = doc.rotate_layer(30, "rotsprite").expect_err("over the cap");
        assert!(err.contains("512"), "{err}");
        assert_eq!(doc.cels, before);
        assert_eq!(doc.undo_depth(), depth);
        assert_eq!(doc.rotate_layer(30, "nearest"), Ok(true));
    }

    #[test]
    fn preview_rotate_leaves_document_and_history_untouched() {
        let doc_before = bar_doc();
        let mut doc = bar_doc();
        let depth = doc.undo_depth();
        let frame = doc.preview_rotate(90, "rotsprite", 1).expect("preview");
        assert_eq!((frame.width(), frame.height()), (6, 6));
        let pixels = frame.pixels();
        let at = |x: usize, y: usize| &pixels[(y * 6 + x) * 4..(y * 6 + x) * 4 + 4];
        assert_eq!(at(2, 1), &[255, 0, 0, 255], "preview shows the turned bar");
        assert_eq!(at(1, 2)[3], 0);
        assert_eq!(doc.cels, doc_before.cels, "document unchanged");
        assert_eq!(doc.undo_depth(), depth);
        // Nothing to rotate: the preview shows the document as it is.
        doc.set_selection(4, 4, 2, 2);
        let frame = doc.preview_rotate(45, "nearest", 1).expect("preview");
        let pixels = frame.pixels();
        assert_eq!(
            &pixels[(2 * 6 + 1) * 4..(2 * 6 + 1) * 4 + 4],
            &[255, 0, 0, 255]
        );
        doc.clear_selection();
        assert_eq!(doc.rotate_layer(90, "rotsprite"), Ok(true));
        assert!(doc.undo());
    }
}
