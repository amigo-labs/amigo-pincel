//! Unit tests of the wire format (`EffectSpec::from_params` / `run`).

use super::*;

fn solid(w: u32, h: u32, px: [u8; 4]) -> EffectImage {
    let data = px.repeat((w * h) as usize);
    EffectImage::from_rgba8(w, h, data).unwrap()
}

#[test]
fn from_params_every_name_parses_with_its_documented_arity() {
    let cases: &[(&str, &[f64])] = &[
        ("gaussian_blur", &[2.0]),
        ("box_blur", &[3.0, 3.0]),
        ("motion_blur", &[5.0, 45.0]),
        ("radial_blur", &[10.0, 0.5, 0.5, 1.0]),
        ("sharpen", &[]),
        ("unsharp_mask", &[50.0, 1.0, 0.0]),
        ("emboss", &[45.0, 45.0, 1.0]),
        ("edge_detect", &[2.0, 100.0]),
        ("relief", &[45.0, 1.0]),
        ("add_noise", &[10.0, 1.0, 1.0, 7.0]),
        ("reduce_noise", &[1.0]),
        ("brightness_contrast", &[10.0, 10.0, 1.0]),
        ("hue_saturation", &[10.0, 0.0, 0.0, 0.0]),
        ("curves", &[0.0, 0.0, 0.0, 255.0, 255.0]),
        ("levels", &[0.0, 0.0, 255.0, 1.0, 0.0, 255.0]),
        (
            "color_balance",
            &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
        ),
        ("invert", &[]),
        ("grayscale", &[0.0]),
        ("grayscale", &[3.0, 0.5, 0.25, 0.25]),
        ("posterize", &[4.0]),
        ("threshold", &[128.0]),
        ("auto_level", &[]),
        ("exposure", &[1.0]),
        ("highlights_shadows", &[20.0, -20.0]),
        ("invert_alpha", &[]),
        ("sepia", &[100.0]),
        ("temperature_tint", &[30.0, -30.0]),
        ("motion_blur", &[5.0, 45.0, 0.0]),
        ("bokeh", &[10.0, 25.0]),
        ("fragment", &[4.0, 8.0, 0.0]),
        ("median", &[10.0, 50.0]),
        ("sketch_blur", &[10.0, 1.0]),
        ("surface_blur", &[6.0, 25.0]),
        ("zoom_blur", &[10.0, 0.5, 0.5]),
        ("outline", &[0.0, 1.0, 1.0, 255.0]),
        ("bulge", &[45.0, 0.5, 0.5, 0.0]),
        ("crystalize", &[8.0, 1.0]),
        ("dents", &[25.0, 50.0, 10.0, 1.0, 0.0]),
        ("frosted_glass", &[10.0, 1.0]),
        ("morphology", &[0.0, 0.0, 1.0]),
        ("pixelate", &[8.0, 1.0]),
        ("polar_inversion", &[1.0, 0.5, 0.5, 1.0]),
        ("tile_reflection", &[40.0, 30.0, 8.0, 0.0]),
        ("twist", &[30.0, 100.0, 0.5, 0.5, 0.0]),
        ("reduce_noise", &[6.0, 40.0]),
        ("drop_shadow", &[4.0, 4.0, 4.0, 255.0, 50.0]),
        ("stylize_outline", &[3.0, 50.0]),
        ("quantize", &[16.0, 0.0, 0.0]),
        ("clouds", &[250.0, 50.0, 1.0, 255.0, 4294967295.0, 0.0]),
        ("turbulence", &[200.0, 5.0, 1.0, 255.0, 4294967295.0, 0.0]),
        ("julia_fractal", &[4.0, 1.0, 0.0, 2.0]),
        ("mandelbrot_fractal", &[1.0, 10.0, 0.0, 2.0, 0.0]),
        ("rotate_zoom", &[30.0, 10.0, -10.0, 1.5, 2.0]),
    ];
    for (name, params) in cases {
        let spec = EffectSpec::from_params(name, params).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(
            EFFECT_NAMES.contains(name),
            "{name} missing from EFFECT_NAMES"
        );
        assert!(!spec.label().is_empty());
        // Every spec must run on a small image without panicking.
        let out = spec.run(&solid(4, 4, [200, 100, 50, 255]));
        assert_eq!((out.width(), out.height()), (4, 4));
    }
}

#[test]
fn from_params_unknown_name_is_an_error() {
    assert_eq!(
        EffectSpec::from_params("no_such_effect", &[]),
        Err(EffectError::UnknownEffect("no_such_effect".into()))
    );
}

#[test]
fn from_params_wrong_arity_is_an_error() {
    assert!(matches!(
        EffectSpec::from_params("gaussian_blur", &[]),
        Err(EffectError::ParamCount {
            expected: 1,
            got: 0,
            ..
        })
    ));
    assert!(matches!(
        EffectSpec::from_params("curves", &[0.0, 1.0]),
        Err(EffectError::ParamCount { .. })
    ));
}

#[test]
fn run_invert_flips_rgb_and_keeps_alpha() {
    let spec = EffectSpec::from_params("invert", &[]).unwrap();
    let out = spec.run(&solid(1, 1, [10, 20, 30, 200]));
    assert_eq!(out.data(), &[245, 235, 225, 200]);
}

#[test]
fn every_effect_name_is_accepted() {
    for name in EFFECT_NAMES {
        if let Err(err) = EffectSpec::from_params(name, &[]) {
            assert!(
                !matches!(err, EffectError::UnknownEffect(_)),
                "{name} rejected as unknown"
            );
        }
    }
}

/// A 9x9 black / white checkerboard, so any resampling shows.
fn checker9() -> EffectImage {
    let mut data = Vec::with_capacity(9 * 9 * 4);
    for y in 0..9u8 {
        for x in 0..9u8 {
            let v = if (x + y) % 2 == 0 { 255 } else { 0 };
            data.extend_from_slice(&[v, v, v, 255]);
        }
    }
    EffectImage::from_rgba8(9, 9, data).unwrap()
}

fn px(img: &EffectImage, x: usize, y: usize) -> [u8; 4] {
    let o = (y * img.width() as usize + x) * 4;
    let d = img.data();
    [d[o], d[o + 1], d[o + 2], d[o + 3]]
}

#[test]
fn radial_blur_center_is_a_fraction_of_the_image() {
    // The catalog and the agent tools send the centre as 0..1 of the
    // image; 0.5 / 0.5 must spin around the middle pixel, not (0.5, 0.5) px.
    let src = checker9();
    let spec = EffectSpec::from_params("radial_blur", &[40.0, 0.5, 0.5, 0.0]).unwrap();
    let out = spec.run(&src);
    assert_eq!(px(&out, 4, 4), px(&src, 4, 4), "centre pixel must stay");
    assert_ne!(px(&out, 0, 4), px(&src, 0, 4), "edge pixel must spin");
}

/// image-transform-handles-rotate-zoom: the wire slots, the clamped ranges,
/// the quality index and the history name.
#[test]
fn from_params_rotate_zoom_reads_angle_pans_zoom_and_quality() {
    use crate::transform::{Quality, RotateZoom};
    let spec = EffectSpec::from_params("rotate_zoom", &[90.0, 12.5, -25.0, 2.0, 0.0]).unwrap();
    assert_eq!(
        spec,
        EffectSpec::RotateZoom(RotateZoom::new(90.0, 12.5, -25.0, 2.0, Quality::Nearest))
    );
    assert_eq!(spec.label(), "Rotate / Zoom");
    let wide = EffectSpec::from_params("rotate_zoom", &[999.0, 500.0, -500.0, 0.0, 7.0]).unwrap();
    assert_eq!(
        wide,
        EffectSpec::RotateZoom(RotateZoom::new(180.0, 100.0, -100.0, 0.1, Quality::Bicubic))
    );
    let bilinear = EffectSpec::from_params("rotate_zoom", &[0.0, 0.0, 0.0, 1.0, 1.0]).unwrap();
    assert!(matches!(bilinear, EffectSpec::RotateZoom(r) if r.quality == Quality::Bilinear));
    assert!(EffectSpec::from_params("rotate_zoom", &[0.0, 0.0, 0.0, 1.0]).is_err());
    // A quarter turn of the checkerboard about the middle pixel keeps it.
    let src = checker9();
    let out = EffectSpec::from_params("rotate_zoom", &[90.0, 0.0, 0.0, 1.0, 0.0])
        .unwrap()
        .run(&src);
    assert_eq!(out, src);
}
