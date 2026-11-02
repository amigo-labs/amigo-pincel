//! Potrace modes through the public session API
//! (`docs/specs/vectorize-core.md` AC-03 – AC-11, AC-15, AC-16, AC-18, AC-19).

use pincel_trace::{
    Mode, PotraceParams, Rgb, Threshold, TraceError, TraceOptions, TraceResult, TraceSession,
};

use crate::geom::{contains, polygons};
use crate::images::{self, Img};

fn session(img: &Img) -> TraceSession {
    TraceSession::new(&img.rgba, img.w, img.h).expect("valid image")
}

fn mono(threshold: Threshold) -> TraceOptions {
    TraceOptions {
        mode: Mode::Mono {
            threshold,
            invert: false,
            fill: Rgb::new(0, 0, 0),
        },
        ..TraceOptions::default()
    }
}

fn colors(n: u8) -> TraceOptions {
    TraceOptions {
        mode: Mode::Color {
            colors: n,
            pinned: None,
        },
        ..TraceOptions::default()
    }
}

fn pinned(hexes: &[&str]) -> TraceOptions {
    TraceOptions {
        mode: Mode::Color {
            colors: 6,
            pinned: Some(
                hexes
                    .iter()
                    .map(|h| Rgb::from_hex(h).expect("hex"))
                    .collect(),
            ),
        },
        ..TraceOptions::default()
    }
}

fn trace(img: &Img, opts: &TraceOptions) -> TraceResult {
    session(img).trace(opts).expect("trace")
}

fn outlines(r: &TraceResult) -> usize {
    r.layers
        .iter()
        .map(|l| l.path_data.matches('M').count())
        .sum()
}

fn hexes(r: &TraceResult) -> Vec<String> {
    r.layers.iter().map(|l| l.color.rgb().hex()).collect()
}

#[test]
fn mono_ring_pixel_centres_follow_even_odd_outlines() {
    let img = images::ring();
    let r = trace(&img, &mono(Threshold::Auto));
    assert_eq!(r.layers.len(), 1);
    assert_eq!(outlines(&r), 2);
    let polys = polygons(&r.layers[0].path_data);
    let mut checked = 0;
    for y in 0..img.h {
        for x in 0..img.w {
            let (cx, cy) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let dist = (cx - 60.5).hypot(cy - 60.5);
            if (dist - 45.0).abs() < 2.0 || (dist - 20.0).abs() < 2.0 {
                continue;
            }
            let filled = img.filled(x, y)[0] == 0;
            assert_eq!(contains(&polys, cx, cy), filled, "pixel ({x}, {y})");
            checked += 1;
        }
    }
    assert!(checked > 10_000);
}

#[test]
fn mono_rectangle_starts_at_reference_point() {
    let r = trace(&images::rectangle(), &mono(Threshold::Auto));
    assert!(
        r.layers[0].path_data.starts_with("M10 30"),
        "{}",
        r.layers[0].path_data
    );
}

#[test]
fn mono_min_area_drops_the_stray_pixel() {
    let img = images::square_with_stray_pixel();
    let with = |min_area| TraceOptions {
        potrace: PotraceParams {
            min_area,
            ..PotraceParams::default()
        },
        ..mono(Threshold::Auto)
    };
    assert_eq!(outlines(&trace(&img, &with(2))), 1);
    assert_eq!(outlines(&trace(&img, &with(0))), 2);
}

#[test]
fn color_four_colour_image_finds_all_four_largest_first() {
    let r = trace(&images::four_colour(), &colors(6));
    let mut got = hexes(&r);
    assert_eq!(got[0], "#f4efe6");
    got.sort();
    assert_eq!(got, ["#27ae60", "#2e86ab", "#c0392b", "#f4efe6"]);
    assert!(r.layers.windows(2).all(|w| w[0].area >= w[1].area));
    assert_eq!(r.layers.iter().map(|l| l.area).sum::<u64>(), 30_000);
}

#[test]
fn color_transparent_pixels_belong_to_no_layer() {
    let base = images::four_colour();
    let img = Img::new(base.w, base.h, |x, y| {
        let mut px = base.filled(x as u32, y as u32);
        if px[..3] == [192, 57, 43] {
            px[3] = 100;
        }
        px
    });
    let opaque = img
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[3] >= 128)
        .count() as u64;
    let r = trace(&img, &colors(6));
    assert!(!hexes(&r).contains(&"#c0392b".to_string()));
    assert_eq!(r.layers.iter().map(|l| l.area).sum::<u64>(), opaque);
    let t = trace(&images::transparent_1x1(), &colors(6));
    assert!(t.layers.is_empty());
}

#[test]
fn mono_auto_threshold_lands_in_the_valley() {
    let r = trace(&images::noisy_two_tone(), &mono(Threshold::Auto));
    let th = r.threshold_used.expect("threshold reported");
    assert!((101..160).contains(&th), "threshold {th}");
}

#[test]
fn mono_manual_threshold_invert_and_fill() {
    let img = images::disc();
    let manual = trace(&img, &mono(Threshold::Manual(77)));
    assert_eq!(manual.threshold_used, Some(77));
    assert_eq!(manual.layers.len(), 1);
    let inverted = trace(
        &img,
        &TraceOptions {
            mode: Mode::Mono {
                threshold: Threshold::Manual(128),
                invert: true,
                fill: Rgb::new(10, 20, 30),
            },
            ..TraceOptions::default()
        },
    );
    assert_eq!(inverted.layers[0].color.rgb().hex(), "#0a141e");
    let polys = polygons(&inverted.layers[0].path_data);
    assert!(
        !contains(&polys, 50.5, 50.5),
        "disc centre is not filled when inverted"
    );
    assert!(
        contains(&polys, 5.5, 5.5),
        "background is filled when inverted"
    );
    let white = Img::new(10, 10, |_, _| [255, 255, 255, 255]);
    let none = trace(&white, &mono(Threshold::Auto));
    assert!(none.layers.is_empty());
    assert!(none.threshold_used.is_some());
    assert!(trace(&img, &mono(Threshold::Manual(0))).layers.is_empty());
}

#[test]
fn mono_invert_fills_a_transparent_image() {
    let img = Img::new(8, 8, |_, _| [0, 0, 0, 0]);
    let r = trace(
        &img,
        &TraceOptions {
            mode: Mode::Mono {
                threshold: Threshold::Manual(128),
                invert: true,
                fill: Rgb::new(0, 0, 0),
            },
            ..TraceOptions::default()
        },
    );
    assert_eq!(r.layers.len(), 1);
    assert_eq!(r.layers[0].area, 64);
}

#[test]
fn despeckle_collapses_salt_and_pepper() {
    let img = images::speckled_two_tone();
    let opts = |despeckle| TraceOptions {
        despeckle,
        potrace: PotraceParams {
            min_area: 0,
            ..PotraceParams::default()
        },
        ..mono(Threshold::Manual(130))
    };
    let mut s = session(&img);
    let before = outlines(&s.trace(&opts(false)).expect("trace"));
    let after = outlines(&s.trace(&opts(true)).expect("trace"));
    assert!(after * 5 < before, "{before} -> {after}");
}

#[test]
fn pinned_palette_uses_exactly_the_pinned_colours() {
    let img = images::four_colour();
    let two = trace(&img, &pinned(&["#f4efe6", "#2e86ab"]));
    let mut got = hexes(&two);
    got.sort();
    assert_eq!(got, ["#2e86ab", "#f4efe6"]);
    assert_eq!(two.layers.iter().map(|l| l.area).sum::<u64>(), 30_000);

    let five = trace(
        &img,
        &pinned(&["#f4efe6", "#2e86ab", "#c0392b", "#27ae60", "#ff00ff"]),
    );
    assert_eq!(five.layers.len(), 4);
    assert!(!hexes(&five).contains(&"#ff00ff".to_string()));

    let dup = trace(&img, &pinned(&["#f4efe6", "#2e86ab", "#2e86ab"]));
    assert_eq!(
        dup.layers.len(),
        2,
        "the repeated colour receives no pixels"
    );

    assert!(trace(&img, &pinned(&[])).layers.is_empty());

    let ignored = TraceOptions {
        mode: Mode::Color {
            colors: 50,
            pinned: Some(vec![Rgb::new(0, 0, 0), Rgb::new(255, 255, 255)]),
        },
        ..TraceOptions::default()
    };
    assert_eq!(trace(&img, &ignored).layers.len(), 2);
}

#[test]
fn stats_add_up() {
    let r = trace(&images::four_colour(), &colors(6));
    let s = r.stats;
    assert_eq!(s.curves + s.lines, s.segments);
    assert_eq!(s.paths as usize, outlines(&r));
    assert!(s.curves > 0 && s.lines > 0);
}

#[test]
fn same_options_give_identical_results() {
    let img = images::four_colour();
    let mut s = session(&img);
    let a = s.trace(&colors(6)).expect("trace");
    let b = s.trace(&colors(6)).expect("trace");
    let c = trace(&img, &colors(6));
    assert_eq!(a, b);
    assert_eq!(a, c);
}

#[test]
fn session_rejects_bad_images() {
    assert!(matches!(
        TraceSession::new(&[0; 12], 2, 2),
        Err(TraceError::BufferLength { .. })
    ));
    assert!(matches!(
        TraceSession::new(&[], 0, 5),
        Err(TraceError::EmptyImage { .. })
    ));
    assert!(matches!(
        TraceSession::new(&[], 5, 0),
        Err(TraceError::EmptyImage { .. })
    ));
    let wide = vec![0_u8; 32_768 * 4];
    assert!(matches!(
        TraceSession::new(&wide, 32_768, 1),
        Err(TraceError::TooLarge { .. })
    ));
    let edge = vec![0_u8; 32_767 * 4];
    assert!(TraceSession::new(&edge, 32_767, 1).is_ok());
    assert!(matches!(
        TraceSession::new(&wide, 1, 32_768),
        Err(TraceError::TooLarge { .. })
    ));
    assert!(TraceSession::new(&edge, 1, 32_767).is_ok());
    assert!(matches!(
        TraceSession::new(&[], u32::MAX, u32::MAX),
        Err(TraceError::TooLarge { .. })
    ));
}

#[test]
fn trace_rejects_out_of_range_options_and_recovers() {
    let mut s = session(&images::disc());
    let bad = |opts: TraceOptions| match s_trace(&opts) {
        Err(TraceError::InvalidOption { option, .. }) => option,
        other => panic!("expected an invalid option, got {other:?}"),
    };
    fn s_trace(opts: &TraceOptions) -> Result<TraceResult, TraceError> {
        TraceSession::new(&images::disc().rgba, 100, 100)?.trace(opts)
    }
    assert_eq!(bad(colors(1)), "colors");
    assert_eq!(bad(colors(17)), "colors");
    assert_eq!(bad(pinned(&["#000000"; 17])), "pinned");
    let pot = |alpha_max, opt_tolerance| TraceOptions {
        potrace: PotraceParams {
            alpha_max,
            opt_tolerance,
            ..PotraceParams::default()
        },
        ..TraceOptions::default()
    };
    assert_eq!(bad(pot(1.35, 0.2)), "alpha_max");
    assert_eq!(bad(pot(-0.1, 0.2)), "alpha_max");
    assert_eq!(bad(pot(f64::NAN, 0.2)), "alpha_max");
    assert_eq!(bad(pot(1.0, -0.1)), "opt_tolerance");
    assert_eq!(bad(pot(1.0, f64::INFINITY)), "opt_tolerance");
    // The first invalid option in AC-19's order is named.
    assert_eq!(
        bad(TraceOptions {
            mode: Mode::Color {
                colors: 1,
                pinned: None
            },
            ..pot(9.0, -1.0)
        }),
        "colors"
    );
    // Options are checked in modes that ignore them too.
    assert_eq!(
        bad(TraceOptions {
            mode: Mode::Mono {
                threshold: Threshold::Auto,
                invert: false,
                fill: Rgb::new(0, 0, 0)
            },
            ..pot(9.0, 0.2)
        }),
        "alpha_max"
    );

    assert!(s.trace(&pot(1.35, 0.2)).is_err());
    assert_eq!(
        s.trace(&mono(Threshold::Auto))
            .expect("recovers")
            .layers
            .len(),
        1
    );
}

/// A `w × h` image, left half `left`, right half `right`.
fn halves(w: u32, h: u32, left: [u8; 4], right: [u8; 4]) -> Img {
    Img::new(w, h, |x, _| if x < i64::from(w / 2) { left } else { right })
}

#[test]
fn mono_partial_alpha_blends_towards_white() {
    // Black at alpha 128: 0 × a + 255 × (1 − 128/255) = 127.
    // Black at alpha 191: 255 × (64/255) = 64.
    for (alpha, brightness) in [(128_u8, 127_u8), (191, 64)] {
        let img = Img::new(4, 4, |_, _| [0, 0, 0, alpha]);
        let above = trace(&img, &mono(Threshold::Manual(brightness + 1)));
        assert_eq!(
            above.layers.len(),
            1,
            "alpha {alpha}: {brightness} < threshold"
        );
        assert_eq!(above.layers[0].area, 16);
        let at = trace(&img, &mono(Threshold::Manual(brightness)));
        assert!(
            at.layers.is_empty(),
            "alpha {alpha}: {brightness} is not below itself"
        );
    }
}

#[test]
fn pinned_equal_areas_keep_the_listed_order() {
    let img = halves(8, 4, [255, 0, 0, 255], [0, 0, 255, 255]);
    assert_eq!(
        hexes(&trace(&img, &pinned(&["#0000ff", "#ff0000"]))),
        ["#0000ff", "#ff0000"]
    );
    assert_eq!(
        hexes(&trace(&img, &pinned(&["#ff0000", "#0000ff"]))),
        ["#ff0000", "#0000ff"]
    );
}

#[test]
fn pinned_colours_closer_than_the_merge_distance_stay_apart() {
    // Greys 127 and 128 differ by far less than ΔE 2.5 but sit in different
    // colour bins, so each receives its own half.
    let img = halves(8, 4, [127, 127, 127, 255], [128, 128, 128, 255]);
    let r = trace(&img, &pinned(&["#7f7f7f", "#808080"]));
    assert_eq!(hexes(&r), ["#7f7f7f", "#808080"]);
    assert!(r.layers.iter().all(|l| l.area == 16));
}

#[test]
fn color_alpha_cut_off_is_128_free_and_pinned() {
    // Left half alpha 128 (opaque enough), right half alpha 127 (ignored).
    let img = halves(8, 4, [255, 0, 0, 128], [0, 0, 255, 127]);
    for opts in [colors(6), pinned(&["#ff0000", "#0000ff"])] {
        let r = trace(&img, &opts);
        assert_eq!(hexes(&r), ["#ff0000"]);
        assert_eq!(r.layers[0].area, 16);
    }
}

#[test]
fn mono_with_every_outline_dropped_has_zero_layers() {
    // One black pixel: filled, but its outline (area 1) is not above the
    // minimum area 2 (AC-09).
    let img = Img::new(5, 5, |x, y| {
        if (x, y) == (2, 2) {
            [0, 0, 0, 255]
        } else {
            [255, 255, 255, 255]
        }
    });
    let r = trace(&img, &mono(Threshold::Manual(128)));
    assert!(r.layers.is_empty());
    assert_eq!(r.stats.paths, 0);
}

#[test]
fn colours_without_a_kept_outline_get_no_layer() {
    // The four-colour image plus a 1-px magenta speck: magenta receives the
    // pixel but its only outline falls to the minimum area (AC-11).
    let mut img = images::four_colour();
    img.rgba[0..4].copy_from_slice(&[255, 0, 255, 255]);
    let all = ["#f4efe6", "#2e86ab", "#c0392b", "#27ae60", "#ff00ff"];
    let r = trace(&img, &pinned(&all));
    assert_eq!(r.layers.len(), 4);
    assert!(!hexes(&r).contains(&"#ff00ff".to_string()));
    assert!(r.layers.iter().all(|l| !l.path_data.is_empty()));
}

#[test]
fn range_boundaries_are_accepted() {
    let img = images::disc();
    let pot = |alpha_max, opt_tolerance| TraceOptions {
        potrace: PotraceParams {
            alpha_max,
            opt_tolerance,
            ..PotraceParams::default()
        },
        ..TraceOptions::default()
    };
    for opts in [
        colors(2),
        colors(16),
        pot(0.0, 0.2),
        pot(1.34, 0.2),
        pot(1.0, 0.0),
        pinned(&[
            "#000000", "#111111", "#222222", "#333333", "#444444", "#555555", "#666666", "#777777",
            "#888888", "#999999", "#aaaaaa", "#bbbbbb", "#cccccc", "#dddddd", "#eeeeee", "#ffffff",
        ]),
    ] {
        assert!(session(&img).trace(&opts).is_ok(), "{opts:?}");
    }
}

#[test]
fn auto_threshold_cache_keeps_despeckle_apart() {
    // A grey disc on a light ground with isolated black points on a 3 × 3
    // grid: the median removes the points, which moves Otsu's threshold
    // (60 without despeckle, 150 with it).
    let img = Img::new(60, 60, |x, y| {
        let v = if (x - 30).pow(2) + (y - 30).pow(2) <= 15 * 15 {
            120
        } else if x % 3 == 1 && y % 3 == 1 {
            0
        } else {
            180
        };
        [v, v, v, 255]
    });
    let with = |despeckle| TraceOptions {
        despeckle,
        ..mono(Threshold::Auto)
    };
    let mut s = session(&img);
    let plain = s.trace(&with(false)).expect("trace");
    let cached = s.trace(&with(true)).expect("trace");
    let fresh = session(&img).trace(&with(true)).expect("trace");
    assert_eq!(cached.threshold_used, fresh.threshold_used);
    assert_eq!(cached, fresh);
    assert_ne!(
        plain.threshold_used, fresh.threshold_used,
        "despeckle moves the threshold here"
    );
}
