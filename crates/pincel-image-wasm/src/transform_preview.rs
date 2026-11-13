//! The drag preview of the Move tools' transform box
//! (docs/specs/image-transform-handles.md AC-20, AC-23): once per drag the
//! editor gets the lifted selection content and the visible composite with
//! the hole the lift leaves, draws both itself while the pointer moves, and
//! sends the final box to the core when the drag ends.

use js_sys::{Array, Object, Reflect, Uint8ClampedArray};
use pincel_effects::EffectImage;
use pincel_effects::transform::{Affine, Quality, resample};
use pincel_image_core::RenderCache;
use pincel_image_core::geometry::Rect;
use wasm_bindgen::prelude::*;

use crate::{CACHES, to_js, with_bus};

/// The longest side of the patch a drag preview draws: a longer selection
/// is downscaled for the preview only (a canvas this size stays within
/// every browser's limits), the released result keeps full resolution.
const PATCH_MAX_SIDE: u32 = 4096;

/// `patch` at most [`PATCH_MAX_SIDE`] on its longer side, aspect kept.
fn preview_patch(patch: EffectImage) -> EffectImage {
    let (w, h) = (patch.width(), patch.height());
    let long = w.max(h);
    if long <= PATCH_MAX_SIDE {
        return patch;
    }
    let f = f64::from(PATCH_MAX_SIDE) / f64::from(long);
    let nw = ((f64::from(w) * f).round() as u32).max(1);
    let nh = ((f64::from(h) * f).round() as u32).max(1);
    let inverse = Affine {
        a: f64::from(w) / f64::from(nw),
        e: f64::from(h) / f64::from(nh),
        ..Affine::IDENTITY
    };
    resample(&patch, nw, nh, &inverse, Quality::Bilinear)
}

fn set(obj: &Object, key: &str, value: &JsValue) -> Result<(), JsError> {
    Reflect::set(obj, &JsValue::from_str(key), value)
        .map(|_| ())
        .map_err(|_| JsError::new("transform preview: cannot build the result"))
}

fn numbers(values: &[f64]) -> JsValue {
    values
        .iter()
        .map(|&v| JsValue::from_f64(v))
        .collect::<Array>()
        .into()
}

/// Lifts the selection's content out of `layer` for a drag of the transform
/// box: `pixels` for Move Selected Pixels, outline only otherwise; a live
/// transform's own layer and kind win. Returns `null` without a selection
/// that selects a pixel, else `{ bounds: [x, y, w, h], opacity, patch,
/// patchSize: [w, h], hole }`: `patch` the lifted pixels (straight RGBA8,
/// `null` for Move Selection), downscaled to `patchSize` when longer than
/// 4096 px; `hole` the canvas rect `view = [x, y, w, h, out_w, out_h]`
/// presented at `out_w × out_h` exactly as `composite_region` would, with
/// the content taken out. The document, its selection and its history stay
/// as they were.
#[wasm_bindgen]
pub fn transform_preview(
    handle: u32,
    layer: usize,
    pixels: bool,
    view: &[u32],
) -> Result<JsValue, JsError> {
    let &[x, y, w, h, out_w, out_h] = view else {
        return Err(JsError::new(
            "transform preview: view is [x, y, w, h, out_w, out_h]",
        ));
    };
    let rect = Rect::new(
        i32::try_from(x).unwrap_or(i32::MAX),
        i32::try_from(y).unwrap_or(i32::MAX),
        w,
        h,
    );
    with_bus(handle, |bus| {
        let lifted = bus
            .with_transform_lifted(layer, pixels, |doc, damage| {
                CACHES.with(|caches| {
                    let mut caches = caches.borrow_mut();
                    let cache = caches
                        .entry(handle)
                        .or_insert_with(|| RenderCache::new(doc));
                    cache.update(doc, damage);
                    cache.region(doc, rect, out_w, out_h).into_raw()
                })
            })
            .map_err(to_js)?;
        let Some((lift, hole)) = lifted else {
            return Ok(JsValue::NULL);
        };
        let out = Object::new();
        let b = lift.bounds;
        set(
            &out,
            "bounds",
            &numbers(&[
                f64::from(b.x),
                f64::from(b.y),
                f64::from(b.w),
                f64::from(b.h),
            ]),
        )?;
        set(&out, "opacity", &JsValue::from_f64(f64::from(lift.opacity)))?;
        match lift.patch.map(preview_patch) {
            Some(p) => {
                set(
                    &out,
                    "patchSize",
                    &numbers(&[f64::from(p.width()), f64::from(p.height())]),
                )?;
                set(&out, "patch", &Uint8ClampedArray::from(p.data()).into())?;
            }
            None => {
                set(&out, "patchSize", &numbers(&[0.0, 0.0]))?;
                set(&out, "patch", &JsValue::NULL)?;
            }
        }
        set(
            &out,
            "hole",
            &Uint8ClampedArray::from(hole.as_slice()).into(),
        )?;
        Ok(out.into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(w: u32, h: u32) -> EffectImage {
        EffectImage::from_rgba8(w, h, vec![200; (w * h * 4) as usize]).expect("image")
    }

    #[test]
    fn preview_patch_keeps_a_patch_up_to_the_limit() {
        let p = preview_patch(image(PATCH_MAX_SIDE, 3));
        assert_eq!((p.width(), p.height()), (PATCH_MAX_SIDE, 3));
    }

    #[test]
    fn preview_patch_downscales_a_longer_patch_keeping_its_aspect() {
        let p = preview_patch(image(PATCH_MAX_SIDE * 2, 400));
        assert_eq!((p.width(), p.height()), (PATCH_MAX_SIDE, 200));
        assert_eq!(&p.data()[..4], &[200, 200, 200, 200]);
    }
}
