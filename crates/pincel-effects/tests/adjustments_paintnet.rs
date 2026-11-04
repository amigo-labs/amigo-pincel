//! docs/specs/adjustments-paintnet.md: the six paint.net adjustments and
//! the Black and White rename, through the wire format both cores use.

use pincel_effects::{EFFECT_NAMES, EffectError, EffectImage, EffectSpec};

const NEW: [&str; 6] = [
    "auto_level",
    "exposure",
    "highlights_shadows",
    "invert_alpha",
    "sepia",
    "temperature_tint",
];

fn spec(name: &str, params: &[f64]) -> EffectSpec {
    EffectSpec::from_params(name, params).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn image(pixels: &[[u8; 4]]) -> EffectImage {
    let data = pixels.iter().flatten().copied().collect();
    EffectImage::from_rgba8(pixels.len() as u32, 1, data).expect("one row")
}

fn pixels(img: &EffectImage) -> Vec<[u8; 4]> {
    img.data().as_chunks::<4>().0.to_vec()
}

fn run(name: &str, params: &[f64], src: &[[u8; 4]]) -> Vec<[u8; 4]> {
    pixels(&spec(name, params).run(&image(src)))
}

/// 2048 pixels covering every byte value in every channel, alpha included.
fn varied() -> Vec<[u8; 4]> {
    (0..2048u32)
        .map(|i| {
            [
                (i % 256) as u8,
                ((i * 37 + 11) % 256) as u8,
                ((i * 91 + 3) % 256) as u8,
                ((i * 13 + 7) % 256) as u8,
            ]
        })
        .collect()
}

/// Luma (0.2126 R + 0.7152 G + 0.0722 B) of a pixel, 0–255.
fn luma(p: [u8; 4]) -> f64 {
    0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2])
}

/// Every parameter vector of the new adjustments at their defaults.
fn defaults(name: &str) -> &'static [f64] {
    match name {
        "auto_level" | "invert_alpha" => &[],
        "exposure" => &[0.0],
        "highlights_shadows" | "temperature_tint" => &[0.0, 0.0],
        "sepia" => &[100.0],
        other => panic!("not a new adjustment: {other}"),
    }
}

const MID: [u8; 4] = [128, 128, 128, 255];

#[test]
fn new_adjustments_are_known_wire_names() {
    for name in NEW {
        assert!(EFFECT_NAMES.contains(&name), "{name} missing");
        let s = spec(name, defaults(name));
        assert!(!s.label().is_empty());
    }
}

#[test]
fn grayscale_is_labelled_black_and_white() {
    assert_eq!(spec("grayscale", &[0.0]).label(), "Black and White");
}

// AC-03
#[test]
fn neutral_settings_leave_every_pixel_unchanged() {
    let src = varied();
    for (name, neutral) in [
        ("exposure", &[0.0][..]),
        ("highlights_shadows", &[0.0, 0.0][..]),
        ("sepia", &[0.0][..]),
        ("temperature_tint", &[0.0, 0.0][..]),
    ] {
        assert_eq!(run(name, neutral, &src), src, "{name}");
    }
}

// AC-04
#[test]
fn every_adjustment_of_the_spec_except_invert_alpha_keeps_alpha() {
    let src = varied();
    for (name, params) in [
        ("auto_level", &[][..]),
        ("exposure", &[2.5][..]),
        ("exposure", &[-3.0][..]),
        ("highlights_shadows", &[70.0, -40.0][..]),
        ("highlights_shadows", &[-60.0, 80.0][..]),
        ("sepia", &[100.0][..]),
        ("sepia", &[35.0][..]),
        ("temperature_tint", &[100.0, -100.0][..]),
        ("temperature_tint", &[-50.0, 60.0][..]),
        // Black and White (renamed Grayscale) is an adjustment of this spec too.
        ("grayscale", &[0.0][..]),
        ("grayscale", &[1.0][..]),
        ("grayscale", &[2.0][..]),
        ("grayscale", &[3.0, 0.5, 0.25, 0.25][..]),
    ] {
        let out = run(name, params, &src);
        for (o, s) in out.iter().zip(&src) {
            assert_eq!(o[3], s[3], "{name} {params:?} changed alpha");
        }
    }
}

// AC-04
#[test]
fn invert_alpha_flips_alpha_keeps_colour_and_twice_restores() {
    let src = varied();
    let once = run("invert_alpha", &[], &src);
    for (o, s) in once.iter().zip(&src) {
        assert_eq!(&o[..3], &s[..3]);
        assert_eq!(o[3], 255 - s[3]);
    }
    let twice = pixels(&spec("invert_alpha", &[]).run(&image(&once)));
    assert_eq!(twice, src);
    // A fully transparent pixel becomes opaque with its stored colour.
    assert_eq!(run("invert_alpha", &[], &[[0, 0, 0, 0]]), [[0, 0, 0, 255]]);
}

// AC-05
#[test]
fn auto_level_stretches_a_gray_target_to_the_full_range() {
    let src = [
        [60, 60, 60, 255],
        [100, 100, 100, 255],
        [180, 180, 180, 128],
        [90, 90, 90, 255],
    ];
    let out = run("auto_level", &[], &src);
    let values: Vec<u8> = out.iter().map(|p| p[0]).collect();
    assert!(out.iter().all(|p| p[0] == p[1] && p[1] == p[2]));
    assert!(values[0] <= 1, "darkest {}", values[0]);
    assert!(values[2] >= 254, "brightest {}", values[2]);
    assert!(values[0] < values[3] && values[3] < values[1] && values[1] < values[2]);
}

// AC-05
#[test]
fn auto_level_ignores_transparent_pixels_for_its_range() {
    // The transparent pixels are darker / brighter than every opaque one,
    // but only opaque pixels set L0 and L1.
    let src = [
        [0, 0, 0, 0],
        [80, 80, 80, 255],
        [160, 160, 160, 255],
        [255, 255, 255, 0],
    ];
    let out = run("auto_level", &[], &src);
    assert!(out[1][0] <= 1 && out[2][0] >= 254, "{out:?}");
}

// AC-05
#[test]
fn auto_level_without_a_luma_range_changes_nothing() {
    // One colour.
    let one = [[90, 30, 200, 255]; 4];
    assert_eq!(run("auto_level", &[], &one), one);
    // Several colours with one luma (2126 R + 7152 G + 722 B equal).
    let same_luma = [[40, 40, 161, 255], [71, 43, 40, 255]];
    assert_eq!(luma(same_luma[0]), luma(same_luma[1]));
    assert_eq!(run("auto_level", &[], &same_luma), same_luma);
    // No opaque pixel.
    let clear = [[10, 20, 30, 0], [200, 100, 50, 0]];
    assert_eq!(run("auto_level", &[], &clear), clear);
}

// AC-05, AC-10: the range comes from the target only.
#[test]
fn auto_level_takes_its_range_from_the_target_pixels() {
    let src = image(&[
        [0, 0, 0, 255],
        [80, 80, 80, 255],
        [160, 160, 160, 255],
        [255, 255, 255, 255],
    ]);
    let s = spec("auto_level", &[]);
    // Target = the middle two pixels: 80 → 0, 160 → 255.
    let out = pixels(&s.run_in(&src, |x, _| x == 1 || x == 2));
    assert!(out[1][0] <= 1 && out[2][0] >= 254, "{out:?}");
    // A target with only transparent pixels leaves the image unchanged.
    let clear = image(&[[0, 0, 0, 0], [80, 80, 80, 255], [160, 160, 160, 255]]);
    assert_eq!(s.run_in(&clear, |x, _| x == 0), clear);
}

// AC-06
#[test]
fn exposure_scales_mid_gray_in_linear_light() {
    let up = run("exposure", &[1.0], &[MID])[0];
    let down = run("exposure", &[-1.0], &[MID])[0];
    assert!(up[0] == up[1] && up[1] == up[2] && up[0] > 128, "{up:?}");
    assert!(
        down[0] == down[1] && down[1] == down[2] && down[0] < 128,
        "{down:?}"
    );
    assert_eq!(run("exposure", &[5.0], &[MID]), [[255, 255, 255, 255]]);
    // Pure black stays black.
    assert_eq!(run("exposure", &[5.0], &[[0, 0, 0, 255]]), [[0, 0, 0, 255]]);
}

// AC-07
#[test]
fn highlights_shadows_act_on_their_tonal_range() {
    let dark = [40, 40, 40, 255];
    let bright = [215, 215, 215, 255];
    let gain =
        |params: &[f64], p: [u8; 4]| luma(run("highlights_shadows", params, &[p])[0]) - luma(p);
    let (d, b) = (gain(&[100.0, 0.0], dark), gain(&[100.0, 0.0], bright));
    assert!(d > b && d > 0.0, "shadows: dark {d}, bright {b}");
    let (d, b) = (gain(&[0.0, 100.0], dark), gain(&[0.0, 100.0], bright));
    assert!(-b > -d && b < 0.0, "highlights: dark {d}, bright {b}");
}

// AC-08
#[test]
fn sepia_at_full_intensity_is_a_function_of_luma() {
    let src = varied();
    let out = run("sepia", &[100.0], &src);
    for p in &out {
        assert!(p[0] >= p[1] && p[1] >= p[2], "{p:?}");
    }
    let pair = run("sepia", &[100.0], &[[40, 40, 161, 255], [71, 43, 40, 255]]);
    assert_eq!(pair[0], pair[1]);
    assert_eq!(
        run("sepia", &[100.0], &[[255, 255, 255, 255]]),
        [[255, 219, 171, 255]]
    );
}

// AC-09
#[test]
fn temperature_and_tint_shift_mid_gray() {
    let p = |t: f64, n: f64| run("temperature_tint", &[t, n], &[MID])[0];
    let warm = p(100.0, 0.0);
    assert!(warm[0] > 128 && 128 > warm[2], "{warm:?}");
    let cool = p(-100.0, 0.0);
    assert!(cool[2] > 128 && 128 > cool[0], "{cool:?}");
    let magenta = p(0.0, 100.0);
    assert!(
        magenta[1] < magenta[0] && magenta[1] < magenta[2],
        "{magenta:?}"
    );
    let green = p(0.0, -100.0);
    assert!(green[1] > green[0] && green[1] > green[2], "{green:?}");
}

// AC-11
#[test]
fn wrong_parameter_count_is_rejected() {
    for name in NEW {
        let n = defaults(name).len();
        let wrong = vec![0.0; n + 1];
        assert!(
            matches!(
                EffectSpec::from_params(name, &wrong),
                Err(EffectError::ParamCount { expected, got, .. }) if expected == n && got == n + 1
            ),
            "{name}"
        );
    }
}

// AC-11
#[test]
fn non_finite_numbers_read_as_zero_then_clamp_to_the_range() {
    let same = |name: &str, a: &[f64], b: &[f64]| {
        assert_eq!(spec(name, a), spec(name, b), "{name} {a:?} vs {b:?}");
    };
    same("exposure", &[f64::NAN], &[0.0]);
    same("exposure", &[f64::INFINITY], &[0.0]);
    same("exposure", &[99.0], &[5.0]);
    same("exposure", &[-99.0], &[-5.0]);
    same("highlights_shadows", &[f64::NAN, 500.0], &[0.0, 100.0]);
    same(
        "highlights_shadows",
        &[-500.0, f64::NEG_INFINITY],
        &[-100.0, 0.0],
    );
    same("sepia", &[f64::NAN], &[0.0]);
    same("sepia", &[250.0], &[100.0]);
    same("sepia", &[-5.0], &[0.0]);
    same("temperature_tint", &[300.0, -300.0], &[100.0, -100.0]);
    same("temperature_tint", &[f64::NAN, f64::INFINITY], &[0.0, 0.0]);
}
