//! Pixel mode's dithered Gradient tool (`docs/specs/dithered-gradient.md`).

use pincel_core::{ColorMode, DitherMatrix, DrawDitherGradient, GradientKind, Rgba};
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

#[wasm_bindgen]
impl Document {
    /// Fill the selection — or, without one, the active layer's cel on the
    /// current frame — with a dithered ramp from `foreground` at the drag
    /// start to `background` at the drag end, as one undo entry.
    ///
    /// - `line`: `[x0, y0, x1, y1]`, the drag's start and end sprite pixels.
    /// - `kind`: `linear` or `radial`.
    /// - `dither`: the Bayer matrix size `2`, `4` or `8`, or `0` for two
    ///   hard bands split halfway.
    /// - `foreground` / `background`: `0xRRGGBBAA`.
    ///
    /// Returns `false` — and changes nothing — for a zero-length drag.
    /// Errors (and changes nothing) on a locked layer, a document without a
    /// paintable image layer, and an indexed or grayscale sprite.
    #[wasm_bindgen(js_name = applyGradient)]
    pub fn apply_gradient(
        &mut self,
        line: &[i32],
        kind: &str,
        dither: u32,
        foreground: u32,
        background: u32,
    ) -> Result<bool, String> {
        let &[x0, y0, x1, y1] = line else {
            return Err(format!(
                "gradient line needs 4 coordinates, got {}",
                line.len()
            ));
        };
        let kind = GradientKind::from_name(kind)
            .ok_or_else(|| format!("unknown gradient kind `{kind}`"))?;
        let matrix = DitherMatrix::from_size(dither)
            .ok_or_else(|| format!("unknown dither matrix size {dither}"))?;
        let layer = self.edit_target_layer()?;
        if !matches!(self.sprite.color_mode, ColorMode::Rgba) {
            return Err("the gradient paints RGBA sprites only".to_string());
        }
        let frame = self.current_frame;
        let colors = (Rgba::from_u32(foreground), Rgba::from_u32(background));
        let Some(cmd) =
            DrawDitherGradient::new(layer, frame, (x0, y0), (x1, y1), kind, matrix, colors)
        else {
            return Ok(false);
        };
        self.ensure_paint_cel(layer, frame);
        self.bus.seal();
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to draw gradient: {e}"))?;
        self.bus.seal();
        if let Some(ev) = Event::from_dirty(self.bus.last_dirty_region()) {
            self.events.push(ev);
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pincel_core::{Layer, LayerId};

    const FG: u32 = 0x112233FF;
    const BG: u32 = 0xDDEEFFFF;

    fn row(doc: &Document, w: i32, y: i32) -> Vec<u32> {
        (0..w)
            .map(|x| doc.pick_color(0, x, y).expect("pick"))
            .collect()
    }

    #[test]
    fn apply_gradient_paints_fg_to_bg_as_one_undo_entry() {
        let mut doc = Document::new(16, 1).expect("dims");
        assert!(
            doc.apply_gradient(&[0, 0, 15, 0], "linear", 4, FG, BG)
                .expect("gradient")
        );
        let after = row(&doc, 16, 0);
        assert_eq!(after[0], FG);
        assert_eq!(after[15], BG);
        assert!(after.iter().all(|&c| c == FG || c == BG));
        assert_eq!(doc.undo_depth(), 1);
        assert!(doc.undo());
        assert!(row(&doc, 16, 0).iter().all(|&c| c & 0xFF == 0));
        assert!(doc.redo().expect("redo"));
        assert_eq!(row(&doc, 16, 0), after);
    }

    #[test]
    fn apply_gradient_zero_length_drag_changes_nothing() {
        let mut doc = Document::new(4, 4).expect("dims");
        let drawn = doc
            .apply_gradient(&[2, 1, 2, 1], "radial", 2, FG, BG)
            .expect("no error");
        assert!(!drawn);
        assert_eq!(doc.undo_depth(), 0);
        assert!(row(&doc, 4, 1).iter().all(|&c| c & 0xFF == 0));
    }

    #[test]
    fn apply_gradient_on_locked_layer_errs_without_undo_entry() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.set_layer_editable(0, false).expect("lock");
        let depth = doc.undo_depth();
        let err = doc
            .apply_gradient(&[0, 0, 3, 0], "linear", 4, FG, BG)
            .expect_err("locked");
        assert!(err.contains("locked"));
        assert_eq!(doc.undo_depth(), depth);
        assert!(row(&doc, 4, 0).iter().all(|&c| c & 0xFF == 0));
    }

    #[test]
    fn apply_gradient_without_image_layer_errs() {
        let mut doc = Document::new(2, 2).expect("dims");
        doc.sprite.layers.clear();
        doc.sprite
            .layers
            .push(Layer::group(LayerId::new(7), "folder"));
        let err = doc
            .apply_gradient(&[0, 0, 1, 1], "linear", 4, FG, BG)
            .expect_err("nothing to paint");
        assert!(err.contains("no paintable image layer"));
        assert_eq!(doc.undo_depth(), 0);
    }

    #[test]
    fn apply_gradient_on_indexed_or_grayscale_sprite_errs_without_change() {
        for mode in [
            ColorMode::Indexed {
                transparent_index: 0,
            },
            ColorMode::Grayscale,
        ] {
            let mut doc = Document::new(4, 4).expect("dims");
            doc.sprite.color_mode = mode;
            let cels_before = doc.cels.clone();
            let err = doc
                .apply_gradient(&[0, 0, 3, 0], "linear", 4, FG, BG)
                .expect_err("not RGBA");
            assert!(err.contains("RGBA"), "{err}");
            assert_eq!(doc.undo_depth(), 0);
            assert_eq!(doc.cels, cels_before);
        }
    }

    #[test]
    fn apply_gradient_rejects_bad_arguments() {
        let mut doc = Document::new(4, 4).expect("dims");
        assert!(doc.apply_gradient(&[0, 0, 3], "linear", 4, FG, BG).is_err());
        assert!(
            doc.apply_gradient(&[0, 0, 3, 0], "conical", 4, FG, BG)
                .is_err()
        );
        assert!(
            doc.apply_gradient(&[0, 0, 3, 0], "linear", 3, FG, BG)
                .is_err()
        );
        assert_eq!(doc.undo_depth(), 0);
    }
}
