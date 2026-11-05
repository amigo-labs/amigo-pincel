//! docs/specs/effects-object-stylize-color-noise.md: Object ▸ Drop Shadow,
//! Stylize ▸ Outline, Color ▸ Quantize and the rebuilt Noise ▸ Reduce Noise
//! through the wire format both cores use.

use std::collections::{HashMap, HashSet};

use pincel_effects::{EFFECT_NAMES, EffectError, EffectImage, EffectSpec};

const NAMES: [&str; 4] = ["drop_shadow", "stylize_outline", "quantize", "reduce_noise"];

const MEDIAN_CUT: f64 = 0.0;
const OCTREE: f64 = 1.0;

const BLACK: f64 = 0x0000_00FF_u32 as f64;
const CLEAR: [u8; 4] = [0, 0, 0, 0];

fn spec(name: &str, params: &[f64]) -> EffectSpec {
    EffectSpec::from_params(name, params).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn run(name: &str, params: &[f64], src: &EffectImage) -> EffectImage {
    spec(name, params).run(src)
}

fn image(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> EffectImage {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            data.extend_from_slice(&f(x, y));
        }
    }
    EffectImage::from_rgba8(w, h, data).expect("w * h pixels")
}

fn px(img: &EffectImage, x: u32, y: u32) -> [u8; 4] {
    let o = ((y * img.width() + x) * 4) as usize;
    let d = img.data();
    [d[o], d[o + 1], d[o + 2], d[o + 3]]
}

fn pixels(img: &EffectImage) -> Vec<[u8; 4]> {
    img.data().as_chunks::<4>().0.to_vec()
}

/// The pixels that differ between `a` and `b`, row-major.
fn changed(a: &EffectImage, b: &EffectImage) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for y in 0..a.height() {
        for x in 0..a.width() {
            if px(a, x, y) != px(b, x, y) {
                out.push((x, y));
            }
        }
    }
    out
}

/// A deterministic hash of a position, for test images.
fn hash(x: u32, y: u32, salt: u32) -> u32 {
    let mut h = x
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add(y.wrapping_mul(0x85EB_CA77))
        .wrapping_add(salt.wrapping_mul(0xC2B2_AE3D));
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h
}

/// A noisy picture with opaque, partly transparent and fully transparent
/// pixels; the transparent ones store loud colours.
fn mixed(w: u32, h: u32) -> EffectImage {
    image(w, h, |x, y| {
        let v = hash(x, y, 1);
        let alpha = match v % 7 {
            0 => 0,
            1 => 90,
            2 => 1,
            _ => 255,
        };
        [(v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8, alpha]
    })
}

/// A sprite: a few hard-edged shapes in a handful of colours on
/// transparency (alpha is 0 or 255 only).
fn sprite(w: u32, h: u32) -> EffectImage {
    image(w, h, |x, y| match (x, y) {
        (2..=5, 2..=5) => [200, 40, 40, 255],
        (6..=8, 3..=4) => [30, 120, 220, 255],
        (4..=6, 7..=9) => [250, 230, 10, 255],
        (10, 1) => [0, 0, 0, 255],
        _ => CLEAR,
    })
}

/// `src` with every fully transparent pixel's colour replaced by `rgb`.
fn recolour_transparent(src: &EffectImage, rgb: [u8; 3]) -> EffectImage {
    image(src.width(), src.height(), |x, y| {
        let p = px(src, x, y);
        if p[3] == 0 {
            [rgb[0], rgb[1], rgb[2], 0]
        } else {
            p
        }
    })
}

fn bt709(p: [u8; 4]) -> f64 {
    0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2])
}

fn opaque_rgb(img: &EffectImage) -> HashSet<[u8; 3]> {
    pixels(img)
        .into_iter()
        .filter(|p| p[3] > 0)
        .map(|p| [p[0], p[1], p[2]])
        .collect()
}

// --- Drop Shadow --------------------------------------------------------

/// Drop Shadow settings: `[offset_x, offset_y, blur, color, opacity]`.
fn shadow_settings() -> Vec<[f64; 5]> {
    vec![
        [4.0, 4.0, 4.0, BLACK, 50.0],
        [4.0, 4.0, 0.0, BLACK, 50.0],
        [-3.0, 2.0, 1.5, 0xFF80_10C0_u32 as f64, 100.0],
        [0.0, 0.0, 6.0, 0x3366_99FF_u32 as f64, 80.0],
        [100.0, -100.0, 50.0, BLACK, 100.0],
        [1.0, -1.0, 0.0, 0x00FF_0080_u32 as f64, 30.0],
    ]
}

/// AC-03: an opaque pixel keeps its colour under every setting.
#[test]
fn drop_shadow_never_changes_an_opaque_pixel() {
    for src in [mixed(17, 13), sprite(12, 11)] {
        for s in shadow_settings() {
            let out = run("drop_shadow", &s, &src);
            for (i, (a, b)) in pixels(&src).iter().zip(pixels(&out)).enumerate() {
                if a[3] == 255 {
                    assert_eq!(*a, b, "pixel {i} under {s:?}");
                }
            }
        }
    }
}

/// AC-03: Opacity 0 or a fully transparent Colour changes nothing.
#[test]
fn drop_shadow_opacity_zero_or_clear_colour_changes_nothing() {
    for src in [mixed(17, 13), sprite(12, 11)] {
        for s in shadow_settings() {
            let no_opacity = [s[0], s[1], s[2], s[3], 0.0];
            assert_eq!(run("drop_shadow", &no_opacity, &src), src, "{no_opacity:?}");
            let clear = 0x3366_9900_u32 as f64;
            let no_alpha = [s[0], s[1], s[2], clear, s[4]];
            assert_eq!(run("drop_shadow", &no_alpha, &src), src, "{no_alpha:?}");
        }
    }
}

/// AC-04: one opaque pixel casts exactly one shadow pixel at the offset,
/// in exactly the shadow colour.
#[test]
fn drop_shadow_blur_zero_single_pixel_lands_at_the_offset() {
    let colour = [0x33, 0x66, 0x99, 0xFF];
    let slot = f64::from(u32::from_be_bytes(colour));
    for (x, y) in [(3, 2), (0, 0), (7, 5)] {
        // Transparent pixels store a loud colour; only the shadow pixel
        // may change.
        let src = image(12, 10, |px, py| {
            if (px, py) == (x, y) {
                [250, 250, 250, 255]
            } else {
                [255, 0, 255, 0]
            }
        });
        let out = run("drop_shadow", &[4.0, 4.0, 0.0, slot, 100.0], &src);
        assert_eq!(px(&out, x + 4, y + 4), colour, "shadow of ({x}, {y})");
        assert_eq!(changed(&src, &out), vec![(x + 4, y + 4)], "from ({x}, {y})");
    }
}

/// AC-04: with Blur 0 a hard-edged target gains at most one new colour,
/// the shadow colour at its alpha.
#[test]
fn drop_shadow_blur_zero_on_hard_alpha_adds_at_most_one_colour() {
    let src = sprite(14, 13);
    let before: HashSet<[u8; 4]> = pixels(&src).into_iter().collect();
    for (offset, colour, opacity) in [
        ((4.0, 4.0), [0u8, 0, 0, 255], 50.0),
        ((1.0, 2.0), [20, 200, 90, 200], 50.0),
        ((-2.0, 3.0), [255, 255, 255, 255], 100.0),
        ((3.0, -1.0), [10, 20, 30, 77], 33.0),
    ] {
        let slot = f64::from(u32::from_be_bytes(colour));
        let out = run(
            "drop_shadow",
            &[offset.0, offset.1, 0.0, slot, opacity],
            &src,
        );
        let alpha = (f64::from(colour[3]) * opacity / 100.0).round() as u8;
        let shadow = [colour[0], colour[1], colour[2], alpha];
        let new: HashSet<[u8; 4]> = pixels(&out)
            .into_iter()
            .filter(|p| !before.contains(p))
            .collect();
        assert!(!new.is_empty(), "{offset:?}: the shadow shows");
        assert_eq!(new, HashSet::from([shadow]), "{offset:?} {colour:?}");
    }
}

/// Edge case: a shadow pushed fully outside the layer changes nothing.
#[test]
fn drop_shadow_offset_outside_the_layer_changes_nothing() {
    let src = sprite(12, 11);
    for (ox, oy) in [(100.0, 0.0), (0.0, -100.0), (-20.0, 30.0)] {
        assert_eq!(run("drop_shadow", &[ox, oy, 0.0, BLACK, 100.0], &src), src);
    }
}

/// The shadow lies behind a partly transparent pixel and blurs beyond the
/// hard edge.
#[test]
fn drop_shadow_shows_beneath_partly_transparent_pixels_and_blurs() {
    let src = image(16, 16, |x, y| match (x, y) {
        (4..=7, 4..=7) => [255, 255, 255, 255],
        (8..=9, 8..=9) => [255, 255, 255, 100],
        _ => CLEAR,
    });
    let out = run("drop_shadow", &[2.0, 2.0, 0.0, BLACK, 100.0], &src);
    // (8, 8) sits over the shadow of (6, 6): more opaque, darker.
    let p = px(&out, 8, 8);
    assert_eq!(p[3], 255);
    assert!(p[0] < 255, "the shadow shows through: {p:?}");
    // Blurred: the shadow reaches pixels the hard shadow misses.
    let soft = run("drop_shadow", &[2.0, 2.0, 4.0, BLACK, 100.0], &src);
    assert_eq!(px(&out, 12, 12), CLEAR);
    assert!(px(&soft, 12, 12)[3] > 0, "{:?}", px(&soft, 12, 12));
}

// --- Stylize ▸ Outline --------------------------------------------------

/// AC-05: a solid single-colour layer stays as it is.
#[test]
fn stylize_outline_solid_layer_is_unchanged() {
    for colour in [[200, 100, 50, 255], [0, 0, 0, 255], [10, 250, 90, 120]] {
        let src = image(11, 9, |_, _| colour);
        for s in [[3.0, 50.0], [1.0, 100.0], [200.0, 100.0]] {
            assert_eq!(run("stylize_outline", &s, &src), src, "{colour:?} {s:?}");
        }
    }
}

/// AC-05: Intensity 0 leaves every layer unchanged.
#[test]
fn stylize_outline_intensity_zero_changes_nothing() {
    for src in [mixed(17, 13), sprite(12, 11)] {
        for t in [1.0, 3.0, 200.0] {
            assert_eq!(
                run("stylize_outline", &[t, 0.0], &src),
                src,
                "thickness {t}"
            );
        }
    }
}

/// AC-05: at a white / black boundary the white pixels within Thickness
/// get darker, the ones farther away stay white.
#[test]
fn stylize_outline_darkens_white_within_thickness_only() {
    const WHITE: [u8; 4] = [255, 255, 255, 255];
    let src = image(24, 6, |x, _| if x < 10 { [0, 0, 0, 255] } else { WHITE });
    for t in [1u32, 3, 6] {
        let out = run("stylize_outline", &[f64::from(t), 50.0], &src);
        for y in 0..6 {
            for x in 10..24 {
                let p = px(&out, x, y);
                if x - 10 < t {
                    assert!(bt709(p) < bt709(WHITE), "t {t}: ({x}, {y}) {p:?}");
                    assert_eq!(p[3], 255);
                } else {
                    assert_eq!(p, WHITE, "t {t}: ({x}, {y})");
                }
            }
        }
    }
}

/// Edge case: a Thickness larger than the layer works (edges clamp).
#[test]
fn stylize_outline_thickness_beyond_the_layer_works() {
    let src = mixed(9, 7);
    let out = run("stylize_outline", &[200.0, 100.0], &src);
    assert_eq!((out.width(), out.height()), (9, 7));
    assert_ne!(out, src);
}

// --- Quantize -----------------------------------------------------------

/// A picture with many distinct colours, some transparency.
fn colourful(w: u32, h: u32) -> EffectImage {
    image(w, h, |x, y| {
        let alpha = if hash(x, y, 9).is_multiple_of(11) {
            0
        } else if hash(x, y, 9).is_multiple_of(13) {
            140
        } else {
            255
        };
        [
            (x * 255 / w) as u8,
            (y * 255 / h) as u8,
            (hash(x, y, 3) % 256) as u8,
            alpha,
        ]
    })
}

/// AC-06: at most Colours distinct colours, every alpha unchanged.
#[test]
fn quantize_leaves_at_most_colours_and_keeps_alpha() {
    let src = colourful(32, 24);
    assert!(opaque_rgb(&src).len() > 300);
    for algorithm in [MEDIAN_CUT, OCTREE] {
        for n in [2u32, 3, 5, 16, 64, 256] {
            for dither in [0.0, 50.0, 100.0] {
                let out = run("quantize", &[f64::from(n), algorithm, dither], &src);
                let label = format!("algorithm {algorithm} n {n} dither {dither}");
                assert!(opaque_rgb(&out).len() <= n as usize, "{label}");
                for (a, b) in pixels(&src).iter().zip(pixels(&out)) {
                    assert_eq!(a[3], b[3], "{label}");
                }
            }
        }
    }
}

/// AC-06: a target with no more than Colours distinct colours and
/// Dithering 0 is unchanged.
#[test]
fn quantize_few_colours_stay_unchanged() {
    let src = sprite(14, 13);
    let colours = opaque_rgb(&src).len() as u32;
    assert_eq!(colours, 4);
    // Ten colours on a 10-colour picture, and 256.
    let ten = image(10, 4, |x, y| {
        [
            (x * 25) as u8,
            255 - (x * 20) as u8,
            7,
            if y == 3 { 0 } else { 255 },
        ]
    });
    for algorithm in [MEDIAN_CUT, OCTREE] {
        for n in [colours, 16, 256] {
            assert_eq!(run("quantize", &[f64::from(n), algorithm, 0.0], &src), src);
        }
        for n in [10.0, 256.0] {
            assert_eq!(run("quantize", &[n, algorithm, 0.0], &ten), ten);
        }
        // Edge case: a fully transparent target.
        let clear = image(6, 6, |x, y| [x as u8 * 40, y as u8 * 40, 9, 0]);
        assert_eq!(run("quantize", &[2.0, algorithm, 100.0], &clear), clear);
    }
}

/// AC-06: the target is the selection — its own colours stay when there
/// are few enough of them, whatever the rest of the layer holds.
#[test]
fn quantize_reads_its_colours_from_the_target() {
    let src = image(16, 16, |x, y| {
        if x < 4 && y < 4 {
            [[10, 20, 30, 255], [200, 0, 0, 255], [0, 200, 0, 255]][((x + y) % 3) as usize]
        } else {
            [(x * 16) as u8, (y * 16) as u8, ((x * y) % 256) as u8, 255]
        }
    });
    for algorithm in [MEDIAN_CUT, OCTREE] {
        let out = spec("quantize", &[3.0, algorithm, 0.0]).run_in(&src, |x, y| x < 4 && y < 4);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(px(&out, x, y), px(&src, x, y), "({x}, {y})");
            }
        }
    }
}

/// AC-07: without dithering equal input colours map to equal output
/// colours; with dithering the result is still deterministic.
#[test]
fn quantize_is_a_colour_map_without_dither_and_deterministic_with() {
    let src = image(40, 30, |x, y| {
        let v = (hash(x / 3, y / 3, 5) % 40) as u8;
        [v * 6, 255 - v * 5, v * 3, 255]
    });
    for algorithm in [MEDIAN_CUT, OCTREE] {
        let out = run("quantize", &[6.0, algorithm, 0.0], &src);
        let mut map: HashMap<[u8; 4], [u8; 4]> = HashMap::new();
        for (a, b) in pixels(&src).into_iter().zip(pixels(&out)) {
            assert_eq!(*map.entry(a).or_insert(b), b, "{a:?}");
        }
        for dither in [10.0, 100.0] {
            let p = [6.0, algorithm, dither];
            let first = run("quantize", &p, &src);
            assert_eq!(run("quantize", &p, &src), first, "{p:?}");
            assert_ne!(first, out, "dithering diffuses the error: {p:?}");
        }
    }
}

/// The two algorithms are different algorithms.
#[test]
fn quantize_median_cut_and_octree_differ() {
    let src = colourful(32, 24);
    let a = run("quantize", &[8.0, MEDIAN_CUT, 0.0], &src);
    let b = run("quantize", &[8.0, OCTREE, 0.0], &src);
    assert_ne!(a, b);
}

// --- Reduce Noise -------------------------------------------------------

/// AC-08: Strength 0 leaves the layer unchanged.
#[test]
fn reduce_noise_strength_zero_changes_nothing() {
    let src = mixed(17, 13);
    for r in [1.0, 6.0, 200.0] {
        assert_eq!(run("reduce_noise", &[r, 0.0], &src), src, "radius {r}");
    }
}

/// AC-08: Strength 100 is the neighbourhood median; the old one-slot form
/// is Strength 100 at its own radius, also above 10.
#[test]
fn reduce_noise_strength_100_and_the_old_form_are_the_median() {
    let src = mixed(23, 19);
    for r in [1.0, 2.0, 6.0, 15.0] {
        let median = run("median", &[r, 50.0], &src);
        assert_eq!(run("reduce_noise", &[r, 100.0], &src), median, "radius {r}");
        assert_eq!(
            run("reduce_noise", &[r], &src),
            median,
            "old form radius {r}"
        );
    }
    assert_ne!(
        run("reduce_noise", &[15.0], &src),
        run("reduce_noise", &[10.0], &src),
        "radius 15 is not capped at 10"
    );
}

/// Strength between 0 and 100 lands between the layer and its median.
#[test]
fn reduce_noise_partial_strength_blends_toward_the_median() {
    let src = image(9, 9, |x, y| {
        if (x, y) == (4, 4) {
            [255, 255, 255, 255]
        } else {
            [100, 100, 100, 255]
        }
    });
    let out = run("reduce_noise", &[1.0, 40.0], &src);
    // 255 + (100 − 255) · 0.4 = 193.
    assert_eq!(px(&out, 4, 4), [193, 193, 193, 255]);
    assert_eq!(px(&out, 0, 0), [100, 100, 100, 255]);
}

// --- Shared -------------------------------------------------------------

/// AC-09: Stylize ▸ Outline and Reduce Noise never let a fully transparent
/// pixel's stored colour reach an opaque neighbour.
#[test]
fn neighbourhood_effects_ignore_transparent_colour() {
    let src = mixed(19, 15);
    let other = recolour_transparent(&src, [0, 0, 0]);
    let loud = recolour_transparent(&src, [255, 255, 255]);
    for (name, params) in [
        ("stylize_outline", vec![3.0, 50.0]),
        ("stylize_outline", vec![9.0, 100.0]),
        ("reduce_noise", vec![6.0, 40.0]),
        ("reduce_noise", vec![2.0, 100.0]),
        ("reduce_noise", vec![3.0]),
    ] {
        let a = run(name, &params, &other);
        let b = run(name, &params, &loud);
        for (i, ((s, pa), pb)) in pixels(&src)
            .iter()
            .zip(pixels(&a))
            .zip(pixels(&b))
            .enumerate()
        {
            if s[3] > 0 {
                assert_eq!(pa, pb, "{name} {params:?}: pixel {i}");
            }
        }
    }
}

/// AC-10: a wrong number of parameters is rejected.
#[test]
fn from_params_wrong_parameter_count_is_rejected() {
    for (name, wrong) in [
        ("drop_shadow", vec![vec![], vec![4.0; 4], vec![4.0; 6]]),
        ("stylize_outline", vec![vec![], vec![3.0], vec![3.0; 3]]),
        ("quantize", vec![vec![], vec![16.0; 2], vec![16.0; 4]]),
        ("reduce_noise", vec![vec![], vec![6.0; 3]]),
    ] {
        for p in wrong {
            assert!(
                matches!(
                    EffectSpec::from_params(name, &p),
                    Err(EffectError::ParamCount { .. })
                ),
                "{name} with {} slots",
                p.len()
            );
        }
    }
}

/// AC-10: Drop Shadow's colour slot must be an integer 0…2³²−1.
#[test]
fn drop_shadow_bad_colour_slot_is_rejected() {
    for bad in [
        -1.0,
        0.5,
        4_294_967_296.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        assert_eq!(
            EffectSpec::from_params("drop_shadow", &[4.0, 4.0, 4.0, bad, 50.0]),
            Err(EffectError::ColorSlot {
                effect: "drop_shadow".into(),
                slot: 3,
            }),
            "{bad}"
        );
    }
    for good in [0.0, 4_294_967_295.0] {
        assert!(EffectSpec::from_params("drop_shadow", &[4.0, 4.0, 4.0, good, 50.0]).is_ok());
    }
}

/// AC-10: non-finite numbers read as 0, then every number is clamped to
/// its range.
#[test]
fn from_params_non_finite_reads_as_zero_and_numbers_clamp() {
    let nan = f64::NAN;
    let inf = f64::INFINITY;
    for (name, given, same) in [
        (
            "drop_shadow",
            vec![nan, inf, -inf, BLACK, nan],
            vec![0.0, 0.0, 0.0, BLACK, 0.0],
        ),
        (
            "drop_shadow",
            vec![500.0, -500.0, 99.0, BLACK, 200.0],
            vec![100.0, -100.0, 50.0, BLACK, 100.0],
        ),
        (
            "drop_shadow",
            vec![3.4, -3.6, -1.0, BLACK, -5.0],
            vec![3.0, -4.0, 0.0, BLACK, 0.0],
        ),
        ("stylize_outline", vec![nan, inf], vec![1.0, 0.0]),
        ("stylize_outline", vec![999.0, 101.0], vec![200.0, 100.0]),
        ("quantize", vec![nan, nan, nan], vec![2.0, 0.0, 0.0]),
        (
            "quantize",
            vec![1000.0, 7.0, 150.0],
            vec![256.0, 1.0, 100.0],
        ),
        ("quantize", vec![1.0, -3.0, -1.0], vec![2.0, 0.0, 0.0]),
        ("reduce_noise", vec![nan, nan], vec![1.0, 0.0]),
        ("reduce_noise", vec![999.0, 150.0], vec![200.0, 100.0]),
        ("reduce_noise", vec![inf], vec![1.0, 100.0]),
    ] {
        assert_eq!(spec(name, &given), spec(name, &same), "{name} {given:?}");
    }
}

/// The four wire ids are known, in `EFFECT_NAMES`, and run on any image.
#[test]
fn every_new_effect_is_listed_and_runs() {
    for name in NAMES {
        assert!(EFFECT_NAMES.contains(&name), "{name}");
    }
    let src = mixed(5, 4);
    for (name, p) in [
        ("drop_shadow", vec![4.0, 4.0, 4.0, BLACK, 50.0]),
        ("stylize_outline", vec![3.0, 50.0]),
        ("quantize", vec![16.0, MEDIAN_CUT, 0.0]),
        ("reduce_noise", vec![6.0, 40.0]),
    ] {
        let out = run(name, &p, &src);
        assert_eq!((out.width(), out.height()), (5, 4), "{name}");
        let one = EffectImage::from_rgba8(1, 1, vec![9, 9, 9, 255]).expect("1x1");
        assert_eq!(run(name, &p, &one).width(), 1, "{name} on 1x1");
        let empty = EffectImage::new_transparent(0, 0);
        assert_eq!(run(name, &p, &empty).width(), 0, "{name} on 0x0");
    }
}
