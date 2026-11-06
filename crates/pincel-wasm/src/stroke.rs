//! Pencil / Eraser gesture API: `beginStroke` / `strokeTo` / `endStroke`
//! drive one [`PencilStroke`] per press-drag-release (spec §9.3, Decision
//! Log 2026-10-29). `beginShadingStroke` starts the Pencil with the
//! Shading ink (`docs/specs/shading-ink.md`). `applyTool` stays for single
//! pixels.

use pincel_core::{
    BrushShape, DirtyRegion, FrameIndex, Ink, LayerId, Mirror, PencilStroke, PixelBrush, Point,
    Rect, Rgba, Wrap,
};
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

/// Parameters of the gesture between `beginStroke` and `endStroke`.
#[derive(Debug, Clone)]
pub(crate) struct LiveStroke {
    layer: LayerId,
    frame: FrameIndex,
    color: Rgba,
    brush: PixelBrush,
    pixel_perfect: bool,
    ink: Ink,
    /// Symmetry axes (`setStrokeMirror`), if any.
    mirror: Option<Mirror>,
    /// Tiled-mode wrapping (`setStrokeWrap`).
    wrap: Wrap,
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
        self.start_stroke(color, size, shape, pixel_perfect, Ink::Simple)
    }

    /// Start a Pencil gesture with the Shading ink: every pixel the stroke
    /// touches whose color before the stroke is exactly entry `i` of `ramp`
    /// becomes entry `i + step`, clamped to the ramp; other colors stay.
    /// Each pixel changes at most once per gesture. Continue it with
    /// [`Self::stroke_to`] and close it with `endStroke` like any stroke.
    ///
    /// - `ramp`: the colors in palette order (`0xRRGGBBAA`), at least one.
    /// - `step`: `1` (left button, toward the end) or `-1` (right button,
    ///   toward the start).
    /// - `size`, `shape`, `pixel_perfect`: as in [`Self::begin_stroke`].
    ///
    /// Errors (and starts nothing) for an empty ramp, another step, an
    /// unknown shape, a locked target layer, or a document without a
    /// paintable image layer.
    #[wasm_bindgen(js_name = beginShadingStroke)]
    pub fn begin_shading_stroke(
        &mut self,
        ramp: &[u32],
        step: i32,
        size: u32,
        shape: &str,
        pixel_perfect: bool,
    ) -> Result<(), String> {
        self.stroke = None;
        if ramp.is_empty() {
            return Err("the shading ramp is empty".to_string());
        }
        let step = match step {
            1 => 1,
            -1 => -1,
            _ => return Err(format!("shading step must be 1 or -1, got {step}")),
        };
        let ink = Ink::Shading {
            ramp: ramp.iter().map(|&c| Rgba::from_u32(c)).collect(),
            step,
        };
        self.start_stroke(Rgba::TRANSPARENT, size, shape, pixel_perfect, ink)
    }

    /// Mirror the gesture in progress (symmetry): every pixel it paints is
    /// also painted at its mirror images. `axis_x2` / `axis_y2` are the
    /// vertical / horizontal axis in half pixels from the left / top canvas
    /// edge (column `x` mirrors to `axis_x2 − 1 − x`); absent turns that
    /// axis off. Mirror images off the canvas are skipped unless the
    /// gesture wraps. Call it after `beginStroke` / `beginShadingStroke`
    /// and before the first [`Self::stroke_to`].
    ///
    /// Errors when no gesture is armed or it has already painted.
    #[wasm_bindgen(js_name = setStrokeMirror)]
    pub fn set_stroke_mirror(
        &mut self,
        axis_x2: Option<i32>,
        axis_y2: Option<i32>,
    ) -> Result<(), String> {
        let live = self.unstarted_stroke()?;
        live.mirror = Some(Mirror { axis_x2, axis_y2 });
        Ok(())
    }

    /// Let the gesture in progress wrap across the canvas edges (tiled
    /// mode): along a wrapped axis, points beyond the canvas are taken
    /// modulo the canvas size instead of skipped, and the line between two
    /// points continues at the opposite edge. Call it after `beginStroke` /
    /// `beginShadingStroke` and before the first [`Self::stroke_to`].
    ///
    /// Errors when no gesture is armed or it has already painted.
    #[wasm_bindgen(js_name = setStrokeWrap)]
    pub fn set_stroke_wrap(&mut self, x: bool, y: bool) -> Result<(), String> {
        let live = self.unstarted_stroke()?;
        live.wrap = Wrap { x, y };
        Ok(())
    }

    /// Extend the gesture to sprite-space `(x, y)`: the first call stamps
    /// the brush there, every later call stamps it at each pixel of the
    /// straight line from the previous point. Pixels outside the canvas
    /// are skipped silently. The whole gesture is one undo entry; emits a
    /// `dirty-rect` for the painted segment.
    ///
    /// `pressure` is the optional pen pressure at `(x, y)`: absent means
    /// full pressure (mouse, touch, or the Size option off). It is clamped
    /// to 0–1, and each stamp has size `max(1, round(size × p))` with `p`
    /// interpolated linearly between consecutive points.
    #[wasm_bindgen(js_name = strokeTo)]
    pub fn stroke_to(&mut self, x: i32, y: i32, pressure: Option<f32>) -> Result<(), String> {
        let dirty = self.stroke_point(x, y, pressure.unwrap_or(1.0))?;
        if let Some(ev) = Event::from_dirty(dirty) {
            self.events.push(ev);
        }
        Ok(())
    }

    /// [`Self::stroke_to`] for several pointer samples at once — all
    /// samples of one animation frame (`docs/specs/performance-input-path.md`).
    /// Paints exactly what calling `strokeTo` once per point would, but
    /// emits one `dirty-rect` covering the whole batch.
    ///
    /// - `xy`: the sprite-space points as `x0, y0, x1, y1, …`.
    /// - `pressure`: empty for full pressure at every point, otherwise one
    ///   value per point (as `strokeTo`'s `pressure`).
    ///
    /// Errors for an odd `xy` length, a `pressure` length that is neither
    /// zero nor the point count, or no stroke in progress; points before a
    /// failing one stay painted.
    #[wasm_bindgen(js_name = strokeToBatch)]
    pub fn stroke_to_batch(&mut self, xy: &[i32], pressure: &[f32]) -> Result<(), String> {
        if !xy.len().is_multiple_of(2) {
            return Err(format!("odd coordinate count: {}", xy.len()));
        }
        let count = xy.len() / 2;
        if !pressure.is_empty() && pressure.len() != count {
            return Err(format!(
                "{} pressure values for {count} points",
                pressure.len()
            ));
        }
        let mut dirty = DirtyRegion::None;
        let mut result = Ok(());
        for (i, &[x, y]) in xy.as_chunks::<2>().0.iter().enumerate() {
            match self.stroke_point(x, y, pressure.get(i).copied().unwrap_or(1.0)) {
                Ok(d) => dirty = union_dirty(dirty, d),
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        if let Some(ev) = Event::from_dirty(dirty) {
            self.events.push(ev);
        }
        result
    }
}

/// The region covering both `a` and `b`: their bounding box when both are
/// rects on the same cel, the whole canvas when they are on different cels.
fn union_dirty(a: DirtyRegion, b: DirtyRegion) -> DirtyRegion {
    match (a, b) {
        (DirtyRegion::None, r) | (r, DirtyRegion::None) => r,
        (
            DirtyRegion::Layer {
                layer,
                frame,
                rect: ra,
            },
            DirtyRegion::Layer {
                layer: lb,
                frame: fb,
                rect: rb,
            },
        ) if layer == lb && frame == fb => {
            let x0 = ra.x.min(rb.x);
            let y0 = ra.y.min(rb.y);
            let x1 =
                (i64::from(ra.x) + i64::from(ra.width)).max(i64::from(rb.x) + i64::from(rb.width));
            let y1 = (i64::from(ra.y) + i64::from(ra.height))
                .max(i64::from(rb.y) + i64::from(rb.height));
            let w = (x1 - i64::from(x0)).clamp(0, i64::from(u32::MAX)) as u32;
            let h = (y1 - i64::from(y0)).clamp(0, i64::from(u32::MAX)) as u32;
            DirtyRegion::layer_rect(layer, frame, Rect::new(x0, y0, w, h))
        }
        _ => DirtyRegion::Canvas,
    }
}

impl Document {
    /// One point of the gesture in progress (see [`Self::stroke_to`]);
    /// returns what it changed instead of emitting the event.
    fn stroke_point(&mut self, x: i32, y: i32, pressure: f32) -> Result<DirtyRegion, String> {
        let Some(live) = self.stroke.clone() else {
            return Err("no stroke in progress: call beginStroke first".to_string());
        };
        let to = Point::new(x, y);
        let extended = live.started
            && self
                .bus
                .continue_stroke(to, pressure, &mut self.cels)
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
                pressure,
            )
            .with_ink(live.ink.clone())
            .with_wrap(live.wrap);
            let cmd = match live.mirror {
                Some(m) => cmd.with_mirror(m),
                None => cmd,
            };
            self.bus
                .execute(cmd.into(), &mut self.sprite, &mut self.cels)
                .map_err(|e| format!("failed to paint: {e}"))?;
            self.stroke = Some(LiveStroke {
                started: true,
                ..live
            });
        }
        Ok(self.bus.last_dirty_region())
    }
}

impl Document {
    /// The armed gesture, if it has not painted yet.
    fn unstarted_stroke(&mut self) -> Result<&mut LiveStroke, String> {
        match self.stroke.as_mut() {
            None => Err("no stroke in progress: call beginStroke first".to_string()),
            Some(live) if live.started => {
                Err("the stroke has already started painting".to_string())
            }
            Some(live) => Ok(live),
        }
    }

    /// Shared tail of [`Self::begin_stroke`] and
    /// [`Self::begin_shading_stroke`]: resolve the target and arm the
    /// gesture.
    fn start_stroke(
        &mut self,
        color: Rgba,
        size: u32,
        shape: &str,
        pixel_perfect: bool,
        ink: Ink,
    ) -> Result<(), String> {
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
            ink,
            mirror: None,
            wrap: Wrap::default(),
            started: false,
        });
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
            doc.stroke_to(x, y, None).expect("stroke");
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
            doc.stroke_to(0, y, None).expect("fill");
            doc.stroke_to(8, y, None).expect("fill");
        }
        doc.end_stroke();
        assert!(painted(&doc, 8, 8, BLUE).len() == 64, "seeded opaque");
        doc.begin_stroke("eraser", RED, 2, "round", false)
            .expect("begin");
        for &(x, y) in &pts {
            doc.stroke_to(x, y, None).expect("erase");
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
        assert!(doc.stroke_to(1, 1, None).is_err(), "no stroke started");
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

    // --- Pen pressure (docs/specs/pixel-pressure.md) ---

    /// A pencil gesture with a pen pressure per point.
    fn pen_gesture(doc: &mut Document, size: u32, pts: &[(i32, i32, Option<f32>)]) {
        doc.begin_stroke("pencil", RED, size, "square", false)
            .expect("begin");
        for &(x, y, p) in pts {
            doc.stroke_to(x, y, p).expect("stroke");
        }
        doc.end_stroke();
    }

    /// Every painted (non-transparent) pixel of a `w`×`h` document.
    fn covered(doc: &Document, w: i32, h: i32) -> Vec<(i32, i32)> {
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| px(doc, x, y) != 0)
            .collect()
    }

    #[test]
    fn stroke_to_pressure_scales_the_stamp() {
        // Size 5 at pressure 0.5: round(2.5) = 3, a 3×3 block.
        let mut doc = Document::new(8, 8).expect("dims");
        pen_gesture(&mut doc, 5, &[(3, 3, Some(0.5))]);
        let want: Vec<_> = (2..=4).flat_map(|y| (2..=4).map(move |x| (x, y))).collect();
        assert_eq!(painted(&doc, 8, 8, RED), want);
        assert_eq!(covered(&doc, 8, 8), want, "only the foreground color");
    }

    #[test]
    fn stroke_to_without_pressure_paints_full_size() {
        let pts = [(1, 1), (6, 2), (2, 6)];
        let mut absent = Document::new(8, 8).expect("dims");
        pen_gesture(&mut absent, 3, &pts.map(|(x, y)| (x, y, None)));
        let mut full = Document::new(8, 8).expect("dims");
        pen_gesture(&mut full, 3, &pts.map(|(x, y)| (x, y, Some(1.0))));
        assert_eq!(covered(&absent, 8, 8), covered(&full, 8, 8));
        assert!(covered(&absent, 8, 8).len() > 20, "size-3 stamps");
    }

    #[test]
    fn stroke_to_light_pressure_covers_fewer_pixels() {
        let mut light = Document::new(16, 16).expect("dims");
        pen_gesture(&mut light, 8, &[(2, 8, Some(0.2)), (13, 8, Some(0.2))]);
        let mut firm = Document::new(16, 16).expect("dims");
        pen_gesture(&mut firm, 8, &[(2, 8, Some(1.0)), (13, 8, Some(1.0))]);
        assert!(covered(&light, 16, 16).len() < covered(&firm, 16, 16).len());
    }

    #[test]
    fn stroke_to_clamps_pressure_outside_unit_range() {
        // Below 0 paints a size-1 stamp, above 1 the full size.
        let mut low = Document::new(8, 8).expect("dims");
        pen_gesture(&mut low, 6, &[(3, 3, Some(-2.0))]);
        assert_eq!(painted(&low, 8, 8, RED), vec![(3, 3)]);
        let mut high = Document::new(8, 8).expect("dims");
        pen_gesture(&mut high, 6, &[(3, 3, Some(4.0))]);
        let mut full = Document::new(8, 8).expect("dims");
        pen_gesture(&mut full, 6, &[(3, 3, None)]);
        assert_eq!(covered(&high, 8, 8), covered(&full, 8, 8));
        assert_eq!(covered(&full, 8, 8).len(), 36);
    }

    #[test]
    fn pressure_stroke_is_one_undo_entry_and_redo_reproduces_it() {
        let mut doc = Document::new(12, 12).expect("dims");
        pen_gesture(
            &mut doc,
            8,
            &[(2, 2, Some(0.0)), (9, 3, Some(1.0)), (3, 9, Some(0.4))],
        );
        assert_eq!(doc.undo_depth(), 1, "one gesture, one entry");
        let after = covered(&doc, 12, 12);
        assert!(doc.undo());
        assert!(covered(&doc, 12, 12).is_empty(), "whole stroke undone");
        assert!(doc.redo().expect("redo"));
        assert_eq!(covered(&doc, 12, 12), after);
    }

    // Shading ink (docs/specs/shading-ink.md).

    const R0: u32 = 0x0a00_00ff;
    const R1: u32 = 0x1400_00ff;
    const R2: u32 = 0x1e00_00ff;

    /// A 6×1 document painted R0 R1 R2 R0 R1 R2 with the pencil.
    fn ramp_doc() -> Document {
        let mut doc = Document::new(6, 1).expect("dims");
        for (x, c) in [R0, R1, R2, R0, R1, R2].into_iter().enumerate() {
            doc.begin_stroke("pencil", c, 1, "square", false)
                .expect("begin");
            doc.stroke_to(x as i32, 0, None).expect("paint");
            doc.end_stroke();
        }
        doc
    }

    fn shading_gesture(doc: &mut Document, step: i32, pts: &[(i32, i32)]) -> Result<(), String> {
        doc.begin_shading_stroke(&[R0, R1, R2], step, 1, "square", false)?;
        for &(x, y) in pts {
            doc.stroke_to(x, y, None)?;
        }
        doc.end_stroke();
        Ok(())
    }

    fn row(doc: &Document) -> Vec<u32> {
        (0..6).map(|x| px(doc, x, 0)).collect()
    }

    #[test]
    fn shading_stroke_steps_each_pixel_once_as_one_undo_entry() {
        let mut doc = ramp_doc();
        let depth = doc.undo_depth();
        shading_gesture(&mut doc, 1, &[(0, 0), (5, 0), (0, 0)]).expect("shade");
        assert_eq!(row(&doc), [R1, R2, R2, R1, R2, R2]);
        assert_eq!(doc.undo_depth(), depth + 1);
        assert!(doc.undo());
        assert_eq!(row(&doc), [R0, R1, R2, R0, R1, R2]);
    }

    #[test]
    fn shading_stroke_backwards_steps_toward_the_start() {
        let mut doc = ramp_doc();
        shading_gesture(&mut doc, -1, &[(0, 0), (5, 0)]).expect("shade");
        assert_eq!(row(&doc), [R0, R0, R1, R0, R0, R1]);
    }

    #[test]
    fn shading_stroke_on_locked_layer_errs_without_undo_entry() {
        let mut doc = ramp_doc();
        doc.set_layer_editable(0, false).expect("lock");
        let depth = doc.undo_depth();
        let err = shading_gesture(&mut doc, 1, &[(0, 0)]).expect_err("locked");
        assert!(err.contains("locked"), "{err}");
        assert_eq!(doc.undo_depth(), depth);
        assert_eq!(row(&doc), [R0, R1, R2, R0, R1, R2]);
    }

    #[test]
    fn shading_stroke_without_image_layer_errs() {
        let mut doc = Document::new(2, 2).expect("dims");
        doc.sprite.layers.clear();
        doc.sprite
            .layers
            .push(Layer::group(LayerId::new(7), "folder"));
        let err = shading_gesture(&mut doc, 1, &[(0, 0)]).expect_err("nothing to paint");
        assert!(err.contains("no paintable image layer"));
        assert_eq!(doc.undo_depth(), 0);
    }

    // Symmetry and tiled mode (docs/specs/symmetry-tiled-mode.md).

    #[test]
    fn set_stroke_mirror_paints_mirror_images() {
        let mut doc = Document::new(8, 6).expect("dims");
        doc.begin_stroke("pencil", RED, 1, "square", false)
            .expect("begin");
        doc.set_stroke_mirror(Some(8), Some(6)).expect("mirror");
        doc.stroke_to(1, 0, None).expect("stroke");
        doc.end_stroke();
        assert_eq!(
            painted(&doc, 8, 6, RED),
            vec![(1, 0), (6, 0), (1, 5), (6, 5)]
        );
        assert_eq!(doc.undo_depth(), 1);
        assert!(doc.undo());
        assert!(painted(&doc, 8, 6, RED).is_empty(), "one undo, all four");
    }

    #[test]
    fn set_stroke_wrap_continues_across_the_seam() {
        let mut doc = Document::new(8, 4).expect("dims");
        doc.begin_stroke("pencil", RED, 1, "square", false)
            .expect("begin");
        doc.set_stroke_wrap(true, false).expect("wrap");
        doc.stroke_to(6, 1, None).expect("stroke");
        doc.stroke_to(9, 1, None).expect("stroke");
        doc.end_stroke();
        assert_eq!(
            painted(&doc, 8, 4, RED),
            vec![(0, 1), (1, 1), (6, 1), (7, 1)]
        );
    }

    #[test]
    fn stroke_aids_need_an_unstarted_gesture() {
        let mut doc = Document::new(4, 4).expect("dims");
        assert!(doc.set_stroke_mirror(Some(4), None).is_err(), "no gesture");
        assert!(doc.set_stroke_wrap(true, true).is_err(), "no gesture");
        doc.begin_stroke("pencil", RED, 1, "square", false)
            .expect("begin");
        doc.stroke_to(0, 0, None).expect("stroke");
        assert!(doc.set_stroke_mirror(Some(4), None).is_err(), "started");
        assert!(doc.set_stroke_wrap(true, false).is_err(), "started");
        doc.end_stroke();
        // A plain stroke afterwards neither mirrors nor wraps.
        gesture(&mut doc, "pencil", 1, "square", false, &[(3, 3)]);
        assert_eq!(painted(&doc, 4, 4, RED), vec![(0, 0), (3, 3)]);
    }

    #[test]
    fn begin_shading_stroke_rejects_bad_step_and_empty_ramp() {
        let mut doc = Document::new(2, 2).expect("dims");
        assert!(
            doc.begin_shading_stroke(&[R0], 2, 1, "square", false)
                .is_err()
        );
        assert!(
            doc.begin_shading_stroke(&[R0], 0, 1, "square", false)
                .is_err()
        );
        assert!(
            doc.begin_shading_stroke(&[], 1, 1, "square", false)
                .is_err()
        );
        assert!(
            doc.begin_shading_stroke(&[R0], 1, 1, "oval", false)
                .is_err()
        );
    }

    /// Every pixel of a `w`×`h` document, row by row.
    fn all_pixels(doc: &Document, w: i32, h: i32) -> Vec<u32> {
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| px(doc, x, y))
            .collect()
    }

    /// A deterministic wandering drag: `n` points with jumps of up to
    /// ±`step` pixels, plus a pressure per point.
    fn wander(n: usize, step: i32, seed: u32) -> (Vec<i32>, Vec<f32>) {
        let mut state = seed;
        let mut next = move || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state >> 8
        };
        let (mut x, mut y) = (16, 16);
        let mut xy = Vec::with_capacity(n * 2);
        let mut pressure = Vec::with_capacity(n);
        for _ in 0..n {
            x = (x + (next() % (2 * step as u32 + 1)) as i32 - step).clamp(-4, 36);
            y = (y + (next() % (2 * step as u32 + 1)) as i32 - step).clamp(-4, 36);
            xy.extend([x, y]);
            pressure.push((next() % 100) as f32 / 99.0);
        }
        (xy, pressure)
    }

    #[test]
    fn stroke_to_batch_matches_per_sample_stroke_to() {
        // (tool, size, shape, pixel-perfect, with pressure)
        let cases = [
            ("pencil", 1, "square", false, false),
            ("pencil", 1, "square", true, false),
            ("pencil", 3, "round", false, true),
            ("pencil", 8, "square", false, true),
            ("eraser", 2, "round", false, false),
        ];
        for (seed, &(tool, size, shape, pp, with_pressure)) in cases.iter().enumerate() {
            let (xy, pressure) = wander(60, 5, seed as u32 + 7);
            let pressure: &[f32] = if with_pressure { &pressure } else { &[] };
            let make = || {
                let mut doc = Document::new(32, 32).expect("dims");
                if tool == "eraser" {
                    gesture(
                        &mut doc,
                        "pencil",
                        8,
                        "square",
                        false,
                        &[(0, 0), (31, 31), (0, 31)],
                    );
                }
                doc
            };
            let mut one_by_one = make();
            one_by_one
                .begin_stroke(tool, BLUE, size, shape, pp)
                .expect("begin");
            for (i, &[x, y]) in xy.as_chunks::<2>().0.iter().enumerate() {
                one_by_one
                    .stroke_to(x, y, pressure.get(i).copied())
                    .expect("stroke");
            }
            one_by_one.end_stroke();
            // Batched as a frame loop would: uneven batches of 1–9 points.
            let mut batched = make();
            batched
                .begin_stroke(tool, BLUE, size, shape, pp)
                .expect("begin");
            let mut start = 0;
            let mut len = 1;
            while start < xy.len() / 2 {
                let end = (start + len).min(xy.len() / 2);
                let pr = if pressure.is_empty() {
                    &[][..]
                } else {
                    &pressure[start..end]
                };
                batched
                    .stroke_to_batch(&xy[start * 2..end * 2], pr)
                    .expect("batch");
                start = end;
                len = len % 9 + 1;
            }
            batched.end_stroke();
            assert_eq!(
                all_pixels(&batched, 32, 32),
                all_pixels(&one_by_one, 32, 32),
                "{tool} size {size} {shape} pp={pp} pressure={with_pressure}"
            );
            assert_eq!(
                batched.undo_depth(),
                one_by_one.undo_depth(),
                "one undo entry"
            );
        }
    }

    #[test]
    fn stroke_to_batch_passes_through_every_sample() {
        let mut doc = Document::new(32, 32).expect("dims");
        doc.drain_events();
        let samples = [(2, 2), (9, 4), (14, 13), (5, 20), (25, 28), (30, 3)];
        let xy: Vec<i32> = samples.iter().flat_map(|&(x, y)| [x, y]).collect();
        doc.begin_stroke("pencil", RED, 1, "square", false)
            .expect("begin");
        doc.stroke_to_batch(&xy, &[]).expect("batch");
        doc.end_stroke();
        for (x, y) in samples {
            assert_eq!(px(&doc, x, y), RED, "sample ({x}, {y}) painted");
        }
        // One dirty-rect for the whole batch, covering every sample.
        let events = doc.drain_events();
        assert_eq!(events.len(), 1, "one event per batch");
        let ev = &events[0];
        assert_eq!(ev.kind(), "dirty-rect");
        assert_eq!((ev.x(), ev.y()), (2, 2));
        assert_eq!((ev.width(), ev.height()), (29, 27));
    }

    #[test]
    fn stroke_to_batch_rejects_bad_lengths_and_missing_stroke() {
        let mut doc = Document::new(4, 4).expect("dims");
        assert!(doc.stroke_to_batch(&[1, 1], &[]).is_err(), "no stroke");
        doc.begin_stroke("pencil", RED, 1, "square", false)
            .expect("begin");
        assert!(doc.stroke_to_batch(&[1, 1, 2], &[]).is_err(), "odd xy");
        assert!(
            doc.stroke_to_batch(&[1, 1, 2, 2], &[1.0]).is_err(),
            "pressure count"
        );
        assert!(
            painted(&doc, 4, 4, RED).is_empty(),
            "rejected batches paint nothing"
        );
        doc.stroke_to_batch(&[], &[])
            .expect("empty batch is a no-op");
        assert!(doc.drain_events().is_empty());
    }
}
