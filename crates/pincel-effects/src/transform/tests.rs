//! Rotate / Zoom and the affine resampler
//! (docs/specs/image-transform-handles-rotate-zoom.md AC-04 – AC-06).

use super::*;
use crate::effect::Effect;

const CLEAR: [u8; 4] = [0, 0, 0, 0];
const QUALITIES: [Quality; 3] = [Quality::Nearest, Quality::Bilinear, Quality::Bicubic];

fn image(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> EffectImage {
    let data = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .flat_map(|(x, y)| f(x, y))
        .collect();
    EffectImage::from_rgba8(w, h, data).expect("size matches")
}

fn px(img: &EffectImage, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.width() + x) * 4) as usize;
    img.data()[i..i + 4].try_into().expect("four bytes")
}

/// A distinct colour per pixel, some semi-transparent, one transparent
/// with a colour (which premultiplying would lose).
fn varied(w: u32, h: u32) -> EffectImage {
    image(w, h, |x, y| {
        let i = (y * w + x) as u8;
        let a = match i % 4 {
            0 => 255,
            1 => 128,
            2 => 7,
            _ => 0,
        };
        [i.wrapping_mul(37), 255 - i, i.wrapping_mul(11), a]
    })
}

fn rz(angle: f64, pan_x: f64, pan_y: f64, zoom: f64, q: Quality) -> RotateZoom {
    RotateZoom::new(angle, pan_x, pan_y, zoom, q)
}

/// Layer ▸ Rotate 90° CW's mapping: `(x, y)` lands on `(h - 1 - y, x)`.
fn quarter_cw(src: &EffectImage) -> EffectImage {
    let (w, h) = (src.width(), src.height());
    image(h, w, |x, y| px(src, y, h - 1 - x))
}

#[test]
fn rotate_zoom_identity_leaves_every_pixel_unchanged() {
    for (w, h) in [(4, 4), (5, 3), (1, 1), (6, 7)] {
        let src = varied(w, h);
        for q in QUALITIES {
            let out = rz(0.0, 0.0, 0.0, 1.0, q).apply(&src);
            assert_eq!(out, src, "{w} x {h}, {q:?}");
        }
    }
}

#[test]
fn resample_identity_map_copies_every_pixel_exactly() {
    for (w, h) in [(4, 4), (5, 3)] {
        let src = varied(w, h);
        for q in QUALITIES {
            let out = resample(&src, w, h, &Affine::IDENTITY, q);
            assert_eq!(out, src, "{w} x {h}, {q:?}");
        }
    }
}

#[test]
fn rotate_zoom_quarter_turn_is_clockwise_about_the_centre() {
    // Even and odd sizes: the odd centre is the middle pixel's centre.
    for size in [4, 5] {
        let src = varied(size, size);
        let out = rz(90.0, 0.0, 0.0, 1.0, Quality::Nearest).apply(&src);
        assert_eq!(out, quarter_cw(&src), "{size} x {size}");
    }
    // A marker right of the top edge's middle moves to the right edge.
    let src = image(4, 4, |x, y| {
        if (x, y) == (2, 0) {
            [255, 0, 0, 255]
        } else {
            CLEAR
        }
    });
    let out = rz(90.0, 0.0, 0.0, 1.0, Quality::Nearest).apply(&src);
    assert_eq!(px(&out, 3, 2), [255, 0, 0, 255]);
}

#[test]
fn rotate_zoom_half_turn_either_way_is_the_same() {
    let src = varied(5, 4);
    for q in QUALITIES {
        let cw = rz(180.0, 0.0, 0.0, 1.0, q).apply(&src);
        let ccw = rz(-180.0, 0.0, 0.0, 1.0, q).apply(&src);
        assert_eq!(cw, ccw, "{q:?}");
        assert_eq!(px(&cw, 0, 0), px(&src, 4, 3), "{q:?}");
    }
}

#[test]
fn rotate_zoom_arbitrary_angle_turns_clockwise_about_the_centre() {
    // Each source pixel carries its own coordinates, so every target pixel
    // names the source pixel it shows (an asymmetric marker everywhere).
    let coords = |n: u32| image(n, n, |x, y| [x as u8 * 30, y as u8 * 30, 200, 255]);
    let source_of = |out: &EffectImage, x: u32, y: u32| {
        let p = px(out, x, y);
        (p[3] != 0).then_some((u32::from(p[0]) / 30, u32::from(p[1]) / 30))
    };
    // Target pixel (x, y) shows source floor(c + R(-angle) · (p - c)) with
    // p = (x + 0.5, y + 0.5), c = (n / 2, n / 2) and
    // R(-angle) · (dx, dy) = (dx cos + dy sin, -dx sin + dy cos), worked by
    // hand; e.g. n = 8, 30°, target (6, 4): d = (2.5, 0.5),
    // source = (4 + 2.165 + 0.25, 4 - 1.25 + 0.433) = (6.415, 3.183) → (6, 3).
    // Every listed source position is at least 0.17 px from a pixel edge,
    // and a negated angle, the angle read as radians or the centre at
    // ((n - 1) / 2, (n - 1) / 2) each show another source pixel there. The
    // top-left corner reads from outside the source and stays transparent.
    // (target pixel, the source pixel it shows or `None` for transparent)
    type Probe = ((u32, u32), Option<(u32, u32)>);
    let cases: [(u32, f64, [Probe; 5]); 4] = [
        (
            8,
            30.0,
            [
                ((3, 0), Some((1, 1))),
                ((5, 0), Some((3, 0))),
                ((6, 4), Some((6, 3))),
                ((3, 7), Some((5, 7))),
                ((0, 0), None),
            ],
        ),
        (
            8,
            45.0,
            [
                ((5, 0), Some((2, 0))),
                ((0, 2), Some((0, 5))),
                ((7, 2), Some((5, 0))),
                ((5, 7), Some((7, 5))),
                ((0, 0), None),
            ],
        ),
        (
            7,
            30.0,
            [
                ((2, 1), Some((1, 2))),
                ((4, 1), Some((3, 1))),
                ((1, 5), Some((2, 6))),
                ((5, 5), Some((6, 4))),
                ((0, 0), None),
            ],
        ),
        (
            7,
            45.0,
            [
                ((3, 0), Some((1, 1))),
                ((6, 3), Some((5, 1))),
                ((1, 5), Some((3, 6))),
                ((4, 5), Some((5, 4))),
                ((0, 0), None),
            ],
        ),
    ];
    for (n, angle, expected) in cases {
        let out = rz(angle, 0.0, 0.0, 1.0, Quality::Nearest).apply(&coords(n));
        for ((x, y), source) in expected {
            assert_eq!(
                source_of(&out, x, y),
                source,
                "{n} x {n}, {angle}°, ({x}, {y})"
            );
        }
    }
}

/// The source column each target column of row 0 shows, read from the red
/// channel (`x · 30` in the sources below); `None` where transparent.
fn columns(out: &EffectImage) -> Vec<Option<u32>> {
    (0..out.width())
        .map(|x| px(out, x, 0))
        .map(|p| (p[3] != 0).then_some(u32::from(p[0]) / 30))
        .collect()
}

#[test]
fn rotate_zoom_zoom_scales_about_the_canvas_centre() {
    let opaque = |w: u32, h: u32| image(w, h, |x, y| [x as u8 * 30, y as u8 * 30, 9, 255]);
    let src = opaque(4, 4);
    let out = rz(0.0, 0.0, 0.0, 2.0, Quality::Nearest).apply(&src);
    // Target x reads source floor(2 + (x + 0.5 - 2) / 2).
    assert_eq!(columns(&out), [Some(1), Some(1), Some(2), Some(2)]);
    assert_eq!(px(&out, 0, 0), px(&src, 1, 1));
    // An odd canvas zooms about its middle pixel's centre.
    let src = opaque(5, 5);
    let out = rz(0.0, 0.0, 0.0, 3.0, Quality::Nearest).apply(&src);
    assert_eq!(px(&out, 2, 2), px(&src, 2, 2));
    assert_eq!(px(&out, 0, 2), px(&src, 1, 2));
    assert_eq!(px(&out, 4, 2), px(&src, 3, 2));
    assert_eq!(px(&out, 1, 2), px(&src, 2, 2));
    // Zooming out leaves the border transparent.
    let out = rz(0.0, 0.0, 0.0, 0.5, Quality::Nearest).apply(&opaque(8, 8));
    assert_eq!(px(&out, 0, 0), CLEAR);
    assert_eq!(px(&out, 4, 4)[3], 255);
}

#[test]
fn rotate_zoom_pan_shifts_after_rotation_and_zoom() {
    let src = image(4, 4, |x, y| [x as u8 * 30, y as u8 * 30, 9, 255]);
    // 25 % of 4 px is one pixel, not scaled by the zoom.
    let out = rz(0.0, 25.0, 0.0, 2.0, Quality::Nearest).apply(&src);
    assert_eq!(columns(&out), [Some(0), Some(1), Some(1), Some(2)]);
    // The pan is not rotated: after a quarter turn it still moves right.
    let out = rz(90.0, 25.0, 0.0, 1.0, Quality::Nearest).apply(&src);
    let turned = quarter_cw(&src);
    for y in 0..4 {
        assert_eq!(px(&out, 0, y), CLEAR);
        for x in 1..4 {
            assert_eq!(px(&out, x, y), px(&turned, x - 1, y), "({x}, {y})");
        }
    }
    // Pan Y moves down by its share of the height.
    let src = image(3, 4, |x, y| [x as u8 * 30, y as u8 * 30, 9, 255]);
    let out = rz(0.0, 0.0, 50.0, 1.0, Quality::Nearest).apply(&src);
    for x in 0..3 {
        assert_eq!(px(&out, x, 0), CLEAR);
        assert_eq!(px(&out, x, 1), CLEAR);
        assert_eq!(px(&out, x, 2), px(&src, x, 0));
        assert_eq!(px(&out, x, 3), px(&src, x, 1));
    }
}

#[test]
fn rotate_zoom_pan_by_a_full_side_empties_the_canvas() {
    let src = varied(6, 4);
    for q in QUALITIES {
        for (px_, py) in [(100.0, 0.0), (-100.0, 0.0), (0.0, 100.0), (0.0, -100.0)] {
            let out = rz(0.0, px_, py, 1.0, q).apply(&src);
            assert!(out.data().iter().all(|&b| b == 0), "{q:?} {px_} {py}");
        }
    }
}

#[test]
fn rotate_zoom_sub_pixel_pan_interpolates_and_reads_transparent_outside() {
    let step = [0u8, 0, 0, 255, 255, 255];
    let src = image(6, 1, |x, _| {
        let v = step[x as usize];
        [v, v, v, 255]
    });
    // 12.5 % of 6 px = 0.75 px: pixel 3 reads source centre 2.25.
    let bilinear = rz(0.0, 12.5, 0.0, 1.0, Quality::Bilinear).apply(&src);
    assert_eq!(px(&bilinear, 3, 0), [64, 64, 64, 255]);
    // Catmull-Rom over 0, 0, 255, 255 at t = 0.25: 0.203125 · 255.
    let bicubic = rz(0.0, 12.5, 0.0, 1.0, Quality::Bicubic).apply(&src);
    assert_eq!(px(&bicubic, 3, 0), [52, 52, 52, 255]);
    // Pixel 0 reads source centre -0.75: a quarter of pixel 0, the rest
    // transparent.
    assert_eq!(px(&bilinear, 0, 0), [0, 0, 0, 64]);
    let nearest = rz(0.0, 12.5, 0.0, 1.0, Quality::Nearest).apply(&src);
    assert_eq!(px(&nearest, 0, 0), CLEAR);
    assert_eq!(px(&nearest, 4, 0), px(&src, 3, 0));
}

#[test]
fn rotate_zoom_smooth_qualities_keep_a_solid_interior() {
    let colour = [200, 120, 40, 255];
    let src = image(15, 15, |_, _| colour);
    for q in [Quality::Bilinear, Quality::Bicubic] {
        let out = rz(30.0, 0.0, 0.0, 1.0, q).apply(&src);
        assert_eq!(px(&out, 7, 7), colour, "{q:?}");
        assert_eq!(px(&out, 0, 0), CLEAR, "{q:?}");
    }
}

#[test]
fn rotate_zoom_nearest_keeps_source_colours() {
    let src = varied(7, 5);
    let colours = src.data().as_chunks::<4>().0;
    for angle in [-180.0, -137.3, -45.0, 13.7, 30.0, 90.5, 179.9] {
        for zoom in [0.1, 0.37, 1.0, 2.5, 10.0] {
            for pan in [-33.3, 0.0, 12.5] {
                let out = rz(angle, pan, -pan, zoom, Quality::Nearest).apply(&src);
                for p in out.data().as_chunks::<4>().0 {
                    assert!(
                        p[3] == 0 || colours.contains(p),
                        "{angle} {zoom} {pan}: {p:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn rotate_zoom_tiny_canvas_and_extreme_zoom_do_not_fail() {
    let src = image(1, 1, |_, _| [9, 8, 7, 255]);
    for q in QUALITIES {
        for zoom in [0.1, 10.0] {
            let out = rz(33.0, 0.0, 0.0, zoom, q).apply(&src);
            assert_eq!((out.width(), out.height()), (1, 1));
        }
    }
    let empty = EffectImage::new_transparent(0, 0);
    assert_eq!(
        rz(45.0, 0.0, 0.0, 2.0, Quality::Bicubic).apply(&empty),
        empty
    );
}

#[test]
fn rotate_zoom_new_clamps_and_reads_non_finite_as_zero() {
    let r = RotateZoom::new(500.0, -300.0, 300.0, 50.0, Quality::Bicubic);
    assert_eq!(
        (r.angle, r.pan_x, r.pan_y, r.zoom),
        (180.0, -100.0, 100.0, 10.0)
    );
    let r = RotateZoom::new(f64::NAN, f64::INFINITY, 0.0, f64::NAN, Quality::Nearest);
    assert_eq!((r.angle, r.pan_x, r.zoom), (0.0, 0.0, 0.1));
    assert_eq!(Quality::from_index(0), Quality::Nearest);
    assert_eq!(Quality::from_index(1), Quality::Bilinear);
    assert_eq!(Quality::from_index(2), Quality::Bicubic);
}

#[test]
fn resample_reads_any_source_size_into_any_target_size() {
    // A 2 x 2 patch placed at (1, 1) of a 4 x 3 target.
    let patch = image(2, 2, |x, y| {
        [x as u8 * 100 + 50, y as u8 * 100 + 50, 1, 255]
    });
    let place = Affine::rotate_zoom((0.0, 0.0), 0.0, 1.0, (1.0, 1.0));
    let inverse = place.inverse().expect("invertible");
    for q in QUALITIES {
        let out = resample(&patch, 4, 3, &inverse, q);
        assert_eq!((out.width(), out.height()), (4, 3));
        assert_eq!(px(&out, 1, 1), px(&patch, 0, 0), "{q:?}");
        assert_eq!(px(&out, 2, 2), px(&patch, 1, 1), "{q:?}");
        assert_eq!(px(&out, 0, 0), CLEAR, "{q:?}");
        assert_eq!(px(&out, 3, 0), CLEAR, "{q:?}");
    }
}

#[test]
fn affine_inverse_undoes_the_map_and_refuses_a_collapse() {
    let m = Affine::rotate_zoom((3.5, 2.0), 37.0, 1.7, (0.25, -4.0));
    let inv = m.inverse().expect("invertible");
    let (x, y) = m.apply(1.25, 6.5);
    let (bx, by) = inv.apply(x, y);
    assert!((bx - 1.25).abs() < 1e-9 && (by - 6.5).abs() < 1e-9);
    let flat = Affine::rotate_zoom((0.0, 0.0), 0.0, 0.0, (0.0, 0.0));
    assert_eq!(flat.inverse(), None);
}
