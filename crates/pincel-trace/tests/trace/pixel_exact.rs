//! Pixel-exact mode, the SVG document, determinism and the session caches
//! (`docs/specs/vectorize-core.md` AC-12 – AC-14, AC-16, AC-17, AC-20).

use pincel_trace::{
    Mode, PotraceParams, Rgb, Rgba, Threshold, TraceError, TraceOptions, TraceResult, TraceSession,
    to_svg,
};

use crate::geom::{Step, contains, parse, polygons};
use crate::images::{self, Img};

fn exact() -> TraceOptions {
    TraceOptions {
        mode: Mode::PixelExact,
        ..TraceOptions::default()
    }
}

fn trace(img: &Img, opts: &TraceOptions) -> TraceResult {
    TraceSession::new(&img.rgba, img.w, img.h)
        .expect("valid image")
        .trace(opts)
        .expect("trace")
}

const R: [u8; 4] = [255, 0, 0, 255];
const G: [u8; 4] = [0, 255, 0, 255];
const B: [u8; 4] = [0, 0, 255, 255];
const HALF: [u8; 4] = [255, 0, 0, 128];
const NONE: [u8; 4] = [0, 0, 0, 0];

/// A sprite with a hole, an island inside the hole, diagonal-only
/// neighbours, a semi-transparent colour, transparent pixels and a lone
/// pixel.
fn sprite() -> Img {
    let rows = [
        "RRRRRRRR.",
        "R......R.",
        "R.GGG..R.",
        "R.GBG..RG",
        "R.GGG..R.",
        "RRRRRRRR.",
        ".B.B.hh..",
        "B.B.hhh..",
        "........B",
    ];
    Img::new(9, 9, |x, y| match rows[y as usize].as_bytes()[x as usize] {
        b'R' => R,
        b'G' => G,
        b'B' => B,
        b'h' => HALF,
        _ => NONE,
    })
}

/// Checks AC-12's outline shape: axis-aligned integer steps, no two
/// consecutive steps (including the closing one) in the same direction.
fn assert_pixel_edges(d: &str) {
    let mut start = (0.0, 0.0);
    let mut cur = (0.0, 0.0);
    let mut dirs: Vec<(i8, i8)> = Vec::new();
    let check = |dirs: &[(i8, i8)]| {
        assert!(dirs.len() >= 4, "{d}");
        for i in 0..dirs.len() {
            assert_ne!(
                dirs[i],
                dirs[(i + 1) % dirs.len()],
                "collinear steps in {d}"
            );
        }
    };
    let dir = |dx: f64, dy: f64| {
        assert!(dx.fract() == 0.0 && dy.fract() == 0.0, "{d}");
        assert!((dx == 0.0) != (dy == 0.0), "not axis-aligned in {d}");
        // `f64::signum(0.0)` is 1.0, so compare explicitly.
        let sign = |v: f64| (v > 0.0) as i8 - (v < 0.0) as i8;
        (sign(dx), sign(dy))
    };
    for step in parse(d) {
        match step {
            Step::Move(x, y) => {
                assert!(x.fract() == 0.0 && y.fract() == 0.0, "{d}");
                start = (x, y);
                cur = start;
                dirs.clear();
            }
            Step::Line(dx, dy) => {
                dirs.push(dir(dx, dy));
                cur = (cur.0 + dx, cur.1 + dy);
            }
            Step::Cubic(_) => panic!("curve in pixel-exact data: {d}"),
            Step::Close => {
                dirs.push(dir(start.0 - cur.0, start.1 - cur.1));
                check(&dirs);
            }
        }
    }
}

#[test]
fn pixel_exact_layers_cover_exactly_their_pixels() {
    let img = sprite();
    let r = trace(&img, &exact());
    assert_eq!(r.layers.len(), 4);
    for layer in &r.layers {
        assert_pixel_edges(&layer.path_data);
        let polys = polygons(&layer.path_data);
        let c = layer.color;
        let mut area = 0;
        for y in 0..img.h {
            for x in 0..img.w {
                let px = img.filled(x, y);
                let mine = px == [c.r, c.g, c.b, c.a];
                area += u64::from(mine);
                let inside = contains(&polys, f64::from(x) + 0.5, f64::from(y) + 0.5);
                assert_eq!(inside, mine, "{c:?} at ({x}, {y})");
            }
        }
        assert_eq!(layer.area, area);
    }
}

#[test]
fn pixel_exact_rectangle_is_four_steps_and_ignores_min_area() {
    let img = Img::new(6, 5, |x, y| {
        if (1..5).contains(&x) && (1..4).contains(&y) {
            B
        } else {
            NONE
        }
    });
    let r = trace(&img, &exact());
    assert_eq!(r.layers.len(), 1);
    let d = &r.layers[0].path_data;
    assert_eq!(d.matches('l').count(), 3, "{d}");
    assert!(d.ends_with('z'));
    assert_eq!(r.stats.segments, 4);
    assert_eq!(r.stats.curves, 0);
    assert_eq!(r.stats.lines, 4);

    let lone = Img::new(3, 3, |x, y| if x == 1 && y == 1 { R } else { NONE });
    let opts = TraceOptions {
        potrace: PotraceParams {
            min_area: 100,
            ..PotraceParams::default()
        },
        despeckle: true,
        ..exact()
    };
    let r = trace(&lone, &opts);
    assert_eq!(r.layers.len(), 1);
    assert_eq!(r.layers[0].area, 1);
    assert!(
        trace(&images::transparent_1x1(), &exact())
            .layers
            .is_empty()
    );
}

#[test]
fn pixel_exact_checkerboard_keeps_every_pixel_centre_right() {
    let img = Img::new(3, 3, |x, y| if (x + y) % 2 == 0 { R } else { G });
    let r = trace(&img, &exact());
    for layer in &r.layers {
        assert_pixel_edges(&layer.path_data);
        let polys = polygons(&layer.path_data);
        for y in 0..3 {
            for x in 0..3 {
                let mine = img.filled(x, y) == [layer.color.r, layer.color.g, layer.color.b, 255];
                assert_eq!(
                    contains(&polys, f64::from(x) + 0.5, f64::from(y) + 0.5),
                    mine
                );
            }
        }
    }
}

#[test]
fn pixel_exact_alpha_order_and_ties() {
    let img = sprite();
    let r = trace(&img, &exact());
    let colors: Vec<Rgba> = r.layers.iter().map(|l| l.color).collect();
    assert!(colors.contains(&Rgba::new(255, 0, 0, 128)));
    assert!(colors.contains(&Rgba::new(255, 0, 0, 255)));
    assert!(r.layers.windows(2).all(|w| w[0].area >= w[1].area));

    // Equal areas keep the order in which the colours first appear.
    let tie = Img::new(4, 1, |x, _| [G, B, G, B][x as usize]);
    let t = trace(&tie, &exact());
    assert_eq!(t.layers[0].color, Rgba::new(0, 255, 0, 255));
    assert_eq!(t.layers[1].color, Rgba::new(0, 0, 255, 255));
}

#[test]
fn pixel_exact_refuses_more_than_256_colours_and_recovers() {
    let img = Img::new(257, 1, |x, _| [x as u8, (x >> 8) as u8, 7, 255]);
    let mut s = TraceSession::new(&img.rgba, img.w, img.h).expect("valid image");
    assert_eq!(
        s.trace(&exact()),
        Err(TraceError::TooManyColors {
            count: 257,
            max: 256
        })
    );
    assert!(s.trace(&TraceOptions::default()).is_ok());
    let ok = Img::new(256, 1, |x, _| [x as u8, 0, 7, 255]);
    assert_eq!(trace(&ok, &exact()).layers.len(), 256);
}

#[test]
fn svg_document_per_mode() {
    let img = Img::new(2, 1, |x, _| if x == 0 { R } else { HALF });
    let r = trace(&img, &exact());
    assert_eq!(r.layers[0].path_data, "M0 0l1 0l0 1l-1 0z");
    assert_eq!(r.layers[1].path_data, "M1 0l1 0l0 1l-1 0z");
    assert_eq!(
        to_svg(&r),
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2 1" width="2" height="1" shape-rendering="crispEdges">"#,
            r##"<path d="M0 0l1 0l0 1l-1 0z" fill="#ff0000" fill-rule="evenodd"/>"##,
            r##"<path d="M1 0l1 0l0 1l-1 0z" fill="#ff0000" fill-opacity="0.502" fill-rule="evenodd"/>"##,
            "</svg>"
        )
    );

    let color = to_svg(&trace(&images::four_colour(), &TraceOptions::default()));
    assert!(color.starts_with(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 150" width="200" height="150">"#
    ));
    assert_eq!(color.matches("<path ").count(), 4);
    assert_eq!(
        color
            .matches(r#" stroke-width="0.5" stroke-linejoin="round" fill-rule="evenodd"/>"#)
            .count(),
        4
    );
    assert!(color.contains(r##"fill="#f4efe6" stroke="#f4efe6""##));
    assert!(!color.contains("crispEdges"));

    let mono = TraceOptions {
        mode: Mode::Mono {
            threshold: Threshold::Auto,
            invert: false,
            fill: Rgb::new(1, 2, 3),
        },
        ..TraceOptions::default()
    };
    let m = to_svg(&trace(&images::disc(), &mono));
    assert!(m.contains(r##"fill="#010203" fill-rule="evenodd"/>"##));
    assert!(!m.contains("stroke"));

    let empty = to_svg(&trace(&images::transparent_1x1(), &exact()));
    assert_eq!(
        empty,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1" width="1" height="1" shape-rendering="crispEdges"></svg>"#
    );
}

#[test]
fn svg_is_byte_identical_across_sessions() {
    for opts in [TraceOptions::default(), exact()] {
        let img = images::four_colour();
        let mut s = TraceSession::new(&img.rgba, img.w, img.h).expect("valid image");
        let a = to_svg(&s.trace(&opts).expect("trace"));
        let b = to_svg(&s.trace(&opts).expect("trace"));
        let c = to_svg(&trace(&img, &opts));
        assert_eq!(a, b);
        assert_eq!(a, c);
    }
}

#[test]
fn changing_options_in_one_session_matches_fresh_sessions() {
    let img = images::four_colour();
    let d = TraceOptions::default();
    let seq = vec![
        d.clone(),
        TraceOptions {
            mode: Mode::Color {
                colors: 3,
                pinned: None,
            },
            ..d.clone()
        },
        TraceOptions {
            despeckle: true,
            ..d.clone()
        },
        TraceOptions {
            mode: Mode::Color {
                colors: 6,
                pinned: Some(vec![Rgb::new(0, 0, 0), Rgb::new(255, 255, 255)]),
            },
            ..d.clone()
        },
        TraceOptions {
            mode: Mode::Mono {
                threshold: Threshold::Auto,
                invert: false,
                fill: Rgb::new(0, 0, 0),
            },
            ..d.clone()
        },
        exact(),
        TraceOptions {
            potrace: PotraceParams {
                alpha_max: 0.0,
                ..PotraceParams::default()
            },
            ..d.clone()
        },
        d,
    ];
    let mut s = TraceSession::new(&img.rgba, img.w, img.h).expect("valid image");
    for opts in &seq {
        assert_eq!(s.trace(opts).expect("trace"), trace(&img, opts), "{opts:?}");
    }
}

#[test]
fn pixel_exact_ignores_despeckle() {
    // Despeckle would turn the lone red pixel white; pixel-exact keeps it.
    let img = Img::new(5, 5, |x, y| {
        if (x, y) == (2, 2) {
            R
        } else {
            [255, 255, 255, 255]
        }
    });
    let plain = trace(&img, &exact());
    let despeckled = trace(
        &img,
        &TraceOptions {
            despeckle: true,
            ..exact()
        },
    );
    assert_eq!(to_svg(&despeckled), to_svg(&plain));
    assert!(
        plain
            .layers
            .iter()
            .any(|l| l.color == Rgba::new(255, 0, 0, 255) && l.area == 1)
    );
}
