//! docs/specs/effects-render.md: paint.net's Render effects (Clouds,
//! Turbulence, Julia Fractal, Mandelbrot Fractal) through the wire format
//! both cores use.

use pincel_effects::{EFFECT_NAMES, EffectError, EffectImage, EffectSpec};

const NAMES: [&str; 4] = [
    "clouds",
    "turbulence",
    "julia_fractal",
    "mandelbrot_fractal",
];

/// Blend mode indices on the wire.
const NORMAL: f64 = 0.0;
const MULTIPLY: f64 = 1.0;
const SCREEN: f64 = 2.0;
const OVERLAY: f64 = 3.0;

const BLACK: u32 = 0x0000_00FF;
const WHITE: u32 = 0xFFFF_FFFF;

fn spec(name: &str, params: &[f64]) -> EffectSpec {
    EffectSpec::from_params(name, params).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn pixels(img: &EffectImage) -> Vec<[u8; 4]> {
    img.data().as_chunks::<4>().0.to_vec()
}

fn run(name: &str, params: &[f64], src: &EffectImage) -> EffectImage {
    spec(name, params).run(src)
}

/// A `w` × `h` layer of varied colours and alphas (some fully transparent).
fn noisy(w: u32, h: u32) -> EffectImage {
    let data = (0..w * h)
        .flat_map(|i| {
            let v = i.wrapping_mul(2_654_435_761);
            let a = match i % 5 {
                0 => 0,
                1 => 255,
                _ => (v >> 24) as u8,
            };
            [v as u8, (v >> 8) as u8, (v >> 16) as u8, a]
        })
        .collect();
    EffectImage::from_rgba8(w, h, data).expect("w * h pixels")
}

fn solid(w: u32, h: u32, px: [u8; 4]) -> EffectImage {
    EffectImage::from_rgba8(w, h, px.repeat((w * h) as usize)).expect("w * h pixels")
}

/// The table's defaults, in wire order, with black / white as the
/// foreground / background colours.
fn defaults(name: &str) -> Vec<f64> {
    match name {
        "clouds" => vec![250.0, 50.0, 1.0, f64::from(BLACK), f64::from(WHITE), NORMAL],
        "turbulence" => vec![200.0, 5.0, 1.0, f64::from(BLACK), f64::from(WHITE), NORMAL],
        "julia_fractal" => vec![4.0, 1.0, 0.0, 2.0],
        "mandelbrot_fractal" => vec![1.0, 10.0, 0.0, 2.0, 0.0],
        other => panic!("not a render effect: {other}"),
    }
}

/// Clouds / Turbulence at a small scale so a small layer shows texture.
fn noise_params(name: &str, seed: f64, c1: u32, c2: u32, blend: f64) -> Vec<f64> {
    let second = if name == "clouds" { 70.0 } else { 4.0 };
    vec![12.0, second, seed, f64::from(c1), f64::from(c2), blend]
}

#[test]
fn every_render_effect_is_a_wire_name() {
    for name in NAMES {
        assert!(EFFECT_NAMES.contains(&name), "{name}");
        assert!(!spec(name, &defaults(name)).label().is_empty());
    }
}

// AC-03
#[test]
fn render_effects_are_reproducible_and_clouds_and_turbulence_follow_the_seed() {
    let src = noisy(24, 16);
    for name in NAMES {
        let p = defaults(name);
        let a = run(name, &p, &src);
        assert_eq!(
            a,
            run(name, &p, &src),
            "{name}: same parameters, same pixels"
        );
        assert_eq!(
            a,
            run(name, &p, &solid(24, 16, [0; 4])),
            "{name}: independent of content"
        );
        assert_eq!((a.width(), a.height()), (24, 16), "{name}: size");
    }
    for name in ["clouds", "turbulence"] {
        let one = run(name, &noise_params(name, 1.0, BLACK, WHITE, NORMAL), &src);
        let two = run(name, &noise_params(name, 2.0, BLACK, WHITE, NORMAL), &src);
        assert_ne!(pixels(&one), pixels(&two), "{name}: a different seed");
        // Not a flat fill: the texture varies over the layer.
        let p = pixels(&one);
        assert!(p.iter().any(|&q| q != p[0]), "{name}: texture");
    }
}

// AC-04
#[test]
fn normal_blend_stays_between_the_two_colours_whatever_the_content() {
    let src = noisy(32, 32);
    let pairs = [
        (0x2040_80FF_u32, 0xF0C0_1080_u32),
        (0xFFFF_FF00, 0x0000_00FF),
        (BLACK, WHITE),
    ];
    for name in ["clouds", "turbulence"] {
        for (c1, c2) in pairs {
            for seed in [0.0, 7.0, 9999.0] {
                let out = run(name, &noise_params(name, seed, c1, c2, NORMAL), &src);
                let (a, b) = (c1.to_be_bytes(), c2.to_be_bytes());
                for (i, p) in pixels(&out).iter().enumerate() {
                    for ch in 0..4 {
                        let (lo, hi) = (a[ch].min(b[ch]), a[ch].max(b[ch]));
                        assert!(
                            (lo..=hi).contains(&p[ch]),
                            "{name} {c1:08X}/{c2:08X} pixel {i} channel {ch}: {}",
                            p[ch]
                        );
                    }
                }
            }
        }
        // Equal colours fill the target with that colour.
        let c = 0x3366_CC80;
        let out = run(name, &noise_params(name, 3.0, c, c, NORMAL), &src);
        assert!(pixels(&out).iter().all(|p| *p == c.to_be_bytes()), "{name}");
    }
}

// AC-04: a transparent Colour 2 gives see-through clouds.
#[test]
fn normal_blend_takes_the_colours_alpha() {
    let out = run(
        "clouds",
        &noise_params("clouds", 1.0, BLACK, 0x0000_0000, NORMAL),
        &solid(32, 32, [255, 255, 255, 255]),
    );
    let alphas: Vec<u8> = pixels(&out).iter().map(|p| p[3]).collect();
    assert!(alphas.iter().any(|&a| a < 255), "some see-through pixels");
}

// AC-05
#[test]
fn multiply_never_brightens_and_screen_never_darkens() {
    let src = noisy(32, 32);
    let before = pixels(&src);
    for name in ["clouds", "turbulence"] {
        for (c1, c2) in [
            (0x2040_80FF, 0xF0C0_1080),
            (BLACK, WHITE),
            (0xFF00_00FF, 0x00FF_0040),
        ] {
            let mul = pixels(&run(name, &noise_params(name, 5.0, c1, c2, MULTIPLY), &src));
            let scr = pixels(&run(name, &noise_params(name, 5.0, c1, c2, SCREEN), &src));
            for (i, b) in before.iter().enumerate() {
                for ch in 0..3 {
                    assert!(mul[i][ch] <= b[ch], "{name} multiply pixel {i} ch {ch}");
                    assert!(scr[i][ch] >= b[ch], "{name} screen pixel {i} ch {ch}");
                }
                assert_eq!(mul[i][3], b[3], "{name} multiply keeps alpha");
                assert_eq!(scr[i][3], b[3], "{name} screen keeps alpha");
            }
        }
        // Overlay keeps alpha too and changes something.
        let ovl = pixels(&run(
            name,
            &noise_params(name, 5.0, BLACK, WHITE, OVERLAY),
            &src,
        ));
        assert!(ovl.iter().zip(&before).all(|(o, b)| o[3] == b[3]), "{name}");
        assert_ne!(ovl, before, "{name}: overlay");
    }
}

// Edge case: Multiply over a fully transparent layer stays transparent.
#[test]
fn multiply_over_transparency_stays_transparent() {
    let src = solid(16, 16, [0; 4]);
    for name in ["clouds", "turbulence"] {
        let out = run(name, &noise_params(name, 1.0, BLACK, WHITE, MULTIPLY), &src);
        assert!(pixels(&out).iter().all(|p| p[3] == 0), "{name}");
    }
}

/// Sum of absolute differences between horizontal and vertical neighbours
/// over the RGB channels.
fn roughness(img: &EffectImage) -> u64 {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let p = pixels(img);
    let mut sum = 0u64;
    for y in 0..h {
        for x in 0..w {
            for (nx, ny) in [(x + 1, y), (x, y + 1)] {
                if nx < w && ny < h {
                    let (a, b) = (p[y * w + x], p[ny * w + nx]);
                    sum += a[..3]
                        .iter()
                        .zip(&b[..3])
                        .map(|(u, v)| u64::from(u.abs_diff(*v)))
                        .sum::<u64>();
                }
            }
        }
    }
    sum
}

// AC-06
#[test]
fn clouds_roughness_zero_is_smoother_than_roughness_hundred() {
    let src = solid(64, 64, [0; 4]);
    for seed in [1.0, 2.0, 77.0] {
        for scale in [16.0, 250.0] {
            let at = |r: f64| {
                let p = [scale, r, seed, f64::from(BLACK), f64::from(WHITE), NORMAL];
                roughness(&run("clouds", &p, &src))
            };
            let (smooth, rough) = (at(0.0), at(100.0));
            assert!(
                smooth < rough,
                "seed {seed} scale {scale}: {smooth} vs {rough}"
            );
        }
    }
}

// AC-07
#[test]
fn fractals_fill_every_pixel_opaquely_and_invert_inverts() {
    for src in [noisy(33, 21), solid(33, 21, [0; 4])] {
        for (name, params) in [
            ("julia_fractal", vec![4.0, 1.0, 0.0, 2.0]),
            ("julia_fractal", vec![10.0, 50.0, 45.0, 1.0]),
            ("mandelbrot_fractal", vec![1.0, 10.0, 0.0, 2.0, 0.0]),
            ("mandelbrot_fractal", vec![7.0, 100.0, 120.0, 3.0, 1.0]),
        ] {
            let out = run(name, &params, &src);
            assert!(
                pixels(&out).iter().all(|p| p[3] == 255),
                "{name} {params:?}"
            );
        }
        for (zoom, quality) in [(10.0, 2.0), (1.0, 1.0), (40.0, 4.0)] {
            let off = run("mandelbrot_fractal", &[3.0, zoom, 30.0, quality, 0.0], &src);
            let on = run("mandelbrot_fractal", &[3.0, zoom, 30.0, quality, 1.0], &src);
            for (a, b) in pixels(&off).iter().zip(pixels(&on)) {
                assert_eq!(b, [255 - a[0], 255 - a[1], 255 - a[2], 255]);
            }
        }
    }
}

// The fractals paint the escape-time rainbow: more than a few colours;
// Mandelbrot's view holds points inside the set (black). The Julia constant
// −0.4 + 0.6i lies just outside the Mandelbrot set, so its Julia set has
// no interior to find at this sampling.
#[test]
fn fractals_paint_hues_outside_the_set_and_black_inside() {
    for (name, params) in [
        ("julia_fractal", vec![4.0, 1.0, 0.0, 1.0]),
        ("mandelbrot_fractal", vec![1.0, 10.0, 0.0, 1.0, 0.0]),
    ] {
        let out = pixels(&run(name, &params, &solid(64, 48, [0; 4])));
        let distinct: std::collections::HashSet<[u8; 4]> = out.iter().copied().collect();
        assert!(distinct.len() > 8, "{name}: {} colours", distinct.len());
        if name == "mandelbrot_fractal" {
            assert!(
                out.contains(&[0, 0, 0, 255]),
                "{name}: points inside the set"
            );
        }
    }
}

// AC-08
#[test]
fn quality_keeps_the_size_and_the_set_interior_stays_black() {
    let src = solid(65, 65, [10, 20, 30, 40]);
    for quality in 1..=5 {
        let q = f64::from(quality);
        for (name, params) in [
            ("julia_fractal", vec![4.0, 1.0, 0.0, q]),
            ("mandelbrot_fractal", vec![1.0, 10.0, 0.0, q, 0.0]),
        ] {
            let out = run(name, &params, &src);
            assert_eq!((out.width(), out.height()), (65, 65), "{name} quality {q}");
        }
        // The centre pixel covers −0.5 + 0i ± 0.023: inside the main cardioid.
        let out = pixels(&run("mandelbrot_fractal", &[1.0, 10.0, 0.0, q, 0.0], &src));
        assert_eq!(out[32 * 65 + 32], [0, 0, 0, 255], "quality {q}");
    }
}

// Higher quality averages sub-samples: it changes pixels on the set's edge.
#[test]
fn quality_supersamples_the_edge_of_the_set() {
    let src = solid(40, 30, [0; 4]);
    let one = run("mandelbrot_fractal", &[1.0, 10.0, 0.0, 1.0, 0.0], &src);
    let three = run("mandelbrot_fractal", &[1.0, 10.0, 0.0, 3.0, 0.0], &src);
    assert_ne!(one, three);
}

// Angle rotates the view about its centre: 180° mirrors the symmetric
// Mandelbrot set onto itself only about the centre, so pixels move.
#[test]
fn fractal_angle_rotates_the_view() {
    let src = solid(40, 30, [0; 4]);
    for (name, a, b) in [
        (
            "julia_fractal",
            vec![4.0, 1.0, 0.0, 1.0],
            vec![4.0, 1.0, 90.0, 1.0],
        ),
        (
            "mandelbrot_fractal",
            vec![1.0, 10.0, 0.0, 1.0, 0.0],
            vec![1.0, 10.0, 90.0, 1.0, 0.0],
        ),
    ] {
        assert_ne!(run(name, &a, &src), run(name, &b, &src), "{name}");
    }
}

// Technical notes: the noise sits in layer coordinates, so a buffer placed
// at a canvas offset shows the same texture as that part of the canvas.
#[test]
fn noise_follows_the_canvas_position_of_the_buffer() {
    for name in ["clouds", "turbulence"] {
        let s = spec(name, &noise_params(name, 4.0, BLACK, WHITE, NORMAL));
        let whole = pixels(&s.run(&solid(20, 12, [0; 4])));
        let part = pixels(&s.run_at(&solid(6, 4, [0; 4]), (9, 5), |_, _| true));
        for y in 0..4 {
            for x in 0..6 {
                assert_eq!(
                    part[y * 6 + x],
                    whole[(y + 5) * 20 + x + 9],
                    "{name} ({x}, {y})"
                );
            }
        }
        // A target restriction does not move the texture.
        let only_some = pixels(&s.run_in(&solid(20, 12, [0; 4]), |x, _| x > 10));
        assert_eq!(only_some, whole, "{name}");
    }
}

// Edge case: a scale larger than the layer works and gives soft shapes.
#[test]
fn a_scale_larger_than_the_layer_gives_a_soft_gradient() {
    let p = [
        1000.0,
        50.0,
        1.0,
        f64::from(BLACK),
        f64::from(WHITE),
        NORMAL,
    ];
    let out = run("clouds", &p, &solid(16, 16, [0; 4]));
    let px = pixels(&out);
    for w in px.windows(2) {
        assert!(w[0][0].abs_diff(w[1][0]) <= 32, "no hard steps");
    }
}

// AC-10
#[test]
fn wrong_parameter_count_is_rejected() {
    for name in NAMES {
        let n = defaults(name).len();
        for got in [0, n - 1, n + 1] {
            let mut p = defaults(name);
            p.resize(got, 1.0);
            assert!(
                matches!(
                    EffectSpec::from_params(name, &p),
                    Err(EffectError::ParamCount { expected, got: g, .. }) if expected == n && g == got
                ),
                "{name} with {got}"
            );
        }
    }
}

// AC-10
#[test]
fn an_invalid_colour_slot_is_rejected() {
    for name in ["clouds", "turbulence"] {
        for slot in [3, 4] {
            for bad in [0.5, -1.0, 4_294_967_296.0, f64::NAN, f64::INFINITY] {
                let mut p = defaults(name);
                p[slot] = bad;
                assert!(
                    matches!(
                        EffectSpec::from_params(name, &p),
                        Err(EffectError::ColorSlot { slot: s, .. }) if s == slot
                    ),
                    "{name} slot {slot} = {bad}"
                );
            }
        }
    }
}

// AC-10
#[test]
fn non_finite_numbers_read_as_zero_then_clamp_to_the_range() {
    let same = |name: &str, a: &[f64], b: &[f64]| {
        assert_eq!(spec(name, a), spec(name, b), "{name} {a:?} vs {b:?}");
    };
    let nan = f64::NAN;
    let inf = f64::INFINITY;
    let (k, w) = (f64::from(BLACK), f64::from(WHITE));
    same(
        "clouds",
        &[nan, inf, nan, k, w, nan],
        &[2.0, 0.0, 0.0, k, w, NORMAL],
    );
    same(
        "clouds",
        &[5000.0, 500.0, 1e9, k, w, 9.0],
        &[1000.0, 100.0, 9999.0, k, w, OVERLAY],
    );
    same(
        "clouds",
        &[-5.0, -5.0, -5.0, k, w, -2.0],
        &[2.0, 0.0, 0.0, k, w, NORMAL],
    );
    same(
        "turbulence",
        &[nan, nan, inf, k, w, inf],
        &[2.0, 1.0, 0.0, k, w, NORMAL],
    );
    same(
        "turbulence",
        &[5000.0, 20.0, 12345.0, k, w, 1.4],
        &[1000.0, 8.0, 9999.0, k, w, MULTIPLY],
    );
    same(
        "julia_fractal",
        &[nan, nan, nan, nan],
        &[1.0, 1.0, 0.0, 1.0],
    );
    same(
        "julia_fractal",
        &[50.0, 500.0, 370.0, 9.0],
        &[10.0, 50.0, 10.0, 5.0],
    );
    same(
        "julia_fractal",
        &[-1.0, -1.0, -10.0, -1.0],
        &[1.0, 1.0, 350.0, 1.0],
    );
    same(
        "mandelbrot_fractal",
        &[nan, nan, nan, nan, nan],
        &[1.0, 1.0, 0.0, 1.0, 0.0],
    );
    same(
        "mandelbrot_fractal",
        &[50.0, 500.0, 720.0, 9.0, 1.0],
        &[10.0, 100.0, 0.0, 5.0, 1.0],
    );
}
