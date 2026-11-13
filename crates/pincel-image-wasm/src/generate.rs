//! Image generation (docs/specs/comfyui-generate.md): what the Generate
//! panel reads from the document when the user clicks Generate — the
//! input and the mask, fitted to the generation size — and the variant it
//! previews and applies as a new layer. The generation itself runs in the
//! desktop app's backend.

use pincel_image_core::codec::{decode, to_png_bytes};
use pincel_image_core::command::{HistoryKind, PasteLayer};
use pincel_image_core::generate::{
    apply_mask, fit, fit_mask, generation_input as input_pixels, generation_mask as mask_bytes,
    generation_size as size_for,
};
use pincel_image_core::{ImageBuffer, Rect};
use wasm_bindgen::Clamped;
use wasm_bindgen::prelude::*;

use crate::{to_js, with_bus};

fn codec_js(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// `[width, height]` for `native` pixels at the aspect ratio of a `w × h`
/// target box (Terms, "Generation size").
#[wasm_bindgen]
pub fn generation_size(native: u32, w: u32, h: u32) -> Vec<u32> {
    let (gw, gh) = size_for(native, w, h);
    vec![gw, gh]
}

/// The selection's coverage inside the target box `x, y, w, h` (one byte
/// per pixel); empty without a selection.
#[wasm_bindgen]
pub fn generation_mask(handle: u32, x: i32, y: i32, w: u32, h: u32) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        Ok(mask_bytes(&bus.document, Rect::new(x, y, w, h)))
    })
}

/// The active layer's pixels inside the target box (a group's composite),
/// fitted to `gen_w × gen_h`, as PNG bytes; empty when the layer is fully
/// transparent there.
#[wasm_bindgen]
pub fn generation_input(
    handle: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    gen_w: u32,
    gen_h: u32,
) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        match input_pixels(&bus.document, Rect::new(x, y, w, h)) {
            Some(pixels) => to_png_bytes(&fit(&pixels, gen_w, gen_h), 1).map_err(codec_js),
            None => Ok(Vec::new()),
        }
    })
}

/// A `w × h` mask fitted to `gen_w × gen_h`, as a grey PNG (white =
/// generate).
#[wasm_bindgen]
pub fn generation_mask_png(
    mask: &[u8],
    w: u32,
    h: u32,
    gen_w: u32,
    gen_h: u32,
) -> Result<Vec<u8>, JsError> {
    let fitted = fit_mask(mask, w, h, gen_w, gen_h);
    let rgba: Vec<u8> = fitted.iter().flat_map(|&m| [m, m, m, 255]).collect();
    let buf = ImageBuffer::from_raw(gen_w, gen_h, rgba).map_err(to_js)?;
    to_png_bytes(&buf, 1).map_err(codec_js)
}

/// A variant (encoded image) fitted to the `w × h` target box, its alpha
/// multiplied by `mask` (empty: none) — straight RGBA for the canvas
/// preview and for [`add_generated_layer`].
#[wasm_bindgen]
pub fn fit_variant(data: &[u8], w: u32, h: u32, mask: &[u8]) -> Result<Clamped<Vec<u8>>, JsError> {
    let variant = decode(data).map_err(codec_js)?;
    let mut out = fit(&variant, w, h);
    apply_mask(&mut out, mask);
    Ok(Clamped(out.into_raw()))
}

/// Adds a layer named `name` directly above the active layer (inside its
/// group), holding the `w × h` straight RGBA `rgba` at `x, y` and
/// transparent elsewhere — one undo step (AC-12).
#[wasm_bindgen]
pub fn add_generated_layer(
    handle: u32,
    rgba: &[u8],
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    name: &str,
) -> Result<(), JsError> {
    let pixels = ImageBuffer::from_raw(w, h, rgba.to_vec()).map_err(to_js)?;
    with_bus(handle, |bus| {
        bus.apply(Box::new(
            PasteLayer::at(pixels, x, y, false).labelled(name, HistoryKind::Generate),
        ))
        .map_err(to_js)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{close_document, create_document, undo};
    use pincel_image_core::{Color, SelectionMask};

    fn with_doc<T>(handle: u32, f: impl FnOnce(&mut pincel_image_core::Document) -> T) -> T {
        with_bus(handle, |bus| Ok(f(&mut bus.document)))
            .map_err(|_| "handle")
            .expect("open document")
    }

    #[test]
    fn generation_inputs_come_from_the_active_layer_and_the_selection() {
        let h = create_document(16, 16).unwrap();
        assert!(
            generation_input(h, 0, 0, 16, 16, 64, 64)
                .unwrap()
                .is_empty()
        );
        assert!(generation_mask(h, 0, 0, 16, 16).unwrap().is_empty());
        with_doc(h, |doc| {
            doc.active_layer_mut()
                .pixels
                .set_pixel(4, 4, Color::rgba(200, 0, 0, 255));
            let mut sel = SelectionMask::new_empty(16, 16);
            sel.set(5, 4, 255);
            doc.selection = Some(sel);
        });
        let png = generation_input(h, 4, 4, 8, 4, 128, 64).unwrap();
        let img = decode(&png).unwrap();
        assert_eq!((img.width(), img.height()), (128, 64));
        let mask = generation_mask(h, 4, 4, 8, 4).unwrap();
        assert_eq!(mask.len(), 32);
        assert_eq!((mask[0], mask[1]), (0, 255));
        let mask_png = decode(&generation_mask_png(&mask, 8, 4, 128, 64).unwrap()).unwrap();
        assert_eq!((mask_png.width(), mask_png.height()), (128, 64));
        assert_eq!(generation_size(512 * 512, 8, 4), [704, 384]);
        close_document(h);
    }

    #[test]
    fn a_fitted_variant_lands_as_one_named_layer_that_undo_removes() {
        let h = create_document(8, 8).unwrap();
        let variant = to_png_bytes(
            &ImageBuffer::from_raw(64, 64, [0, 0, 255, 255].repeat(64 * 64)).unwrap(),
            1,
        )
        .unwrap();
        let rgba = fit_variant(&variant, 2, 2, &[255, 0, 128, 255]).unwrap().0;
        assert_eq!(rgba.len(), 16);
        assert_eq!(&rgba[0..4], &[0, 0, 255, 255]);
        assert_eq!(&rgba[4..8], &[0, 0, 0, 0]);
        assert_eq!(rgba[11], 128);
        add_generated_layer(h, &rgba, 3, 5, 2, 2, "Generated: a red apple").unwrap();
        with_doc(h, |doc| {
            assert_eq!(doc.layers().len(), 2);
            assert_eq!(doc.layers()[1].name, "Generated: a red apple");
            assert_eq!(doc.active_layer_index(), 1);
            assert_eq!(
                doc.layers()[1].pixels.get_pixel(3, 5),
                Some(Color::rgba(0, 0, 255, 255))
            );
            assert_eq!(
                doc.layers()[1].pixels.get_pixel(4, 5),
                Some(Color::TRANSPARENT)
            );
            assert_eq!(
                doc.layers()[1].pixels.get_pixel(0, 0),
                Some(Color::TRANSPARENT)
            );
        });
        assert!(undo(h).unwrap());
        with_doc(h, |doc| assert_eq!(doc.layers().len(), 1));
        close_document(h);
    }
}
