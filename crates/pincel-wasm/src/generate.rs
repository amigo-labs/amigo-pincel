//! Image generation in Pixel mode (docs/specs/comfyui-generate-pixel.md):
//! what the Generate panel reads from the sprite when the user clicks
//! Generate — the input and the mask, fitted to the generation size — and
//! each variant scaled to the target box and mapped into the sprite's color
//! mode, previewed as PNG and applied as a new layer. The generation itself
//! runs in the desktop app's backend.

use pincel_core::generate::{
    self as generate, add_generated_layer, generation_input, generation_mask, has_visible_color,
    prepare_variant, shown_rgba,
};
use pincel_core::{HistoryKind, Layer, LayerId, PixelBuffer, Rect, encode_png, import_png};
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

/// `[width, height]` for `native` pixels at the aspect ratio of a `w × h`
/// target box (comfyui-generate Terms, "Generation size").
#[wasm_bindgen(js_name = generationSize)]
pub fn generation_size(native: u32, w: u32, h: u32) -> Vec<u32> {
    let (gw, gh) = generate::generation_size(native, w, h);
    vec![gw, gh]
}

/// A `w × h` mask fitted to `gen_w × gen_h` like the input, as a grey PNG
/// (white = generate).
#[wasm_bindgen(js_name = generationMaskPng)]
pub fn generation_mask_png(
    mask: &[u8],
    w: u32,
    h: u32,
    gen_w: u32,
    gen_h: u32,
) -> Result<Vec<u8>, String> {
    let rgba: Vec<u8> = mask.iter().flat_map(|&m| [m, m, m, 255]).collect();
    let fitted = generate::fit(&rgba, w, h, gen_w, gen_h);
    encode_png(&fitted, gen_w, gen_h).map_err(|e| e.to_string())
}

#[wasm_bindgen]
impl Document {
    /// The selection's coverage inside the target box `x, y, w, h` (one
    /// byte per pixel); empty without a selection.
    #[wasm_bindgen(js_name = generationMask)]
    pub fn generation_mask(&self, x: i32, y: i32, w: u32, h: u32) -> Vec<u8> {
        generation_mask(&self.sprite, Rect::new(x, y, w, h))
    }

    /// The active layer inside the target box as the sprite shows it,
    /// enlarged to `gen_w × gen_h` with hard pixel edges (AC-07, AC-09), as
    /// PNG bytes; empty when the layer is fully transparent there.
    #[wasm_bindgen(js_name = generationInput)]
    pub fn generation_input(
        &self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        gen_w: u32,
        gen_h: u32,
    ) -> Result<Vec<u8>, String> {
        let layer = self.copy_layer()?;
        let area = Rect::new(x, y, w, h);
        let pixels = generation_input(&self.sprite, &self.cels, self.current_frame, layer, area)
            .map_err(|e| e.to_string())?;
        match pixels {
            Some(rgba) => encode_png(&generate::fit(&rgba, w, h, gen_w, gen_h), gen_w, gen_h)
                .map_err(|e| e.to_string()),
            None => Ok(Vec::new()),
        }
    }

    /// `false` while an indexed sprite's palette has no entry besides the
    /// transparent one: nothing generated could show (AC-10).
    #[wasm_bindgen(getter, js_name = canHoldGenerated)]
    pub fn can_hold_generated(&self) -> bool {
        has_visible_color(&self.sprite)
    }

    /// A variant (encoded image at generation size) scaled to the target
    /// box `area` (`[x, y, w, h]`), limited to `mask` (empty: none) and
    /// mapped into the sprite's color mode, as a `w × h` PNG of what Apply
    /// would show (AC-02 – AC-05).
    #[wasm_bindgen(js_name = generationPreview)]
    pub fn generation_preview(
        &self,
        variant: &[u8],
        area: &[i32],
        mask: &[u8],
    ) -> Result<Vec<u8>, String> {
        let (area, pixels) = self.prepared(variant, area, mask)?;
        encode_png(
            &shown_rgba(&self.sprite.palette, &pixels),
            area.width,
            area.height,
        )
        .map_err(|e| e.to_string())
    }

    /// Apply: the variant as [`Document::generation_preview`] makes it, as
    /// a new layer named `name` directly above the active layer with a cel
    /// in the current frame — one undo step (AC-06, AC-09). Returns the
    /// new layer's id, which becomes active.
    #[wasm_bindgen(js_name = addGeneratedLayer)]
    pub fn add_generated_layer(
        &mut self,
        variant: &[u8],
        area: &[i32],
        mask: &[u8],
        name: &str,
    ) -> Result<u32, String> {
        let (area, pixels) = self.prepared(variant, area, mask)?;
        let new_id = match self.sprite.layers.iter().map(|l| l.id.0).max() {
            None => 0,
            Some(u32::MAX) => return Err("layer id space exhausted".to_string()),
            Some(m) => m + 1,
        };
        let above = self.copy_layer().ok();
        let cmd = add_generated_layer(
            &self.sprite,
            Layer::image(LayerId::new(new_id), name),
            above,
            self.current_frame,
            area,
            &pixels,
        );
        self.bus
            .execute_as(
                HistoryKind::Generate,
                cmd.into(),
                &mut self.sprite,
                &mut self.cels,
            )
            .map_err(|e| e.to_string())?;
        self.bus.seal();
        self.active_layer = Some(LayerId::new(new_id));
        self.events.push(Event::dirty_canvas());
        Ok(new_id)
    }
}

impl Document {
    /// The decoded `variant` prepared for the target box `[x, y, w, h]`.
    fn prepared(
        &self,
        variant: &[u8],
        area: &[i32],
        mask: &[u8],
    ) -> Result<(Rect, PixelBuffer), String> {
        let &[x, y, w, h] = area else {
            return Err("the target box is [x, y, width, height]".to_string());
        };
        let (Ok(w), Ok(h)) = (u32::try_from(w), u32::try_from(h)) else {
            return Err("the target box is empty".to_string());
        };
        let area = Rect::new(x, y, w, h);
        let img = import_png(variant).map_err(|e| e.to_string())?;
        let pixels = prepare_variant(&self.sprite, &img.rgba, img.width, img.height, area, mask)
            .map_err(|e| e.to_string())?;
        Ok((area, pixels))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pincel_core::{Cel, CelData, ColorMode, FrameIndex, PaletteEntry, Rgba};

    const RED: u32 = 0xFF0000FF;

    fn png(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
        encode_png(rgba, w, h).expect("png")
    }

    fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let img = import_png(bytes).expect("decode");
        (img.width, img.height, img.rgba)
    }

    /// A 4×4 indexed sprite: 0 transparent, 1 green, 2 blue.
    fn indexed() -> Document {
        let mut doc = Document::new(4, 4).expect("doc");
        doc.sprite.color_mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        doc.sprite.palette.colors = vec![
            PaletteEntry::new(Rgba::new(0, 0, 0, 255)),
            PaletteEntry::new(Rgba::new(0, 200, 0, 255)),
            PaletteEntry::new(Rgba::new(0, 0, 200, 255)),
        ];
        doc
    }

    #[test]
    fn generation_size_matches_the_rule() {
        assert_eq!(generation_size(512 * 512, 32, 16), vec![704, 384]);
    }

    #[test]
    fn generation_input_enlarges_the_active_layer_with_hard_edges() {
        let mut doc = Document::new(2, 1).expect("doc");
        doc.apply_tool("pencil", 0, 0, RED).expect("paint");
        doc.end_stroke();
        let bytes = doc.generation_input(0, 0, 2, 1, 128, 64).expect("input");
        let (w, h, rgba) = decode(&bytes);
        assert_eq!((w, h), (128, 64));
        assert_eq!(&rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&rgba[63 * 4..64 * 4], &[255, 0, 0, 255], "hard edge");
        assert_eq!(&rgba[64 * 4..65 * 4], &[0, 0, 0, 0]);
        // Nothing to refine on an empty layer.
        let empty = Document::new(2, 1).expect("doc");
        assert!(
            empty
                .generation_input(0, 0, 2, 1, 128, 64)
                .expect("input")
                .is_empty()
        );
    }

    #[test]
    fn generation_mask_png_is_the_fitted_mask() {
        let bytes = generation_mask_png(&[255, 0], 2, 1, 4, 2).expect("png");
        let (_, _, rgba) = decode(&bytes);
        assert_eq!(&rgba[0..4], &[255, 255, 255, 255]);
        assert_eq!(&rgba[8..12], &[0, 0, 0, 255]);
    }

    #[test]
    fn generation_preview_is_the_box_size_in_palette_colors() {
        let mut doc = indexed();
        doc.set_selection(1, 1, 2, 2);
        let mask = doc.generation_mask(1, 1, 2, 2);
        // 8×8 bright green variant → 2×2 in the palette's green.
        let variant = png(&[0, 255, 0, 255].repeat(64), 8, 8);
        let (w, h, rgba) = decode(
            &doc.generation_preview(&variant, &[1, 1, 2, 2], &mask)
                .expect("preview"),
        );
        assert_eq!((w, h), (2, 2));
        assert_eq!(rgba, [0, 200, 0, 255].repeat(4));
        assert_eq!(doc.undo_depth(), 0, "a preview changes nothing");
    }

    #[test]
    fn add_generated_layer_is_one_undo_step_with_an_indexed_cel() {
        let mut doc = indexed();
        let variant = png(&[0, 0, 255, 255].repeat(4), 2, 2);
        let id = doc
            .add_generated_layer(&variant, &[2, 2, 2, 2], &[], "Generated: slime")
            .expect("apply");
        assert_eq!(doc.layer_count(), 2);
        assert_eq!(doc.layer_id_at(1).expect("id"), id);
        assert_eq!(doc.undo_kinds(), vec!["generate".to_string()]);
        let cel = doc
            .cels
            .get(LayerId::new(id), FrameIndex::new(0))
            .expect("cel");
        let CelData::Image(buf) = &cel.data else {
            panic!("not an image cel")
        };
        assert_eq!(buf.data[2 * 4 + 2], 2, "the palette's blue");
        assert_eq!(buf.data[0], 0, "transparent outside the box");
        assert_eq!(doc.sprite.palette.colors.len(), 3, "palette unchanged");
        assert!(doc.undo());
        assert_eq!(doc.layer_count(), 1);
    }

    #[test]
    fn a_palette_without_a_visible_color_refuses_generation() {
        let mut doc = indexed();
        doc.sprite.palette.colors.truncate(1);
        assert!(!doc.can_hold_generated());
        let variant = png(&[0, 0, 255, 255], 1, 1);
        let err = doc
            .add_generated_layer(&variant, &[0, 0, 1, 1], &[], "g")
            .expect_err("refused");
        assert_eq!(err, "The palette has no visible color");
        assert_eq!(doc.layer_count(), 1);
        assert!(Document::new(1, 1).expect("doc").can_hold_generated());
    }

    #[test]
    fn grayscale_previews_gray_with_its_alpha() {
        let mut doc = Document::new(1, 1).expect("doc");
        doc.sprite.color_mode = ColorMode::Grayscale;
        doc.cels.insert(Cel::image(
            crate::DEFAULT_LAYER_ID,
            FrameIndex::new(0),
            PixelBuffer::empty(1, 1, ColorMode::Grayscale),
        ));
        let variant = png(&[0, 255, 0, 255], 1, 1);
        let (_, _, rgba) = decode(
            &doc.generation_preview(&variant, &[0, 0, 1, 1], &[])
                .expect("preview"),
        );
        assert_eq!(rgba, vec![211, 211, 211, 255]);
    }
}
