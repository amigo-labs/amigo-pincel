//! docs/specs/outline-effect.md: Object ▸ Outline Object through the wire
//! format both cores use — `outline`: `[place, shape, thickness, color]`
//! with place 0 = outside, 1 = inside, 2 = both, shape 0 = circle,
//! 1 = square and the colour as one `0xRRGGBBAA` slot.

use pincel_effects::{EFFECT_NAMES, EffectError, EffectImage, EffectSpec};

const OUTSIDE: f64 = 0.0;
const INSIDE: f64 = 1.0;
const BOTH: f64 = 2.0;
const CIRCLE: f64 = 0.0;
const SQUARE: f64 = 1.0;

/// The outline colour of most tests: a semi-transparent orange, so a test
/// also sees that alpha is taken exactly (AC-06).
const COLOR: [u8; 4] = [0xFF, 0x80, 0x10, 0xC0];
const COLOR_SLOT: f64 = 0xFF80_10C0_u32 as f64;

const CLEAR: [u8; 4] = [0, 0, 0, 0];
const INK: [u8; 4] = [10, 20, 30, 255];

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

fn outline(src: &EffectImage, place: f64, shape: f64, thickness: f64) -> EffectImage {
    EffectSpec::from_params("outline", &[place, shape, thickness, COLOR_SLOT])
        .expect("outline parses")
        .run(src)
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

/// A sprite with a few opaque pixels in an irregular shape, an alpha-1
/// pixel and room around it: every metric and place differs on it.
fn sprite() -> EffectImage {
    image(12, 11, |x, y| match (x, y) {
        (4..=6, 4..=6) => INK,
        (7, 5) => [200, 0, 0, 255],
        (5, 3) => [0, 200, 0, 1],
        _ => CLEAR,
    })
}

fn opaque(img: &EffectImage, x: i64, y: i64) -> bool {
    x >= 0
        && y >= 0
        && x < i64::from(img.width())
        && y < i64::from(img.height())
        && px(img, x as u32, y as u32)[3] > 0
}

/// Distance from (x, y) to the nearest pixel whose opacity is `want`
/// (pixels beyond the edge are transparent), by the shape's metric; a
/// brute-force reference for AC-03 – AC-05.
fn distance_to(img: &EffectImage, x: i64, y: i64, want: bool, square: bool) -> i64 {
    let (w, h) = (i64::from(img.width()), i64::from(img.height()));
    let mut best = i64::MAX;
    for oy in -10..h + 10 {
        for ox in -10..w + 10 {
            if opaque(img, ox, oy) != want {
                continue;
            }
            let (dx, dy) = ((ox - x).abs(), (oy - y).abs());
            let d = if square { dx.max(dy) } else { dx + dy };
            best = best.min(d);
        }
    }
    best
}

/// The pixels the reference says the outline colours.
fn expected(img: &EffectImage, place: f64, square: bool, t: i64) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            let (xi, yi) = (i64::from(x), i64::from(y));
            let is_opaque = opaque(img, xi, yi);
            let outside = !is_opaque && distance_to(img, xi, yi, true, square) <= t;
            let inside = is_opaque && distance_to(img, xi, yi, false, square) <= t;
            let hit = match place {
                p if p == OUTSIDE => outside,
                p if p == INSIDE => inside,
                _ => outside || inside,
            };
            // A pixel that already has the outline colour does not change.
            if hit && px(img, x, y) != COLOR {
                out.push((x, y));
            }
        }
    }
    out
}

fn check(place: f64, shape: f64) {
    let src = sprite();
    for t in 1..=8 {
        let out = outline(&src, place, shape, f64::from(t));
        let want = expected(&src, place, shape == SQUARE, i64::from(t));
        assert_eq!(
            changed(&src, &out),
            want,
            "place {place}, shape {shape}, t {t}"
        );
        for &(x, y) in &want {
            assert_eq!(px(&out, x, y), COLOR, "({x}, {y}) takes the colour exactly");
        }
    }
}

#[test]
fn outline_is_a_wire_effect_named_outline_object() {
    assert!(EFFECT_NAMES.contains(&"outline"));
    let spec = EffectSpec::from_params("outline", &[0.0, 1.0, 1.0, 255.0]).expect("parses");
    assert_eq!(spec.label(), "Outline Object");
}

/// AC-03: Outside / Square colours every transparent pixel within
/// Chebyshev distance t of an opaque one, nothing else.
#[test]
fn outline_outside_square_colours_the_chebyshev_ring() {
    check(OUTSIDE, SQUARE);
    // A single opaque pixel with t = 1 gets its 8 neighbours.
    let src = image(5, 5, |x, y| if (x, y) == (2, 2) { INK } else { CLEAR });
    let out = outline(&src, OUTSIDE, SQUARE, 1.0);
    let ring: Vec<(u32, u32)> = (1..=3)
        .flat_map(|y| (1..=3).map(move |x| (x, y)))
        .filter(|&p| p != (2, 2))
        .collect();
    assert_eq!(changed(&src, &out), ring);
    assert_eq!(px(&out, 2, 2), INK);
}

/// AC-04: Outside / Circle uses the Manhattan distance.
#[test]
fn outline_outside_circle_colours_the_manhattan_ring() {
    check(OUTSIDE, CIRCLE);
    let src = image(5, 5, |x, y| if (x, y) == (2, 2) { INK } else { CLEAR });
    let out = outline(&src, OUTSIDE, CIRCLE, 1.0);
    assert_eq!(changed(&src, &out), vec![(2, 1), (1, 2), (3, 2), (2, 3)]);
}

/// AC-05: Inside recolours opaque pixels within t of a transparent one.
#[test]
fn outline_inside_recolours_the_opaque_border() {
    check(INSIDE, SQUARE);
    check(INSIDE, CIRCLE);
}

/// AC-13: Both is the union of Outside and Inside.
#[test]
fn outline_both_is_the_union_of_inside_and_outside() {
    check(BOTH, SQUARE);
    check(BOTH, CIRCLE);
    let src = sprite();
    for shape in [CIRCLE, SQUARE] {
        for t in [1.0, 2.0, 5.0] {
            let mut union = changed(&src, &outline(&src, OUTSIDE, shape, t));
            union.extend(changed(&src, &outline(&src, INSIDE, shape, t)));
            union.sort_by_key(|&(x, y)| (y, x));
            assert_eq!(changed(&src, &outline(&src, BOTH, shape, t)), union);
        }
    }
}

/// AC-02: alpha > 0 is opaque, beyond the edge is transparent, and the
/// result never leaves the image.
#[test]
fn outline_edges_and_alpha_one_follow_the_opaque_rule() {
    // An alpha-1 pixel counts as opaque: Outside does not cover it.
    let src = image(3, 3, |x, y| match (x, y) {
        (1, 1) => [9, 9, 9, 1],
        _ => CLEAR,
    });
    let out = outline(&src, OUTSIDE, SQUARE, 1.0);
    assert_eq!(px(&out, 1, 1), [9, 9, 9, 1]);
    assert_eq!(changed(&src, &out).len(), 8);
    // Inside may recolour it.
    let out = outline(&src, INSIDE, SQUARE, 1.0);
    assert_eq!(changed(&src, &out), vec![(1, 1)]);

    // A fully opaque layer: Outside changes nothing, Inside t = 1 only
    // the border ring (the edge counts as transparent).
    let full = image(5, 4, |_, _| INK);
    assert_eq!(outline(&full, OUTSIDE, SQUARE, 8.0), full);
    let ring = outline(&full, INSIDE, CIRCLE, 1.0);
    for y in 0..4 {
        for x in 0..5 {
            let border = x == 0 || y == 0 || x == 4 || y == 3;
            assert_eq!(
                px(&ring, x, y),
                if border { COLOR } else { INK },
                "({x}, {y})"
            );
        }
    }

    // An opaque pixel on the edge: outline pixels only inside the image,
    // nothing wraps around to the far side.
    let corner = image(6, 6, |x, y| if (x, y) == (0, 0) { INK } else { CLEAR });
    let out = outline(&corner, OUTSIDE, SQUARE, 1.0);
    assert_eq!((out.width(), out.height()), (6, 6));
    assert_eq!(changed(&corner, &out), vec![(1, 0), (0, 1), (1, 1)]);

    // A layer with no opaque pixel stays as it is, in every place.
    let empty = image(4, 4, |_, _| CLEAR);
    for place in [OUTSIDE, INSIDE, BOTH] {
        assert_eq!(outline(&empty, place, SQUARE, 3.0), empty);
    }
}

/// Edge case: thickness 3, Square, single pixel colours a 7×7 block.
#[test]
fn outline_thickness_three_square_colours_a_seven_block() {
    let src = image(9, 9, |x, y| if (x, y) == (4, 4) { INK } else { CLEAR });
    let out = outline(&src, OUTSIDE, SQUARE, 3.0);
    for y in 0..9 {
        for x in 0..9 {
            let want = if (x, y) == (4, 4) {
                INK
            } else if (1..=7).contains(&x) && (1..=7).contains(&y) {
                COLOR
            } else {
                CLEAR
            };
            assert_eq!(px(&out, x, y), want, "({x}, {y})");
        }
    }
}

/// AC-06: the colour slot decodes `0xRRGGBBAA` exactly.
#[test]
fn outline_colour_slot_is_rrggbbaa() {
    let src = image(3, 1, |x, _| if x == 0 { INK } else { CLEAR });
    for (slot, rgba) in [
        (0x0000_0000_u32, [0, 0, 0, 0]),
        (0x1234_5678, [0x12, 0x34, 0x56, 0x78]),
        (u32::MAX, [255, 255, 255, 255]),
    ] {
        let out = EffectSpec::from_params("outline", &[OUTSIDE, SQUARE, 1.0, f64::from(slot)])
            .expect("parses")
            .run(&src);
        assert_eq!(px(&out, 1, 0), rgba, "{slot:#010x}");
        assert_eq!(px(&out, 2, 0), CLEAR);
    }
}

/// AC-12: thickness clamps to 1–8; a wrong count or a bad colour slot is
/// an error.
#[test]
fn outline_wire_clamps_thickness_and_rejects_bad_slots() {
    let src = image(21, 21, |x, y| if (x, y) == (10, 10) { INK } else { CLEAR });
    let run = |t: f64| outline(&src, OUTSIDE, SQUARE, t);
    for (given, clamped) in [
        (0.0, 1.0),
        (-5.0, 1.0),
        (f64::NAN, 1.0),
        (9.0, 8.0),
        (100.0, 8.0),
        (f64::INFINITY, 1.0),
    ] {
        assert_eq!(run(given), run(clamped), "thickness {given}");
    }
    assert_ne!(run(1.0), run(2.0));

    for wrong in [
        &[][..],
        &[0.0, 1.0, 1.0][..],
        &[0.0, 1.0, 1.0, 255.0, 0.0][..],
    ] {
        assert!(
            matches!(
                EffectSpec::from_params("outline", wrong),
                Err(EffectError::ParamCount { expected: 4, .. })
            ),
            "{wrong:?}"
        );
    }

    for bad in [
        -1.0,
        0.5,
        4_294_967_296.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        assert_eq!(
            EffectSpec::from_params("outline", &[0.0, 1.0, 1.0, bad]),
            Err(EffectError::ColorSlot {
                effect: "outline".into(),
                slot: 3,
            }),
            "colour slot {bad}"
        );
    }
    for good in [0.0, 4_294_967_295.0] {
        assert!(EffectSpec::from_params("outline", &[0.0, 1.0, 1.0, good]).is_ok());
    }
}
