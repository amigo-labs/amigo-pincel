//! docs/specs/effects-blurs.md: paint.net's remaining blurs (Bokeh,
//! Fragment, Median, Sketch Blur, Surface Blur, Zoom Blur), Motion Blur's
//! Centered switch and Radial Blur's wire compatibility, through the wire
//! format both cores use.

use pincel_effects::{EFFECT_NAMES, EffectError, EffectImage, EffectSpec};

const NEW: [&str; 6] = [
    "bokeh",
    "fragment",
    "median",
    "sketch_blur",
    "surface_blur",
    "zoom_blur",
];

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

/// The table's defaults, in wire order.
fn defaults(name: &str) -> &'static [f64] {
    match name {
        "bokeh" => &[10.0, 25.0],
        "fragment" => &[4.0, 8.0, 0.0],
        "median" => &[10.0, 50.0],
        "sketch_blur" => &[10.0, 1.0],
        "surface_blur" => &[6.0, 25.0],
        "zoom_blur" => &[10.0, 0.5, 0.5],
        other => panic!("not a new blur: {other}"),
    }
}

/// Parameter vectors per new blur: defaults, both ends of every range and
/// a few values in between.
fn settings(name: &str) -> Vec<Vec<f64>> {
    let mut out = vec![defaults(name).to_vec()];
    out.extend(match name {
        "bokeh" => vec![
            vec![1.0, 0.0],
            vec![100.0, 100.0],
            vec![3.0, 100.0],
            vec![7.0, 60.0],
        ],
        "fragment" => vec![
            vec![2.0, 0.0, 0.0],
            vec![50.0, 100.0, 360.0],
            vec![3.0, 2.5, 45.0],
            vec![7.0, 13.0, 300.0],
        ],
        "median" => vec![
            vec![1.0, 0.0],
            vec![200.0, 100.0],
            vec![1.0, 50.0],
            vec![3.0, 25.0],
        ],
        "sketch_blur" => vec![vec![1.0, 0.0], vec![100.0, 9999.0], vec![4.0, 77.0]],
        "surface_blur" => vec![vec![1.0, 1.0], vec![100.0, 100.0], vec![3.0, 50.0]],
        "zoom_blur" => vec![
            vec![1.0, 0.0, 0.0],
            vec![100.0, 1.0, 1.0],
            vec![60.0, 0.25, 0.8],
        ],
        other => panic!("not a new blur: {other}"),
    });
    out
}

/// A 13x9 layer of varied colours, alpha between 40 and 230.
fn varied() -> EffectImage {
    let (w, h) = (13u32, 9u32);
    let px: Vec<[u8; 4]> = (0..w * h)
        .map(|i| {
            [
                (i * 53 % 256) as u8,
                ((i * 97 + 31) % 256) as u8,
                ((i * 151 + 7) % 256) as u8,
                (40 + (i * 71 % 191)) as u8,
            ]
        })
        .collect();
    image(w, h, &px)
}

/// A 13x9 opaque layer of varied colours.
fn varied_opaque() -> EffectImage {
    let px: Vec<[u8; 4]> = pixels(&varied())
        .into_iter()
        .map(|[r, g, b, _]| [r, g, b, 255])
        .collect();
    image(13, 9, &px)
}

#[test]
fn new_blurs_are_known_wire_names_with_labels() {
    for (name, label) in NEW.iter().zip([
        "Bokeh",
        "Fragment",
        "Median",
        "Sketch Blur",
        "Surface Blur",
        "Zoom Blur",
    ]) {
        assert!(EFFECT_NAMES.contains(name), "{name} missing");
        assert_eq!(spec(name, defaults(name)).label(), label);
    }
}

// AC-03
#[test]
fn new_blurs_keep_size_and_the_input_alpha_range() {
    let src = varied();
    let alphas: Vec<u8> = pixels(&src).iter().map(|p| p[3]).collect();
    let (lo, hi) = (
        *alphas.iter().min().expect("pixels"),
        *alphas.iter().max().expect("pixels"),
    );
    for name in NEW {
        for params in settings(name) {
            let out = run(name, &params, &src);
            assert_eq!((out.width(), out.height()), (13, 9), "{name} {params:?}");
            for p in pixels(&out) {
                assert!(
                    (lo..=hi).contains(&p[3]),
                    "{name} {params:?}: alpha {} outside {lo}..={hi}",
                    p[3]
                );
            }
        }
    }
}

// AC-04
#[test]
fn a_solid_opaque_layer_stays_unchanged() {
    let src = image(12, 10, &[[123, 45, 200, 255]; 120]);
    for name in NEW {
        for params in settings(name) {
            assert_eq!(run(name, &params, &src), src, "{name} {params:?}");
        }
    }
}

// AC-05
#[test]
fn transparent_colour_never_tints_an_opaque_neighbour() {
    let (w, h) = (12u32, 12u32);
    let px: Vec<[u8; 4]> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            if (4..8).contains(&x) && (4..8).contains(&y) {
                [255, 0, 0, 255]
            } else {
                [0, 255, 0, 0]
            }
        })
        .collect();
    let src = image(w, h, &px);
    for name in NEW {
        for params in settings(name) {
            for (i, p) in pixels(&run(name, &params, &src)).iter().enumerate() {
                assert!(
                    p[3] == 0 || p[1] <= p[0],
                    "{name} {params:?}: pixel {i} is {p:?}"
                );
            }
        }
    }
}

// AC-06
#[test]
fn fragment_with_distance_zero_is_the_identity() {
    let src = varied();
    for (n, rot) in [(2.0, 0.0), (4.0, 0.0), (17.0, 33.0), (50.0, 359.0)] {
        assert_eq!(run("fragment", &[n, 0.0, rot], &src), src, "{n} {rot}");
    }
}

#[test]
fn fragment_with_a_distance_moves_copies() {
    let src = varied_opaque();
    assert_ne!(run("fragment", &[4.0, 3.0, 0.0], &src), src);
}

/// Every pixel of the clamped `(2r + 1)²` neighbourhood of `(x, y)`.
fn neighbourhood(src: &EffectImage, x: i32, y: i32, r: i32) -> Vec<[u8; 4]> {
    let (w, h) = (src.width() as i32, src.height() as i32);
    let px = pixels(src);
    let mut out = Vec::new();
    for dy in -r..=r {
        for dx in -r..=r {
            let sx = (x + dx).clamp(0, w - 1);
            let sy = (y + dy).clamp(0, h - 1);
            out.push(px[(sy * w + sx) as usize]);
        }
    }
    out
}

// AC-07
#[test]
fn median_percentile_zero_and_hundred_bound_every_channel() {
    let src = varied_opaque();
    let r = 2;
    let low = pixels(&run("median", &[f64::from(r), 0.0], &src));
    let high = pixels(&run("median", &[f64::from(r), 100.0], &src));
    for y in 0..9 {
        for x in 0..13 {
            let near = neighbourhood(&src, x, y, r);
            let i = (y * 13 + x) as usize;
            for c in 0..3 {
                let min = near.iter().map(|p| p[c]).min().expect("pixels");
                let max = near.iter().map(|p| p[c]).max().expect("pixels");
                assert!(low[i][c] <= min, "({x}, {y}) channel {c}: {}", low[i][c]);
                assert!(high[i][c] >= max, "({x}, {y}) channel {c}: {}", high[i][c]);
            }
        }
    }
}

// AC-07
#[test]
fn median_removes_a_single_bright_pixel() {
    let mut px = vec![[20, 30, 40, 255]; 25];
    px[12] = [250, 250, 250, 255];
    let src = image(5, 5, &px);
    let out = pixels(&run("median", &[1.0, 50.0], &src));
    assert_eq!(out, vec![[20, 30, 40, 255]; 25]);
}

#[test]
fn median_of_a_fully_transparent_layer_is_unchanged() {
    let src = image(6, 4, &[[90, 10, 200, 0]; 24]);
    assert_eq!(run("median", &[3.0, 50.0], &src), src);
}

// AC-08
#[test]
fn surface_blur_keeps_a_black_white_edge_sharp() {
    let px: Vec<[u8; 4]> = (0..16 * 8)
        .map(|i| {
            if i % 16 < 8 {
                [0, 0, 0, 255]
            } else {
                [255, 255, 255, 255]
            }
        })
        .collect();
    let src = image(16, 8, &px);
    for r in [1.0, 6.0, 20.0] {
        assert_eq!(run("surface_blur", &[r, 25.0], &src), src, "radius {r}");
    }
}

// AC-08
#[test]
fn surface_blur_smooths_two_close_grays() {
    let (w, h) = (16usize, 8usize);
    let px: Vec<[u8; 4]> = (0..w * h)
        .map(|i| {
            let v = if (i % w + i / w) % 2 == 0 { 100 } else { 110 };
            [v, v, v, 255]
        })
        .collect();
    let out = pixels(&run("surface_blur", &[6.0, 25.0], &image(16, 8, &px)));
    let mut largest = 0;
    for y in 0..h {
        for x in 0..w {
            let p = out[y * w + x];
            for (nx, ny) in [(x + 1, y), (x, y + 1)] {
                if nx < w && ny < h {
                    let q = out[ny * w + nx];
                    for c in 0..3 {
                        largest = largest.max(p[c].abs_diff(q[c]));
                    }
                }
            }
        }
    }
    assert!(largest < 10, "largest neighbour difference {largest}");
}

// AC-09
#[test]
fn sketch_blur_is_reproducible_per_seed() {
    let src = varied();
    for seed in [0.0, 1.0, 4242.0] {
        let a = run("sketch_blur", &[6.0, seed], &src);
        let b = run("sketch_blur", &[6.0, seed], &src);
        assert_eq!(a, b, "seed {seed}");
    }
    assert_ne!(
        run("sketch_blur", &[6.0, 1.0], &src),
        run("sketch_blur", &[6.0, 2.0], &src)
    );
}

/// 21x5 opaque vertical 1-px stripes, black on even columns.
fn stripes() -> EffectImage {
    let px: Vec<[u8; 4]> = (0..21 * 5)
        .map(|i| {
            let v = if i % 21 % 2 == 0 { 0 } else { 255 };
            [v, v, v, 255]
        })
        .collect();
    image(21, 5, &px)
}

// AC-10
#[test]
fn zoom_blur_grows_with_the_distance_from_the_centre() {
    let src = stripes();
    let out = run("zoom_blur", &[10.0, 0.5, 0.5], &src);
    let (a, b) = (pixels(&src), pixels(&out));
    let change = |x: usize| -> f64 {
        (0..5)
            .map(|y| f64::from(a[y * 21 + x][0].abs_diff(b[y * 21 + x][0])))
            .sum::<f64>()
            / 5.0
    };
    assert_eq!(change(10), 0.0, "the middle column stays");
    for d in 1..=10 {
        for (near, far) in [(10 + d - 1, 10 + d), (10 + 1 - d, 10 - d)] {
            assert!(
                change(far) > change(near),
                "distance {d}: column {far} changed {} <= column {near} {}",
                change(far),
                change(near)
            );
        }
    }
}

// AC-10
#[test]
fn zoom_blur_equals_radial_blur_zoom() {
    for src in [stripes(), varied()] {
        for (amount, cx, cy) in [(20.0, 0.5, 0.5), (100.0, 0.0, 0.0), (45.0, 0.3, 0.9)] {
            assert_eq!(
                run("zoom_blur", &[amount, cx, cy], &src),
                run("radial_blur", &[amount, cx, cy, 1.0], &src),
                "{amount} {cx} {cy}"
            );
        }
    }
}

/// Alpha of every pixel of a 9x9 transparent layer with one opaque white
/// pixel in the middle, after Motion Blur with `params`.
fn motion_alpha(params: &[f64]) -> Vec<u8> {
    let mut px = vec![[0, 0, 0, 0]; 81];
    px[40] = [255, 255, 255, 255];
    pixels(&run("motion_blur", params, &image(9, 9, &px)))
        .iter()
        .map(|p| p[3])
        .collect()
}

// AC-11
#[test]
fn motion_blur_uncentered_spreads_to_one_side_only() {
    let a = motion_alpha(&[5.0, 0.0, 0.0]);
    let at = |x: usize, y: usize| a[y * 9 + x];
    assert!(at(2, 4) > 0 && at(0, 4) > 0, "trails behind the angle");
    for x in 5..9 {
        assert_eq!(at(x, 4), 0, "nothing ahead at x = {x}");
    }
    for (i, &v) in a.iter().enumerate() {
        if i / 9 != 4 {
            assert_eq!(v, 0, "row {} stays transparent", i / 9);
        }
    }
    // At 90° (down) the trail runs up.
    let a = motion_alpha(&[5.0, 90.0, 0.0]);
    assert!(a[9 + 4] > 0);
    for y in 5..9 {
        assert_eq!(a[y * 9 + 4], 0, "nothing below at y = {y}");
    }
}

// AC-11
#[test]
fn motion_blur_centered_spreads_symmetrically() {
    let a = motion_alpha(&[5.0, 0.0, 1.0]);
    let at = |x: usize| a[4 * 9 + x];
    assert!(at(2) > 0);
    for k in 1..=4 {
        assert_eq!(at(4 - k), at(4 + k), "offset {k}");
    }
}

// AC-13
#[test]
fn existing_wire_calls_keep_working() {
    let src = varied();
    assert_eq!(
        spec("motion_blur", &[5.0, 30.0]),
        spec("motion_blur", &[5.0, 30.0, 1.0])
    );
    assert_eq!(
        run("motion_blur", &[5.0, 30.0], &src),
        run("motion_blur", &[5.0, 30.0, 1.0], &src)
    );
    let zoom = run("radial_blur", &[50.0, 0.5, 0.5, 1.0], &src);
    assert_eq!(zoom, run("zoom_blur", &[50.0, 0.5, 0.5], &src));
    assert_ne!(zoom, run("radial_blur", &[50.0, 0.5, 0.5, 0.0], &src));
}

// AC-14
#[test]
fn wrong_parameter_count_is_rejected() {
    for name in NEW {
        let n = defaults(name).len();
        for got in [n - 1, n + 1] {
            assert!(
                matches!(
                    EffectSpec::from_params(name, &vec![1.0; got]),
                    Err(EffectError::ParamCount { expected, got: g, .. }) if expected == n && g == got
                ),
                "{name} with {got}"
            );
        }
    }
    for got in [0, 1, 4] {
        assert!(
            matches!(
                EffectSpec::from_params("motion_blur", &vec![1.0; got]),
                Err(EffectError::ParamCount { expected: 3, .. })
            ),
            "motion_blur with {got}"
        );
    }
}

// AC-14
#[test]
fn non_finite_numbers_read_as_zero_then_clamp_to_the_range() {
    let same = |name: &str, a: &[f64], b: &[f64]| {
        assert_eq!(spec(name, a), spec(name, b), "{name} {a:?} vs {b:?}");
    };
    let nan = f64::NAN;
    let inf = f64::INFINITY;
    same("bokeh", &[nan, inf], &[1.0, 0.0]);
    same("bokeh", &[500.0, 500.0], &[100.0, 100.0]);
    same("bokeh", &[-5.0, -5.0], &[1.0, 0.0]);
    same("fragment", &[nan, nan, nan], &[2.0, 0.0, 0.0]);
    same("fragment", &[99.0, 400.0, 0.0], &[50.0, 100.0, 0.0]);
    same("fragment", &[1.0, -3.0, 0.0], &[2.0, 0.0, 0.0]);
    // An angle wraps around, as the dial does.
    same("fragment", &[4.0, 8.0, 370.0], &[4.0, 8.0, 10.0]);
    same("fragment", &[4.0, 8.0, -10.0], &[4.0, 8.0, 350.0]);
    same("median", &[nan, inf], &[1.0, 0.0]);
    same("median", &[900.0, 101.0], &[200.0, 100.0]);
    same("sketch_blur", &[nan, nan], &[1.0, 0.0]);
    same("sketch_blur", &[300.0, 20000.0], &[100.0, 9999.0]);
    same("surface_blur", &[nan, nan], &[1.0, 1.0]);
    same("surface_blur", &[300.0, 300.0], &[100.0, 100.0]);
    same("zoom_blur", &[nan, nan, inf], &[1.0, 0.0, 0.0]);
    same("zoom_blur", &[300.0, 2.0, -1.0], &[100.0, 1.0, 0.0]);
    same("motion_blur", &[5.0, 30.0, nan], &[5.0, 30.0, 0.0]);
}
