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
        let layer = self.paint_target_layer()?;
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
        let out = spec.run(&src).into_raw();

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
        // Keep the original outside the selection: copy the affected rows'
        // spans from the effect output into a clone of the source.
        let mut merged = buffer.data.clone();
        let stride = buffer.width as usize * 4;
        let x0 = (inter.x - cel_rect.x) as usize * 4;
        let span = inter.width as usize * 4;
        for row in 0..inter.height as usize {
            let y = (inter.y - cel_rect.y) as usize + row;
            let start = y * stride + x0;
            merged[start..start + span].copy_from_slice(&out[start..start + span]);
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

    #[test]
    fn apply_effect_unknown_name_or_bad_arity_errors() {
        let mut doc = Document::new(2, 2).expect("dims");
        assert!(doc.apply_effect("sepia", &[]).is_err());
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

    #[test]
    fn preview_effect_honours_zoom() {
        let doc = Document::new(2, 3).expect("dims");
        let frame = doc.preview_effect("sharpen", &[], 4).expect("preview");
        assert_eq!((frame.width(), frame.height()), (8, 12));
    }
}
