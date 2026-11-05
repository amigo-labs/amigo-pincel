//! docs/specs/effects-artistic-photo.md: paint.net's Artistic effects (Ink
//! Sketch, Oil Painting, Pencil Sketch) and Photo effects (Glow, Red Eye
//! Removal, Soften Portrait, Straighten, Vignette) through the wire format
//! both cores use.

use std::collections::HashSet;

use pincel_effects::{EFFECT_NAMES, EffectError, EffectImage, EffectSpec};

const NAMES: [&str; 8] = [
    "ink_sketch",
    "oil_painting",
    "pencil_sketch",
    "glow",
    "red_eye_removal",
    "soften_portrait",
    "straighten",
    "vignette",
];

/// Straighten's Fill on the wire.
const ZOOM: f64 = 0.0;
const CORNERS: f64 = 1.0;

fn spec(name: &str, params: &[f64]) -> EffectSpec {
    EffectSpec::from_params(name, params).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn image(w: u32, h: u32, pixels: &[[u8; 4]]) -> EffectImage {
    let data = pixels.iter().flatten().copied().collect();
    EffectImage::from_rgba8(w, h, data).expect("w * h pixels")
}

fn pixels(img: &EffectImage) -> Vec<[u8; 4]> {
    img.data().as_chunks::<4>().0.to_vec()
}

fn run(name: &str, params: &[f64], src: &EffectImage) -> EffectImage {
    spec(name, params).run(src)
}

fn at(img: &EffectImage, x: u32, y: u32) -> [u8; 4] {
    pixels(img)[(y * img.width() + x) as usize]
}

/// BT.709 luma of a pixel, 0–255.
fn luma(p: [u8; 4]) -> f32 {
    0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])
}

/// The table's defaults, in wire order.
fn defaults(name: &str) -> &'static [f64] {
    match name {
        "ink_sketch" => &[50.0, 50.0],
        "oil_painting" => &[3.0, 50.0],
        "pencil_sketch" => &[2.0, 0.0],
        "glow" => &[6.0, 10.0, 10.0],
        "red_eye_removal" => &[70.0, 90.0],
        "soften_portrait" => &[5.0, 0.0, 10.0],
        "straighten" => &[0.0, ZOOM],
        "vignette" => &[0.5, 0.5, 100.0, 100.0, 0.0],
        other => panic!("not an artistic or photo effect: {other}"),
    }
}

/// Parameter vectors per effect: defaults, both ends of every range and a
/// few values in between.
fn settings(name: &str) -> Vec<Vec<f64>> {
    let mut out = vec![defaults(name).to_vec()];
    out.extend(match name {
        "ink_sketch" => vec![vec![0.0, 0.0], vec![99.0, 100.0], vec![20.0, 80.0]],
        "oil_painting" => vec![vec![1.0, 3.0], vec![8.0, 255.0], vec![2.0, 16.0]],
        "pencil_sketch" => vec![vec![1.0, -20.0], vec![20.0, 20.0], vec![5.0, 3.0]],
        "glow" => vec![
            vec![1.0, -100.0, -100.0],
            vec![20.0, 100.0, 100.0],
            vec![3.0, 40.0, -30.0],
        ],
        "red_eye_removal" => vec![vec![0.0, 0.0], vec![100.0, 100.0], vec![40.0, 60.0]],
        "soften_portrait" => vec![
            vec![0.0, 0.0, 0.0],
            vec![10.0, 20.0, 20.0],
            vec![3.0, -20.0, 5.0],
        ],
        "straighten" => vec![
            vec![-45.0, ZOOM],
            vec![45.0, CORNERS],
            vec![12.5, CORNERS],
            vec![-3.3, ZOOM],
        ],
        "vignette" => vec![
            vec![0.0, 0.0, 10.0, 0.0, 0.0],
            vec![1.0, 1.0, 400.0, 100.0, 1.0],
            vec![0.3, 0.7, 50.0, 60.0, 1.0],
        ],
        _ => vec![],
    });
    out
}

/// A seeded pseudo-random image with varied alpha (some fully transparent
/// pixels), so every effect has edges, colours and transparency to handle.
fn noisy(w: u32, h: u32, seed: u64) -> EffectImage {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 24) as u8
    };
    let px: Vec<[u8; 4]> = (0..w * h)
        .map(|i| {
            let a = match i % 5 {
                0 => 0,
                1 => 128,
                _ => 255,
            };
            [next(), next(), next(), a]
        })
        .collect();
    image(w, h, &px)
}

fn solid(w: u32, h: u32, c: [u8; 4]) -> EffectImage {
    image(w, h, &vec![c; (w * h) as usize])
}

#[test]
fn every_new_name_is_on_the_wire() {
    for name in NAMES {
        assert!(EFFECT_NAMES.contains(&name), "{name} missing");
        spec(name, defaults(name));
    }
}

// AC-03
#[test]
fn every_effect_keeps_size_and_alpha() {
    let src = noisy(13, 9, 7);
    for name in NAMES.into_iter().filter(|&n| n != "straighten") {
        for params in settings(name) {
            let out = run(name, &params, &src);
            assert_eq!((out.width(), out.height()), (13, 9), "{name} {params:?}");
            for (a, b) in pixels(&src).iter().zip(pixels(&out)) {
                assert_eq!(a[3], b[3], "{name} {params:?} alpha");
            }
        }
    }
}

// AC-05
#[test]
fn pencil_sketch_is_gray_and_whitens_a_flat_area() {
    let src = noisy(12, 10, 3);
    for params in settings("pencil_sketch") {
        for p in pixels(&run("pencil_sketch", &params, &src)) {
            let (lo, hi) = (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]));
            assert!(hi - lo <= 1, "{params:?}: {p:?} is not gray");
        }
    }
    for colour in [
        [0, 0, 0, 255],
        [255, 255, 255, 255],
        [200, 30, 90, 255],
        [3, 2, 1, 255],
        [10, 200, 250, 128],
    ] {
        for tip in [1.0, 2.0, 20.0] {
            let out = run("pencil_sketch", &[tip, 0.0], &solid(9, 7, colour));
            for p in pixels(&out) {
                assert!(luma(p) >= 250.0, "{colour:?} tip {tip}: {p:?}");
            }
        }
    }
}

/// Two flat areas side by side: columns `0..w / 2` in `left`, the rest in
/// `right`.
fn halves(w: u32, h: u32, left: [u8; 4], right: [u8; 4]) -> EffectImage {
    let px: Vec<[u8; 4]> = (0..w * h)
        .map(|i| if i % w < w / 2 { left } else { right })
        .collect();
    image(w, h, &px)
}

// AC-06
#[test]
fn ink_sketch_draws_dark_lines_on_edges() {
    for (left, right) in [
        ([255, 255, 255, 255], [128, 128, 128, 255]),
        ([220, 180, 40, 255], [40, 60, 160, 255]),
        ([60, 60, 60, 255], [230, 230, 230, 255]),
    ] {
        let src = halves(20, 8, left, right);
        let out = run("ink_sketch", defaults("ink_sketch"), &src);
        let deep = luma(at(&out, 2, 4)).min(luma(at(&out, 17, 4)));
        for x in [9, 10] {
            assert!(
                luma(at(&out, x, 4)) < deep,
                "{left:?} | {right:?}: boundary x {x} {:?} not darker than {deep}",
                at(&out, x, 4)
            );
        }
    }
    let src = noisy(11, 9, 5);
    for ink in [0.0, 50.0, 99.0] {
        for p in pixels(&run("ink_sketch", &[ink, 0.0], &src)) {
            let (lo, hi) = (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]));
            assert!(hi - lo <= 1, "ink {ink}: {p:?} is not gray");
        }
    }
}

// AC-07
#[test]
fn oil_painting_keeps_a_solid_layer_and_merges_a_ramp() {
    for colour in [[90, 40, 200, 255], [0, 0, 0, 255], [255, 255, 255, 77]] {
        let src = solid(10, 6, colour);
        for params in settings("oil_painting") {
            assert_eq!(
                run("oil_painting", &params, &src),
                src,
                "{colour:?} {params:?}"
            );
        }
    }
    let ramp = image(
        256,
        1,
        &(0..=255u8).map(|v| [v, v, v, 255]).collect::<Vec<_>>(),
    );
    let distinct = |img: &EffectImage| pixels(img).into_iter().collect::<HashSet<_>>().len();
    let out = run("oil_painting", &[3.0, 8.0], &ramp);
    assert_eq!(distinct(&ramp), 256);
    assert!(distinct(&out) < 256, "{} distinct colours", distinct(&out));
}

#[test]
fn oil_painting_leaves_a_transparent_layer_unchanged() {
    let src = image(6, 4, &[[30, 60, 90, 0]; 24]);
    for params in settings("oil_painting") {
        assert_eq!(run("oil_painting", &params, &src), src, "{params:?}");
    }
}

// AC-12
#[test]
fn wrong_parameter_count_is_rejected() {
    for name in NAMES {
        let n = defaults(name).len();
        for got in [0, n - 1, n + 1] {
            assert!(
                matches!(
                    EffectSpec::from_params(name, &vec![1.0; got]),
                    Err(EffectError::ParamCount { expected, got: g, .. }) if expected == n && g == got
                ),
                "{name} with {got}"
            );
        }
    }
}

// AC-12
#[test]
fn non_finite_numbers_read_as_zero_then_clamp_to_the_range() {
    let same = |name: &str, a: &[f64], b: &[f64]| {
        assert_eq!(spec(name, a), spec(name, b), "{name} {a:?} vs {b:?}");
    };
    let nan = f64::NAN;
    let inf = f64::INFINITY;
    same("ink_sketch", &[nan, inf], &[0.0, 0.0]);
    same("ink_sketch", &[500.0, -5.0], &[99.0, 0.0]);
    same("oil_painting", &[nan, nan], &[1.0, 3.0]);
    same("oil_painting", &[99.0, 999.0], &[8.0, 255.0]);
    same("oil_painting", &[2.4, 7.6], &[2.0, 8.0]);
    same("pencil_sketch", &[nan, -inf], &[1.0, 0.0]);
    same("pencil_sketch", &[50.0, -50.0], &[20.0, -20.0]);
    same("pencil_sketch", &[0.0, 30.0], &[1.0, 20.0]);
}

// AC-03
#[test]
fn straighten_keeps_the_size_and_rotates_the_alpha() {
    // A 15 x 9 opaque bar on transparency: rotating it moves alpha too.
    let px: Vec<[u8; 4]> = (0..21 * 21u32)
        .map(|i| {
            let (x, y) = (i % 21, i / 21);
            if (3..18).contains(&x) && (6..15).contains(&y) {
                [200, 80, 40, 255]
            } else {
                [0, 0, 0, 0]
            }
        })
        .collect();
    let src = image(21, 21, &px);
    for params in settings("straighten") {
        let out = run("straighten", &params, &src);
        assert_eq!((out.width(), out.height()), (21, 21), "{params:?}");
    }
    let out = run("straighten", &[30.0, CORNERS], &src);
    assert_ne!(
        pixels(&out).iter().map(|p| p[3]).collect::<Vec<_>>(),
        px.iter().map(|p| p[3]).collect::<Vec<_>>(),
        "the alpha rotates with the image"
    );
}

// AC-04
#[test]
fn neutral_settings_leave_the_layer_unchanged() {
    let src = noisy(14, 11, 9);
    let neutral: &[(&str, &[f64])] = &[
        ("soften_portrait", &[0.0, 0.0, 0.0]),
        ("straighten", &[0.0, ZOOM]),
        ("straighten", &[0.0, CORNERS]),
        ("vignette", &[0.5, 0.5, 100.0, 0.0, 0.0]),
        ("vignette", &[0.1, 0.9, 10.0, 0.0, 1.0]),
        ("red_eye_removal", &[70.0, 0.0]),
        ("red_eye_removal", &[100.0, 0.0]),
        ("red_eye_removal", &[0.0, 0.0]),
    ];
    for (name, params) in neutral {
        assert_eq!(run(name, params, &src), src, "{name} {params:?}");
    }
}

// AC-08
#[test]
fn red_eye_removal_lowers_dominant_red_only() {
    let src = image(2, 1, &[[255, 30, 30, 255], [100, 120, 110, 255]]);
    let out = run("red_eye_removal", defaults("red_eye_removal"), &src);
    let red = at(&out, 0, 0);
    assert!(red[0] < 255, "red {red:?}");
    assert_eq!(&red[1..], &[30, 30, 255]);
    assert_eq!(at(&out, 1, 0), [100, 120, 110, 255]);
}

// AC-09
#[test]
fn vignette_darkens_or_lightens_toward_the_edges() {
    let colour = [128, 100, 200, 255];
    let src = solid(33, 25, colour);
    let corners = [(0, 0), (32, 0), (0, 24), (32, 24)];
    let black = run("vignette", &[0.5, 0.5, 50.0, 100.0, 0.0], &src);
    assert_eq!(at(&black, 16, 12), colour);
    for (x, y) in corners {
        assert!(luma(at(&black, x, y)) < luma(colour), "black ({x}, {y})");
    }
    let white = run("vignette", &[0.5, 0.5, 50.0, 100.0, 1.0], &src);
    assert_eq!(at(&white, 16, 12), colour);
    for (x, y) in corners {
        assert!(luma(at(&white, x, y)) > luma(colour), "white ({x}, {y})");
    }
}

#[test]
fn vignette_centred_on_a_corner_darkens_around_it() {
    let colour = [90, 90, 90, 255];
    let src = solid(30, 20, colour);
    let out = run("vignette", &[0.0, 0.0, 50.0, 100.0, 0.0], &src);
    assert_eq!(at(&out, 0, 0), colour);
    assert!(luma(at(&out, 29, 19)) < luma(colour));
}

/// A fully opaque layer with varied colours.
fn opaque(w: u32, h: u32, seed: u64) -> EffectImage {
    let px: Vec<[u8; 4]> = pixels(&noisy(w, h, seed))
        .into_iter()
        .map(|[r, g, b, _]| [r, g, b, 255])
        .collect();
    image(w, h, &px)
}

const ANGLES: [f64; 8] = [-45.0, -10.0, -1.0, -0.1, 0.1, 1.0, 10.0, 45.0];

// AC-10
#[test]
fn straighten_zoom_to_fill_keeps_every_pixel_opaque() {
    for (w, h) in [(40, 30), (9, 31), (64, 8)] {
        let src = opaque(w, h, 4);
        for angle in ANGLES {
            let out = run("straighten", &[angle, ZOOM], &src);
            assert!(
                pixels(&out).iter().all(|p| p[3] == 255),
                "{w} x {h} at {angle}"
            );
        }
    }
}

// AC-10
#[test]
fn straighten_transparent_corners_clears_the_corners() {
    // Large enough that even 0.1 degrees uncovers each corner pixel.
    let (w, h) = (800, 800);
    let src = solid(w, h, [70, 140, 210, 255]);
    for angle in ANGLES {
        let out = run("straighten", &[angle, CORNERS], &src);
        for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
            assert_eq!(at(&out, x, y)[3], 0, "({x}, {y}) at {angle}");
        }
        assert_eq!(at(&out, w / 2, h / 2)[3], 255, "centre at {angle}");
    }
}

// AC-11
#[test]
fn glow_never_darkens() {
    let gray = solid(12, 12, [128, 128, 128, 255]);
    let src = noisy(12, 12, 2);
    for radius in [1.0, 6.0, 20.0] {
        for brightness in [0.0, 10.0, 50.0, 100.0] {
            let params = [radius, brightness, 0.0];
            for p in pixels(&run("glow", &params, &gray)) {
                assert!(p[..3].iter().all(|&c| c >= 128), "{params:?}: {p:?}");
            }
            for (a, b) in pixels(&src).iter().zip(pixels(&run("glow", &params, &src))) {
                assert!((0..3).all(|c| b[c] >= a[c]), "{params:?}: {a:?} -> {b:?}");
            }
        }
    }
    let lit = run("glow", &[6.0, 10.0, 0.0], &gray);
    assert!(at(&lit, 6, 6)[0] > 128, "a positive brightness lightens");
}

// AC-12
#[test]
fn photo_numbers_read_non_finite_as_zero_then_clamp() {
    let same = |name: &str, a: &[f64], b: &[f64]| {
        assert_eq!(spec(name, a), spec(name, b), "{name} {a:?} vs {b:?}");
    };
    let nan = f64::NAN;
    let inf = f64::INFINITY;
    same("glow", &[nan, nan, inf], &[1.0, 0.0, 0.0]);
    same("glow", &[99.0, 500.0, -500.0], &[20.0, 100.0, -100.0]);
    same("red_eye_removal", &[nan, inf], &[0.0, 0.0]);
    same("red_eye_removal", &[200.0, -5.0], &[100.0, 0.0]);
    same("soften_portrait", &[nan, nan, nan], &[0.0, 0.0, 0.0]);
    same("soften_portrait", &[99.0, 99.0, 99.0], &[10.0, 20.0, 20.0]);
    same("soften_portrait", &[-5.0, -99.0, -1.0], &[0.0, -20.0, 0.0]);
    same("straighten", &[nan, nan], &[0.0, ZOOM]);
    same("straighten", &[90.0, 7.0], &[45.0, CORNERS]);
    same("straighten", &[-90.0, -1.0], &[-45.0, ZOOM]);
    same("vignette", &[nan; 5], &[0.0, 0.0, 10.0, 0.0, 0.0]);
    same(
        "vignette",
        &[2.0, -1.0, 999.0, 500.0, 9.0],
        &[1.0, 0.0, 400.0, 100.0, 1.0],
    );
}
