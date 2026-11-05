//! Effects / adjustments surface — the Fineliner filter set applied to the
//! active cel (spec §13.4; Decision Log 2026-09-06).
//!
//! JS names an effect and passes a flat parameter vector (see
//! [`pincel_effects::EffectSpec`] for the per-effect order). The effect
//! runs over the active layer's cel at the current frame; when a marquee
//! selection is active only the pixels inside it change. `applyEffect`
//! commits the result as one undoable
//! [`ReplaceCelPixels`](pincel_core::ReplaceCelPixels); `previewEffect`
//! composites the would-be result without touching document state or the
//! undo stack, for a live dialog preview.

use pincel_core::{
    CelData, ColorMode, ComposeRequest, FrameIndex, LayerId, Rect, ReplaceCelPixels, compose,
};
use pincel_effects::{EFFECT_NAMES, EffectImage, EffectSpec};
use wasm_bindgen::prelude::*;

use crate::events::Event;
use crate::{ComposeFrame, Document};

/// Wire names of every effect `applyEffect` / `previewEffect` accept, in
/// menu order. Mirrors [`pincel_effects::EFFECT_NAMES`].
#[wasm_bindgen(js_name = effectNames)]
pub fn effect_names() -> Vec<String> {
    EFFECT_NAMES.iter().map(|s| (*s).to_owned()).collect()
}

/// The computed outcome of one effect run, before it is committed or
/// previewed.
struct EffectRun {
    layer: LayerId,
    frame: FrameIndex,
    /// Full replacement buffer for the cel (original pixels kept outside
    /// the selection).
    pixels: Vec<u8>,
    /// Sprite-space rect that actually changed; `None` when the selection
    /// misses the cel entirely (nothing to do).
    dirty: Option<Rect>,
}

impl Document {
    /// Run `name` with `params` over the active cel and return the
    /// replacement buffer plus the changed rect.
    fn run_effect(&self, name: &str, params: &[f64]) -> Result<EffectRun, String> {
        let spec = EffectSpec::from_params(name, params).map_err(|e| e.to_string())?;
        let layer = self.edit_target_layer()?;
        let frame = self.current_frame;
        let cel = self
            .cels
            .get(layer, frame)
            .ok_or_else(|| format!("no cel on layer {} at frame {}", layer.0, frame.0))?;
        let CelData::Image(buffer) = &cel.data else {
            return Err("the active cel is not an image cel".to_owned());
        };
        if buffer.color_mode != ColorMode::Rgba {
            return Err("effects require an RGBA document".to_owned());
        }
        let cel_rect = Rect::new(cel.position.0, cel.position.1, buffer.width, buffer.height);

        let src = EffectImage::from_rgba8(buffer.width, buffer.height, buffer.data.clone())
            .map_err(|e| e.to_string())?;
        // The target is the selection when one exists (Auto-Level reads
        // its luma range from it); cel pixel (x, y) sits at the cel
        // position plus (x, y) in sprite space.
        let sprite = &self.sprite;
        let whole = !sprite.has_selection();
        let (cx, cy) = cel.position;
        let out = spec
            .run_at(&src, (cx, cy), |x, y| {
                whole || sprite.selection_contains(cx + x as i32, cy + y as i32)
            })
            .into_raw();

        let Some(sel) = self.sprite.selection.filter(|r| !r.is_empty()) else {
            return Ok(EffectRun {
                layer,
                frame,
                pixels: out,
                dirty: Some(cel_rect),
            });
        };
        let inter = sel.intersect(cel_rect);
        if inter.is_empty() {
            return Ok(EffectRun {
                layer,
                frame,
                pixels: out,
                dirty: None,
            });
        }
        // Keep the original outside the selection: copy the affected
        // pixels from the effect output into a clone of the source — whole
        // row spans for a rect selection, per covered pixel for a shaped one.
        let mut merged = buffer.data.clone();
        let stride = buffer.width as usize * 4;
        let x0 = (inter.x - cel_rect.x) as usize * 4;
        let span = inter.width as usize * 4;
        let mask = self.sprite.selection_mask.as_ref();
        for row in 0..inter.height as usize {
            let y = (inter.y - cel_rect.y) as usize + row;
            let start = y * stride + x0;
            match mask {
                None => merged[start..start + span].copy_from_slice(&out[start..start + span]),
                Some(m) => {
                    for col in 0..inter.width as usize {
                        let sx = inter.x + col as i32;
                        let sy = inter.y + row as i32;
                        if m.contains(sx, sy) {
                            let i = start + col * 4;
                            merged[i..i + 4].copy_from_slice(&out[i..i + 4]);
                        }
                    }
                }
            }
        }
        Ok(EffectRun {
            layer,
            frame,
            pixels: merged,
            dirty: Some(inter),
        })
    }
}

#[wasm_bindgen]
impl Document {
    /// Apply the named effect to the active layer's cel at the current
    /// frame, restricted to the marquee selection when one is active.
    ///
    /// `name` is one of [`effect_names`]; `params` follows the per-effect
    /// order documented on [`pincel_effects::EffectSpec`]. Commits one
    /// undo entry (never merged with neighbouring strokes) and emits a
    /// `dirty-rect` event for the changed area. Returns `Ok(false)` when
    /// the selection does not overlap the cel and nothing changed.
    ///
    /// Errors on an unknown effect / wrong parameter count, when the
    /// document has no paintable image layer, or when the active cel is
    /// not RGBA.
    #[wasm_bindgen(js_name = applyEffect)]
    pub fn apply_effect(&mut self, name: &str, params: &[f64]) -> Result<bool, String> {
        let run = self.run_effect(name, params)?;
        let Some(dirty) = run.dirty else {
            return Ok(false);
        };
        self.bus.seal();
        let cmd = ReplaceCelPixels::new(run.layer, run.frame, run.pixels).with_dirty_hint(dirty);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to apply effect: {e}"))?;
        self.bus.seal();
        if let Some(ev) = Event::from_dirty(self.bus.last_dirty_region()) {
            self.events.push(ev);
        }
        Ok(true)
    }

    /// Composite the current frame at `zoom` as it would look after
    /// [`Self::apply_effect`] with the same arguments, without changing
    /// the document or the undo history. Same output contract as
    /// [`Document::compose`].
    #[wasm_bindgen(js_name = previewEffect)]
    pub fn preview_effect(
        &self,
        name: &str,
        params: &[f64],
        zoom: u32,
    ) -> Result<ComposeFrame, String> {
        let run = self.run_effect(name, params)?;
        let mut cels = self.cels.clone();
        if run.dirty.is_some()
            && let Some(cel) = cels.get_mut(run.layer, run.frame)
            && let CelData::Image(buffer) = &mut cel.data
        {
            buffer.data = run.pixels;
        }
        let mut request =
            ComposeRequest::full(self.current_frame, self.sprite.width, self.sprite.height);
        request.zoom = zoom;
        let mut pixels = Vec::new();
        let result = compose(&self.sprite, &cels, &request, &mut pixels)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(doc: &Document, x: i32, y: i32) -> u32 {
        doc.pick_color(0, x, y).expect("pick")
    }

    #[test]
    fn effect_names_matches_the_effects_crate() {
        assert_eq!(effect_names().len(), EFFECT_NAMES.len());
        assert_eq!(effect_names()[0], "gaussian_blur");
    }

    #[test]
    fn apply_effect_invert_changes_pixels_and_undo_restores() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_tool("pencil", 1, 1, 0xFF0000FF).expect("paint");
        doc.end_stroke();
        assert!(doc.apply_effect("invert", &[]).expect("apply"));
        assert_eq!(pixel(&doc, 1, 1), 0x00FFFFFF);
        // Transparent pixels stay transparent (alpha untouched).
        assert_eq!(pixel(&doc, 0, 0) & 0xFF, 0);
        assert!(doc.undo());
        assert_eq!(pixel(&doc, 1, 1), 0xFF0000FF);
        assert!(doc.redo().expect("redo"));
        assert_eq!(pixel(&doc, 1, 1), 0x00FFFFFF);
    }

    #[test]
    fn apply_effect_is_its_own_undo_entry_after_a_stroke() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_tool("pencil", 0, 0, 0x102030FF).expect("paint");
        let depth_after_stroke = doc.undo_depth();
        doc.apply_effect("invert", &[]).expect("apply");
        assert_eq!(doc.undo_depth(), depth_after_stroke + 1);
        // A following stroke must not merge into the effect entry.
        doc.apply_tool("pencil", 1, 0, 0x102030FF).expect("paint");
        assert_eq!(doc.undo_depth(), depth_after_stroke + 2);
    }

    #[test]
    fn apply_effect_respects_the_marquee_selection() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_tool("pencil", 0, 0, 0xFF0000FF).expect("paint");
        doc.apply_tool("pencil", 3, 3, 0xFF0000FF).expect("paint");
        doc.end_stroke();
        doc.set_selection(2, 2, 2, 2);
        doc.drain_events();
        assert!(doc.apply_effect("invert", &[]).expect("apply"));
        assert_eq!(pixel(&doc, 0, 0), 0xFF0000FF, "outside selection untouched");
        assert_eq!(pixel(&doc, 3, 3), 0x00FFFFFF, "inside selection inverted");
        let ev = doc.drain_events();
        let dirty = ev
            .iter()
            .find(|e| e.kind() == "dirty-rect")
            .expect("dirty-rect event");
        assert_eq!(
            (dirty.x(), dirty.y(), dirty.width(), dirty.height()),
            (2, 2, 2, 2)
        );
    }

    #[test]
    fn apply_effect_with_selection_off_canvas_is_a_noop() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.set_selection(10, 10, 2, 2);
        let depth = doc.undo_depth();
        assert!(!doc.apply_effect("invert", &[]).expect("apply"));
        assert_eq!(doc.undo_depth(), depth);
    }

    /// effect-catalog-v2 AC-08: Pixel mode's menus hide the neighbourhood
    /// effects, but its core still knows every effect name.
    #[test]
    fn apply_effect_knows_every_effect_name_on_a_pixel_document() {
        let mut doc = Document::new(2, 2).expect("dims");
        for name in EFFECT_NAMES {
            if let Err(err) = doc.apply_effect(name, &[]) {
                assert!(
                    err.contains("expects") && !err.contains("unknown effect"),
                    "{name}: {err}"
                );
            }
        }
    }

    #[test]
    fn apply_effect_unknown_name_or_bad_arity_errors() {
        let mut doc = Document::new(2, 2).expect("dims");
        assert!(doc.apply_effect("no_such_effect", &[]).is_err());
        assert!(doc.apply_effect("gaussian_blur", &[]).is_err());
        assert_eq!(doc.undo_depth(), 0);
    }

    #[test]
    fn preview_effect_leaves_document_and_history_untouched() {
        let mut doc = Document::new(2, 2).expect("dims");
        doc.apply_tool("pencil", 0, 0, 0xFF0000FF).expect("paint");
        doc.end_stroke();
        let depth = doc.undo_depth();
        let frame = doc.preview_effect("invert", &[], 1).expect("preview");
        assert_eq!((frame.width(), frame.height()), (2, 2));
        let px = frame.pixels();
        assert_eq!(
            &px[0..4],
            &[0, 255, 255, 255],
            "preview shows the inverted pixel"
        );
        assert_eq!(pixel(&doc, 0, 0), 0xFF0000FF, "document unchanged");
        assert_eq!(doc.undo_depth(), depth);
    }

    /// adjustments-paintnet: the six new adjustments with settings that
    /// change something.
    const PAINTNET: [(&str, &[f64]); 6] = [
        ("auto_level", &[]),
        ("exposure", &[1.0]),
        ("highlights_shadows", &[60.0, 60.0]),
        ("invert_alpha", &[]),
        ("sepia", &[100.0]),
        ("temperature_tint", &[80.0, 40.0]),
    ];

    fn cel_pixels(doc: &Document, layer: u32, frame: u32) -> Vec<u8> {
        let cel = doc
            .cels
            .get(LayerId::new(layer), FrameIndex::new(frame))
            .expect("cel");
        let CelData::Image(buf) = &cel.data else {
            panic!("image cel");
        };
        buf.data.clone()
    }

    /// Layer 0 painted on frames 0 and 1, a second layer on frame 0, and a
    /// 2x2 selection over the bottom-right corner; layer 0 / frame 0 is
    /// the target.
    fn layered() -> (Document, u32) {
        let mut doc = Document::new(4, 4).expect("dims");
        for (x, y, c) in [(0, 0, 0x5A6B7CFF), (2, 2, 0x303030FF), (3, 3, 0xA0A0A0FF)] {
            doc.apply_tool("pencil", x, y, c).expect("paint");
            doc.end_stroke();
        }
        doc.apply_tool("pencil", 3, 2, 0x8040C080).expect("paint");
        doc.end_stroke();
        let frame = doc.add_frame(100).expect("frame");
        doc.set_current_frame(frame).expect("frame");
        doc.apply_tool("pencil", 2, 2, 0x406080FF).expect("paint");
        doc.end_stroke();
        doc.set_current_frame(0).expect("frame");
        let top = doc.add_layer("top").expect("layer");
        doc.set_active_layer(top);
        doc.apply_tool("pencil", 3, 3, 0x10F010FF).expect("paint");
        doc.end_stroke();
        doc.set_active_layer(0);
        doc.set_selection(2, 2, 2, 2);
        (doc, top)
    }

    /// adjustments-paintnet AC-10: only the target changes, in one undo step.
    #[test]
    fn apply_effect_paintnet_adjustments_change_only_the_target_in_one_undo_step() {
        for (name, params) in PAINTNET {
            let (mut doc, top) = layered();
            let target = cel_pixels(&doc, 0, 0);
            let other_frame = cel_pixels(&doc, 0, 1);
            let other_layer = cel_pixels(&doc, top, 0);
            let depth = doc.undo_depth();
            assert!(doc.apply_effect(name, params).expect(name), "{name}");
            assert_eq!(doc.undo_depth(), depth + 1, "{name}: one undo step");
            let after = cel_pixels(&doc, 0, 0);
            for y in 0..4 {
                for x in 0..4 {
                    let i = (y * 4 + x) * 4;
                    if x < 2 || y < 2 {
                        assert_eq!(after[i..i + 4], target[i..i + 4], "{name}: ({x}, {y})");
                    }
                }
            }
            assert_ne!(after, target, "{name} changed nothing in the selection");
            assert_eq!(cel_pixels(&doc, 0, 1), other_frame, "{name}: other cel");
            assert_eq!(cel_pixels(&doc, top, 0), other_layer, "{name}: other layer");
            assert!(doc.undo());
            assert_eq!(cel_pixels(&doc, 0, 0), target, "{name}: undo restores");
        }
    }

    /// adjustments-paintnet AC-05 / AC-10: Auto-Level reads its range from
    /// the selection, not from the whole cel.
    #[test]
    fn apply_effect_auto_level_takes_its_range_from_the_selection() {
        let mut doc = Document::new(4, 1).expect("dims");
        for (x, c) in [
            (0, 0x000000FF),
            (1, 0x505050FF),
            (2, 0xA0A0A0FF),
            (3, 0xFFFFFFFF),
        ] {
            doc.apply_tool("pencil", x, 0, c).expect("paint");
            doc.end_stroke();
        }
        doc.set_selection(1, 0, 2, 1);
        assert!(doc.apply_effect("auto_level", &[]).expect("apply"));
        assert_eq!(pixel(&doc, 1, 0), 0x000000FF);
        assert_eq!(pixel(&doc, 2, 0), 0xFFFFFFFF);
    }

    /// adjustments-paintnet AC-11: a wrong parameter count is refused and
    /// changes nothing.
    #[test]
    fn apply_effect_paintnet_wrong_parameter_count_changes_nothing() {
        for (name, params) in PAINTNET {
            let (mut doc, _) = layered();
            let before = cel_pixels(&doc, 0, 0);
            let depth = doc.undo_depth();
            let wrong = vec![0.0; params.len() + 1];
            let err = doc.apply_effect(name, &wrong).expect_err(name);
            assert!(err.contains("expects"), "{name}: {err}");
            assert_eq!(doc.undo_depth(), depth);
            assert_eq!(cel_pixels(&doc, 0, 0), before);
        }
    }

    /// adjustments-paintnet AC-13: an indexed document refuses the new
    /// adjustments with the message every other effect gives.
    #[test]
    fn apply_effect_paintnet_on_an_indexed_document_is_refused() {
        let mut doc = Document::new(2, 1).expect("doc");
        doc.sprite.color_mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        let mut buf = pincel_core::PixelBuffer::empty(
            2,
            1,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        );
        buf.data = vec![1, 0];
        doc.cels.insert(pincel_core::Cel::image(
            crate::DEFAULT_LAYER_ID,
            FrameIndex::new(0),
            buf,
        ));
        let before = cel_pixels(&doc, 0, 0);
        let expected = doc.apply_effect("invert", &[]).expect_err("invert");
        for (name, params) in PAINTNET {
            assert_eq!(
                doc.apply_effect(name, params),
                Err(expected.clone()),
                "{name}"
            );
            assert!(
                doc.preview_effect(name, params, 1).is_err(),
                "{name} preview"
            );
        }
        assert_eq!(doc.undo_depth(), 0);
        assert_eq!(cel_pixels(&doc, 0, 0), before);
    }

    /// effects-blurs: the new blurs and Motion Blur with Centered off, at
    /// settings that change something.
    const BLURS: [(&str, &[f64]); 7] = [
        ("bokeh", &[2.0, 50.0]),
        ("fragment", &[3.0, 1.0, 0.0]),
        ("median", &[1.0, 100.0]),
        ("sketch_blur", &[3.0, 5.0]),
        ("surface_blur", &[1.0, 100.0]),
        ("zoom_blur", &[100.0, 0.0, 0.0]),
        ("motion_blur", &[3.0, 0.0, 0.0]),
    ];

    /// effects-blurs: the new blurs run in the Pixel core (no menu entry,
    /// but the wire accepts them) as one undo step.
    #[test]
    fn apply_effect_blurs_run_on_a_pixel_document() {
        for (name, params) in BLURS {
            let (mut doc, _) = layered();
            let before = cel_pixels(&doc, 0, 0);
            let depth = doc.undo_depth();
            assert!(doc.apply_effect(name, params).expect(name), "{name}");
            assert_eq!(doc.undo_depth(), depth + 1, "{name}: one undo step");
            assert_ne!(cel_pixels(&doc, 0, 0), before, "{name} changed nothing");
        }
    }

    /// effects-blurs AC-14: a wrong parameter count is refused and changes
    /// nothing.
    #[test]
    fn apply_effect_blurs_wrong_parameter_count_changes_nothing() {
        for (name, params) in BLURS {
            let (mut doc, _) = layered();
            let before = cel_pixels(&doc, 0, 0);
            let depth = doc.undo_depth();
            // Motion Blur accepts two or three slots; four is wrong.
            let wrong = vec![1.0; params.len() + 1];
            let err = doc.apply_effect(name, &wrong).expect_err(name);
            assert!(err.contains("expects"), "{name}: {err}");
            assert!(
                doc.preview_effect(name, &wrong, 1).is_err(),
                "{name} preview"
            );
            assert_eq!(doc.undo_depth(), depth);
            assert_eq!(cel_pixels(&doc, 0, 0), before);
        }
    }

    /// effects-blurs AC-16: an indexed document refuses the blurs with the
    /// message every other effect gives.
    #[test]
    fn apply_effect_blurs_on_an_indexed_document_are_refused() {
        let mut doc = Document::new(2, 1).expect("doc");
        let mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        doc.sprite.color_mode = mode;
        let mut buf = pincel_core::PixelBuffer::empty(2, 1, mode);
        buf.data = vec![1, 0];
        doc.cels.insert(pincel_core::Cel::image(
            crate::DEFAULT_LAYER_ID,
            FrameIndex::new(0),
            buf,
        ));
        let before = cel_pixels(&doc, 0, 0);
        let expected = doc.apply_effect("invert", &[]).expect_err("invert");
        for (name, params) in BLURS
            .iter()
            .chain(&[("radial_blur", &[10.0, 0.5, 0.5, 1.0][..])])
        {
            assert_eq!(
                doc.apply_effect(name, params),
                Err(expected.clone()),
                "{name}"
            );
            assert!(
                doc.preview_effect(name, params, 1).is_err(),
                "{name} preview"
            );
        }
        assert_eq!(doc.undo_depth(), 0);
        assert_eq!(cel_pixels(&doc, 0, 0), before);
    }

    /// outline-effect: Outline Object, defaults (Outside, Square, 1 px)
    /// in a semi-transparent orange.
    const OUTLINE_COLOR: u32 = 0xFF80_10C0;
    const OUTLINE: [f64; 4] = [0.0, 1.0, 1.0, OUTLINE_COLOR as f64];

    /// outline-effect AC-03 / AC-06 / AC-09: a single pixel's 8 neighbours
    /// take the colour exactly, in one undo step that redo repeats.
    #[test]
    fn apply_effect_outline_colours_the_neighbours_in_one_undo_step() {
        let mut doc = Document::new(5, 5).expect("dims");
        doc.apply_tool("pencil", 2, 2, 0x102030FF).expect("paint");
        doc.end_stroke();
        let before = cel_pixels(&doc, 0, 0);
        let depth = doc.undo_depth();
        assert!(doc.apply_effect("outline", &OUTLINE).expect("apply"));
        assert_eq!(doc.undo_depth(), depth + 1);
        let after = cel_pixels(&doc, 0, 0);
        for y in 0..5 {
            for x in 0..5 {
                let want = if (x, y) == (2, 2) {
                    0x102030FF
                } else if (1..=3).contains(&x) && (1..=3).contains(&y) {
                    OUTLINE_COLOR
                } else {
                    0
                };
                assert_eq!(pixel(&doc, x, y), want, "({x}, {y})");
            }
        }
        assert!(doc.undo());
        assert_eq!(cel_pixels(&doc, 0, 0), before, "undo restores exactly");
        assert!(doc.redo().expect("redo"));
        assert_eq!(cel_pixels(&doc, 0, 0), after, "redo repeats the outline");
    }

    /// outline-effect AC-02: a trimmed cel's edge counts as transparent and
    /// the outline never grows the cel or writes outside it.
    #[test]
    fn apply_effect_outline_clips_at_a_trimmed_cel() {
        let mut doc = Document::new(8, 8).expect("dims");
        let mut buf = pincel_core::PixelBuffer::empty(2, 2, ColorMode::Rgba);
        buf.data = [[9, 9, 9, 255], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]].concat();
        let mut cel = pincel_core::Cel::image(crate::DEFAULT_LAYER_ID, FrameIndex::new(0), buf);
        cel.position = (3, 3);
        doc.cels.insert(cel);
        // Inside: the cel's corner pixel touches the cel's edge.
        assert!(
            doc.apply_effect("outline", &[1.0, 1.0, 1.0, OUTLINE[3]])
                .expect("inside")
        );
        assert_eq!(pixel(&doc, 3, 3), OUTLINE_COLOR);
        assert!(doc.undo());
        // Outside: only the cel's other three pixels; nothing beyond it.
        assert!(doc.apply_effect("outline", &OUTLINE).expect("outside"));
        let cel = doc
            .cels
            .get(crate::DEFAULT_LAYER_ID, FrameIndex::new(0))
            .expect("cel");
        let CelData::Image(buf) = &cel.data else {
            panic!("image cel");
        };
        assert_eq!((cel.position, buf.width, buf.height), ((3, 3), 2, 2));
        assert_eq!(pixel(&doc, 3, 3), 0x090909FF);
        for (x, y) in [(4, 3), (3, 4), (4, 4)] {
            assert_eq!(pixel(&doc, x, y), OUTLINE_COLOR, "({x}, {y})");
        }
        for (x, y) in [(2, 2), (2, 3), (3, 2), (5, 5)] {
            assert_eq!(pixel(&doc, x, y) & 0xFF, 0, "({x}, {y}) outside the cel");
        }
    }

    /// outline-effect AC-07: only the selection changes, and an opaque pixel
    /// outside it still outlines into it.
    #[test]
    fn apply_effect_outline_stays_in_the_selection() {
        let mut doc = Document::new(6, 3).expect("dims");
        doc.apply_tool("pencil", 2, 1, 0x102030FF).expect("paint");
        doc.end_stroke();
        doc.set_selection(3, 0, 3, 3);
        assert!(doc.apply_effect("outline", &OUTLINE).expect("apply"));
        for y in 0..3 {
            assert_eq!(
                pixel(&doc, 3, y),
                OUTLINE_COLOR,
                "(3, {y}) in the selection"
            );
            assert_eq!(
                pixel(&doc, 1, y) & 0xFF,
                0,
                "(1, {y}) outside the selection"
            );
        }
        assert_eq!(pixel(&doc, 2, 0) & 0xFF, 0);
        assert_eq!(pixel(&doc, 2, 1), 0x102030FF);
        assert_eq!(pixel(&doc, 4, 1) & 0xFF, 0, "beyond the thickness");
    }

    /// outline-effect AC-10 / AC-12: an indexed document, a wrong parameter
    /// count and a bad colour slot are refused and change nothing.
    #[test]
    fn apply_effect_outline_errors_change_nothing() {
        let (mut doc, _) = layered();
        let before = cel_pixels(&doc, 0, 0);
        let depth = doc.undo_depth();
        for wrong in [
            &OUTLINE[..3],
            &[0.0, 1.0, 1.0, 0.5][..],
            &[0.0, 1.0, 1.0, -1.0][..],
        ] {
            assert!(doc.apply_effect("outline", wrong).is_err(), "{wrong:?}");
            assert!(
                doc.preview_effect("outline", wrong, 1).is_err(),
                "{wrong:?}"
            );
        }
        assert_eq!(doc.undo_depth(), depth);
        assert_eq!(cel_pixels(&doc, 0, 0), before);

        let mut doc = Document::new(2, 1).expect("doc");
        let mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        doc.sprite.color_mode = mode;
        let mut buf = pincel_core::PixelBuffer::empty(2, 1, mode);
        buf.data = vec![1, 0];
        doc.cels.insert(pincel_core::Cel::image(
            crate::DEFAULT_LAYER_ID,
            FrameIndex::new(0),
            buf,
        ));
        let before = cel_pixels(&doc, 0, 0);
        let expected = doc.apply_effect("invert", &[]).expect_err("invert");
        assert_eq!(doc.apply_effect("outline", &OUTLINE), Err(expected));
        assert!(doc.preview_effect("outline", &OUTLINE, 1).is_err());
        assert_eq!(doc.undo_depth(), 0);
        assert_eq!(cel_pixels(&doc, 0, 0), before);
    }

    /// effects-distort: the distortions with their table defaults (Pixelate
    /// with Pixel mode's Centre).
    const DISTORTIONS: [(&str, &[f64]); 9] = [
        ("bulge", &[45.0, 0.5, 0.5, 0.0]),
        ("crystalize", &[8.0, 1.0]),
        ("dents", &[25.0, 50.0, 10.0, 1.0, 0.0]),
        ("frosted_glass", &[10.0, 1.0]),
        ("morphology", &[0.0, 0.0, 1.0]),
        ("pixelate", &[8.0, 1.0]),
        ("polar_inversion", &[1.0, 0.5, 0.5, 1.0]),
        ("tile_reflection", &[40.0, 30.0, 8.0, 0.0]),
        ("twist", &[30.0, 100.0, 0.5, 0.5, 0.0]),
    ];

    /// effects-distort: Pixel mode's two distortions change the target as
    /// one undo step.
    #[test]
    fn apply_effect_morphology_and_pixelate_run_on_a_pixel_document() {
        for (name, params) in [
            ("morphology", &[0.0, 0.0, 1.0][..]),
            ("pixelate", &[2.0, 1.0]),
        ] {
            let (mut doc, _) = layered();
            let before = cel_pixels(&doc, 0, 0);
            let depth = doc.undo_depth();
            assert!(doc.apply_effect(name, params).expect(name), "{name}");
            assert_eq!(doc.undo_depth(), depth + 1, "{name}: one undo step");
            assert_ne!(cel_pixels(&doc, 0, 0), before, "{name} changed nothing");
        }
    }

    /// effects-distort AC-06: Pixelate's grid starts at the canvas origin,
    /// also for a trimmed cel, so Centre adds no colour.
    #[test]
    fn apply_effect_pixelate_aligns_a_trimmed_cel_to_the_canvas_grid() {
        let mut doc = Document::new(8, 8).expect("doc");
        let (w, h) = (5u32, 5u32);
        let mut buf = pincel_core::PixelBuffer::empty(w, h, ColorMode::Rgba);
        for (i, px) in buf.data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            px.copy_from_slice(&[(i * 10) as u8, (i * 3) as u8, 200, 255]);
        }
        let before = buf.data.clone();
        let mut cel = pincel_core::Cel::image(crate::DEFAULT_LAYER_ID, FrameIndex::new(0), buf);
        cel.position = (3, 1);
        doc.cels.insert(cel);
        assert!(doc.apply_effect("pixelate", &[4.0, 1.0]).expect("pixelate"));
        let after = cel_pixels(&doc, 0, 0);
        let at = |data: &[u8], x: usize, y: usize| {
            let o = (y * w as usize + x) * 4;
            data[o..o + 4].to_vec()
        };
        // Canvas squares 0..4 and 4..8: cel columns 0 | 1..5 (canvas x 3 |
        // 4..8), cel rows 0..3 | 3..5 (canvas y 1..4 | 4..6).
        for (xs, ys, centre) in [
            (0..1, 0..3, (0, 1)),
            (1..5, 0..3, (2, 1)),
            (0..1, 3..5, (0, 3)),
            (1..5, 3..5, (2, 3)),
        ] {
            for y in ys.clone() {
                for x in xs.clone() {
                    assert_eq!(
                        at(&after, x, y),
                        at(&before, centre.0, centre.1),
                        "cel pixel ({x}, {y})"
                    );
                }
            }
        }
    }

    /// effects-distort AC-13: a wrong parameter count is refused and changes
    /// nothing.
    #[test]
    fn apply_effect_distortions_wrong_parameter_count_changes_nothing() {
        for (name, params) in DISTORTIONS {
            let (mut doc, _) = layered();
            let before = cel_pixels(&doc, 0, 0);
            let depth = doc.undo_depth();
            for wrong in [vec![1.0; params.len() + 1], vec![1.0; params.len() - 1]] {
                let err = doc.apply_effect(name, &wrong).expect_err(name);
                assert!(err.contains("expects"), "{name}: {err}");
                assert!(
                    doc.preview_effect(name, &wrong, 1).is_err(),
                    "{name} preview"
                );
            }
            assert_eq!(doc.undo_depth(), depth);
            assert_eq!(cel_pixels(&doc, 0, 0), before);
        }
    }

    /// effects-distort AC-14: an indexed document refuses Morphology and
    /// Pixelate (and every other distortion) with the message every other
    /// effect gives.
    #[test]
    fn apply_effect_distortions_on_an_indexed_document_are_refused() {
        let mut doc = Document::new(2, 1).expect("doc");
        let mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        doc.sprite.color_mode = mode;
        let mut buf = pincel_core::PixelBuffer::empty(2, 1, mode);
        buf.data = vec![1, 0];
        doc.cels.insert(pincel_core::Cel::image(
            crate::DEFAULT_LAYER_ID,
            FrameIndex::new(0),
            buf,
        ));
        let before = cel_pixels(&doc, 0, 0);
        let expected = doc.apply_effect("invert", &[]).expect_err("invert");
        for (name, params) in DISTORTIONS {
            assert_eq!(
                doc.apply_effect(name, params),
                Err(expected.clone()),
                "{name}"
            );
            assert!(
                doc.preview_effect(name, params, 1).is_err(),
                "{name} preview"
            );
        }
        assert_eq!(doc.undo_depth(), 0);
        assert_eq!(cel_pixels(&doc, 0, 0), before);
    }

    #[test]
    fn preview_effect_honours_zoom() {
        let doc = Document::new(2, 3).expect("dims");
        let frame = doc.preview_effect("sharpen", &[], 4).expect("preview");
        assert_eq!((frame.width(), frame.height()), (8, 12));
    }

    /// docs/specs/effects-artistic-photo.md: the Artistic and Photo
    /// effects with the table's defaults, in wire order.
    const ARTISTIC_PHOTO: [(&str, &[f64]); 8] = [
        ("ink_sketch", &[50.0, 50.0]),
        ("oil_painting", &[3.0, 50.0]),
        ("pencil_sketch", &[2.0, 0.0]),
        ("glow", &[6.0, 10.0, 10.0]),
        ("red_eye_removal", &[70.0, 90.0]),
        ("soften_portrait", &[5.0, 0.0, 10.0]),
        ("straighten", &[0.0, 0.0]),
        ("vignette", &[0.5, 0.5, 100.0, 100.0, 0.0]),
    ];

    /// effects-artistic-photo AC-12: a wrong parameter count is refused and
    /// changes nothing; the right count is accepted.
    #[test]
    fn apply_effect_artistic_photo_wrong_parameter_count_changes_nothing() {
        for (name, params) in ARTISTIC_PHOTO {
            let (mut doc, _) = layered();
            let before = cel_pixels(&doc, 0, 0);
            let depth = doc.undo_depth();
            for wrong in [vec![1.0; params.len() + 1], vec![1.0; params.len() - 1]] {
                let err = doc.apply_effect(name, &wrong).expect_err(name);
                assert!(err.contains("expects"), "{name}: {err}");
                assert!(
                    doc.preview_effect(name, &wrong, 1).is_err(),
                    "{name} preview"
                );
            }
            assert_eq!(doc.undo_depth(), depth);
            assert_eq!(cel_pixels(&doc, 0, 0), before);
            assert!(doc.preview_effect(name, params, 1).is_ok(), "{name}");
        }
    }
}
