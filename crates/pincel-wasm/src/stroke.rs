//! Pencil / Eraser gesture API: `beginStroke` / `strokeTo` / `endStroke`
//! drive one [`PencilStroke`] per press-drag-release (spec §9.3, Decision
//! Log 2026-10-29). `applyTool` stays for single pixels.

use pincel_core::{BrushShape, FrameIndex, LayerId, PencilStroke, PixelBrush, Point, Rgba};
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

/// Parameters of the gesture between `beginStroke` and `endStroke`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LiveStroke {
    layer: LayerId,
    frame: FrameIndex,
    color: Rgba,
    brush: PixelBrush,
    pixel_perfect: bool,
    /// A `PencilStroke` for this gesture is on the bus.
    started: bool,
}

#[wasm_bindgen]
impl Document {
    /// Start a Pencil / Eraser gesture on the active layer / current frame.
    /// Nothing is painted until the first [`Self::stroke_to`].
    ///
    /// - `tool`: `"pencil"` paints `color` (`0xRRGGBBAA`); `"eraser"`
    ///   clears to transparent and ignores `color`.
    /// - `size`: brush size, clamped to 1–8.
    /// - `shape`: `"square"` or `"round"`.
    /// - `pixel_perfect`: drop L-corner pixels; only at size 1.
    ///
    /// Errors (and starts nothing) for an unknown tool or shape, a locked
    /// target layer, or a document without a paintable image layer.
    #[wasm_bindgen(js_name = beginStroke)]
    pub fn begin_stroke(
        &mut self,
        tool: &str,
        color: u32,
        size: u32,
        shape: &str,
        pixel_perfect: bool,
    ) -> Result<(), String> {
        self.stroke = None;
        let color = match tool {
            "pencil" => Rgba::from_u32(color),
            "eraser" => Rgba::TRANSPARENT,
            _ => return Err(format!("unknown tool: {tool}")),
        };
        let shape = match shape {
            "square" => BrushShape::Square,
            "round" => BrushShape::Round,
            _ => return Err(format!("unknown brush shape: {shape}")),
        };
        let layer = self.edit_target_layer()?;
        let size = u8::try_from(size).unwrap_or(u8::MAX);
        self.bus.seal();
        self.stroke = Some(LiveStroke {
            layer,
            frame: self.current_frame,
            color,
            brush: PixelBrush::new(size, shape),
            pixel_perfect,
            started: false,
        });
        Ok(())
    }

    /// Extend the gesture to sprite-space `(x, y)`: the first call stamps
    /// the brush there, every later call stamps it at each pixel of the
    /// straight line from the previous point. Pixels outside the canvas
    /// are skipped silently. The whole gesture is one undo entry; emits a
    /// `dirty-rect` for the painted segment.
    #[wasm_bindgen(js_name = strokeTo)]
    pub fn stroke_to(&mut self, x: i32, y: i32) -> Result<(), String> {
        let Some(live) = self.stroke else {
            return Err("no stroke in progress: call beginStroke first".to_string());
        };
        let to = Point::new(x, y);
        let extended = live.started
            && self
                .bus
                .continue_stroke(to, 1.0, &mut self.cels)
                .map_err(|e| format!("failed to paint: {e}"))?;
        if !extended {
            // First point, or the history moved under the gesture (an
            // undo mid-drag): start a fresh stroke here.
            self.ensure_paint_cel(live.layer, live.frame);
            self.bus.seal();
            let cmd = PencilStroke::new(
                live.layer,
                live.frame,
                live.color,
                live.brush,
                live.pixel_perfect,
                to,
                1.0,
            );
            self.bus
                .execute(cmd.into(), &mut self.sprite, &mut self.cels)
                .map_err(|e| format!("failed to paint: {e}"))?;
            self.stroke = Some(LiveStroke {
                started: true,
                ..live
            });
        }
        if let Some(ev) = Event::from_dirty(self.bus.last_dirty_region()) {
            self.events.push(ev);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pincel_core::{Cel, ColorMode, Layer, PixelBuffer};

    use super::*;

    const RED: u32 = 0xff00_00ff;
    const BLUE: u32 = 0x0000_ffff;

    fn px(doc: &Document, x: i32, y: i32) -> u32 {
        doc.pick_color(0, x, y).expect("pick")
    }

    /// Every pixel of a `w`×`h` document equal to `c`.
    fn painted(doc: &Document, w: i32, h: i32, c: u32) -> Vec<(i32, i32)> {
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| px(doc, x, y) == c)
            .collect()
    }

    fn gesture(
        doc: &mut Document,
        tool: &str,
        size: u32,
        shape: &str,
        pp: bool,
        pts: &[(i32, i32)],
    ) {
        doc.begin_stroke(tool, RED, size, shape, pp).expect("begin");
        for &(x, y) in pts {
            doc.stroke_to(x, y).expect("stroke");
        }
        doc.end_stroke();
    }

    #[test]
    fn stroke_size_3_click_paints_centred_block() {
        let mut doc = Document::new(6, 6).expect("dims");
        gesture(&mut doc, "pencil", 3, "square", false, &[(2, 2)]);
        let want: Vec<_> = (1..=3).flat_map(|y| (1..=3).map(move |x| (x, y))).collect();
        assert_eq!(painted(&doc, 6, 6, RED), want);
    }

    #[test]
    fn stroke_round_size_3_click_paints_plus() {
        let mut doc = Document::new(6, 6).expect("dims");
        gesture(&mut doc, "pencil", 3, "round", false, &[(2, 2)]);
        assert_eq!(
            painted(&doc, 6, 6, RED),
            vec![(2, 1), (1, 2), (2, 2), (3, 2), (2, 3)]
        );
    }

    #[test]
    fn stroke_to_jump_paints_connected_line() {
        let mut doc = Document::new(8, 8).expect("dims");
        gesture(&mut doc, "pencil", 1, "square", false, &[(0, 0), (7, 7)]);
        let want: Vec<_> = (0..8).map(|i| (i, i)).collect();
        assert_eq!(painted(&doc, 8, 8, RED), want);
    }

    #[test]
    fn stroke_gesture_is_one_undo_entry_and_redo_reproduces_it() {
        let mut doc = Document::new(8, 8).expect("dims");
        gesture(
            &mut doc,
            "pencil",
            3,
            "square",
            false,
            &[(1, 1), (6, 6), (1, 6)],
        );
        assert_eq!(doc.undo_depth(), 1, "one gesture, one entry");
        let after = painted(&doc, 8, 8, RED);
        assert!(doc.undo());
        assert!(painted(&doc, 8, 8, RED).is_empty(), "whole stroke undone");
        assert!(doc.redo().expect("redo"));
        assert_eq!(painted(&doc, 8, 8, RED), after);
        gesture(&mut doc, "pencil", 1, "square", false, &[(0, 7)]);
        assert_eq!(doc.undo_depth(), 2, "two gestures, two entries");
    }

    #[test]
    fn stroke_beyond_canvas_paints_in_bounds_part_without_error() {
        let mut doc = Document::new(4, 4).expect("dims");
        gesture(
            &mut doc,
            "pencil",
            8,
            "square",
            false,
            &[(0, 0), (-9, -9), (20, 0)],
        );
        assert!(px(&doc, 0, 0) == RED && px(&doc, 3, 3) == RED);
        gesture(
            &mut doc,
            "pencil",
            1,
            "square",
            false,
            &[(-5, -5), (-1, -9)],
        );
        assert_eq!(doc.undo_depth(), 2);
    }

    #[test]
    fn eraser_stroke_clears_the_pencil_footprint() {
        let pts = [(1, 1), (6, 2), (2, 6)];
        let mut pencil = Document::new(8, 8).expect("dims");
        gesture(&mut pencil, "pencil", 2, "round", false, &pts);
        let footprint = painted(&pencil, 8, 8, RED);

        let mut doc = Document::new(8, 8).expect("dims");
        doc.begin_stroke("pencil", BLUE, 8, "square", false)
            .expect("begin");
        for y in (0..8).step_by(2) {
            doc.stroke_to(0, y).expect("fill");
            doc.stroke_to(8, y).expect("fill");
        }
        doc.end_stroke();
        assert!(painted(&doc, 8, 8, BLUE).len() == 64, "seeded opaque");
        doc.begin_stroke("eraser", RED, 2, "round", false)
            .expect("begin");
        for &(x, y) in &pts {
            doc.stroke_to(x, y).expect("erase");
        }
        doc.end_stroke();
        assert_eq!(painted(&doc, 8, 8, 0), footprint);
    }

    #[test]
    fn stroke_pixel_perfect_drops_staircase_corners() {
        let mut doc = Document::new(4, 4).expect("dims");
        gesture(
            &mut doc,
            "pencil",
            1,
            "square",
            true,
            &[(0, 0), (1, 0), (1, 1), (2, 1), (2, 2)],
        );
        assert_eq!(painted(&doc, 4, 4, RED), vec![(0, 0), (1, 1), (2, 2)]);
    }

    #[test]
    fn begin_stroke_on_locked_layer_errs_and_adds_no_undo_entry() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.set_layer_editable(0, false).expect("lock");
        let depth = doc.undo_depth();
        let err = doc
            .begin_stroke("pencil", RED, 1, "square", false)
            .expect_err("locked");
        assert!(err.contains("locked"));
        assert!(doc.stroke_to(1, 1).is_err(), "no stroke started");
        assert_eq!(doc.undo_depth(), depth);
        assert!(painted(&doc, 4, 4, RED).is_empty());
    }

    #[test]
    fn begin_stroke_without_image_layer_errs() {
        let mut doc = Document::new(2, 2).expect("dims");
        doc.sprite.layers.clear();
        doc.sprite
            .layers
            .push(Layer::group(LayerId::new(7), "folder"));
        let err = doc
            .begin_stroke("pencil", RED, 1, "square", false)
            .expect_err("nothing to paint");
        assert!(err.contains("no paintable image layer"));
        assert_eq!(doc.undo_depth(), 0);
    }

    #[test]
    fn stroke_skips_group_layer_to_paint_the_image_layer() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.sprite
            .layers
            .insert(0, Layer::group(LayerId::new(99), "folder"));
        doc.set_active_layer(99);
        gesture(&mut doc, "pencil", 1, "square", false, &[(1, 1)]);
        assert_eq!(px(&doc, 1, 1), RED);
    }

    #[test]
    fn stroke_on_new_layer_creates_its_cel() {
        let mut doc = Document::new(4, 4).expect("dims");
        let id = doc.add_layer("Layer 2").expect("add");
        doc.set_active_layer(id);
        let key = (LayerId::new(id), FrameIndex::new(0));
        assert!(doc.cels.get(key.0, key.1).is_none());
        gesture(&mut doc, "pencil", 1, "square", false, &[(1, 1)]);
        assert!(doc.cels.get(key.0, key.1).is_some());
        assert_eq!(px(&doc, 1, 1), RED);
        // Clearing the new layer's cel shows the default layer untouched.
        doc.cels.insert(Cel::image(
            key.0,
            key.1,
            PixelBuffer::empty(4, 4, ColorMode::Rgba),
        ));
        assert_eq!(px(&doc, 1, 1), 0);
    }

    #[test]
    fn begin_stroke_rejects_unknown_tool_and_shape() {
        let mut doc = Document::new(2, 2).expect("dims");
        assert!(doc.begin_stroke("bucket", RED, 1, "square", false).is_err());
        assert!(doc.begin_stroke("pencil", RED, 1, "star", false).is_err());
    }

    #[test]
    fn stroke_emits_dirty_rect() {
        let mut doc = Document::new(8, 8).expect("dims");
        let _ = doc.drain_events();
        gesture(&mut doc, "pencil", 3, "square", false, &[(4, 4)]);
        assert!(!doc.drain_events().is_empty());
    }
}
