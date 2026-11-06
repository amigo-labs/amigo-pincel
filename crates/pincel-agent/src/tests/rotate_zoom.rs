//! Layer ▸ Rotate / Zoom through `effect.apply`
//! (docs/specs/image-transform-handles-rotate-zoom.md AC-10).

use pincel_effects::EffectSpec;
use pincel_image_core::CommandBus;
use pincel_image_core::SetPixels;
use pincel_image_core::jobs::effect_region;
use serde_json::json;

use super::S;

/// image-transform-handles-rotate-zoom AC-10: `effect.apply` with
/// Rotate / Zoom gives the editor's Apply pixels (its effect job and one
/// `SetPixels`), with and without a selection.
#[test]
fn effect_apply_rotate_zoom_matches_the_editor() {
    let cases = [
        (
            json!({ "angle": 30, "pan_x": 10, "pan_y": -5, "zoom": 1.5, "quality": "bicubic" }),
            [30.0, 10.0, -5.0, 1.5, 2.0],
        ),
        (
            json!({ "angle": -90, "zoom": 0.5, "quality": "nearest" }),
            [-90.0, 0.0, 0.0, 0.5, 0.0],
        ),
        (
            json!({ "pan_x": 12.5, "pan_y": 33.3 }),
            [0.0, 12.5, 33.3, 1.0, 1.0],
        ),
    ];
    for select in [false, true] {
        for (params, wire) in &cases {
            let mut s = S::new();
            let id = s.new_image(16, 12);
            s.ok(
                "draw.gradient",
                json!({ "x0": 0, "y0": 0, "x1": 16, "y1": 12, "from": "#102030", "to": "#e0c04080" }),
            );
            if select {
                s.ok(
                    "select.rect",
                    json!({ "x": 3, "y": 2, "width": 9, "height": 7, "feather": 2 }),
                );
            }
            let doc = s.image(&id).document().clone();
            let spec = EffectSpec::from_params("rotate_zoom", wire).expect("spec");
            let region = effect_region(&doc, 0, &spec).expect("region");
            let mut bus = CommandBus::new(doc);
            let (rect, pixels) = region.expect("a region");
            bus.apply(Box::new(SetPixels::new(0, rect, pixels)))
                .expect("apply");
            s.ok(
                "effect.apply",
                json!({ "id": "rotate_zoom", "params": params }),
            );
            assert_eq!(
                s.image(&id).document().layers()[0].pixels.data(),
                bus.document.layers()[0].pixels.data(),
                "{params} {select}"
            );
        }
    }
}
