//! Arbitrary-angle rotation for pixel art: nearest neighbour and RotSprite.
//!
//! Both methods sample the source at each output pixel's inverse-rotated
//! centre, so every output pixel is a source colour or fully transparent.
//! RotSprite first upscales ×8 with three Scale2x passes and samples that
//! (block-centre rule, `docs/specs/pincel.md` §15, 2026-10-29); it never
//! builds a ×8 rotated buffer. Positive angles turn clockwise. Exact
//! quarter turns take the [`Raster::oriented`] path for either method.
//! See `docs/specs/rotsprite.md`.

use super::transform_support::{Orientation, Raster};

/// Largest input width or height RotSprite accepts; the ×8 source of a
/// 512 × 512 input is 4096² pixels (`docs/specs/rotsprite.md` AC-09).
pub const ROTSPRITE_MAX_SIDE: u32 = 512;

/// How an arbitrary-angle rotation samples the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RotateMethod {
    /// Plain nearest-neighbour sampling: fast, jaggy lines.
    #[default]
    Nearest,
    /// Scale2x ×3 then nearest-neighbour sampling of the ×8 source: keeps
    /// lines clean and adds no colours.
    RotSprite,
}

impl RotateMethod {
    /// Stable snake_case wire name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::RotSprite => "rotsprite",
        }
    }

    /// Inverse of [`RotateMethod::name`].
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Nearest, Self::RotSprite]
            .into_iter()
            .find(|m| m.name() == name)
    }
}

/// A rotated raster and where its top-left lands relative to the source's
/// top-left (both share the same centre).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rotated {
    pub raster: Raster,
    pub offset: (i64, i64),
}

/// Map any angle in degrees into `(-180, 180]`.
pub(crate) fn normalize_angle(angle_deg: i32) -> i32 {
    let a = angle_deg.rem_euclid(360);
    if a > 180 { a - 360 } else { a }
}

/// Rotate `src` clockwise by `angle_deg` with `method`. The result is
/// never cropped: it is as large as the rotated source needs.
pub(crate) fn rotate(src: &Raster, angle_deg: i32, method: RotateMethod) -> Rotated {
    match method {
        RotateMethod::Nearest => rotate_nearest(src, angle_deg),
        RotateMethod::RotSprite => rotsprite(src, angle_deg),
    }
}

/// Nearest-neighbour rotation of `src` by `angle_deg` (clockwise).
pub(crate) fn rotate_nearest(src: &Raster, angle_deg: i32) -> Rotated {
    let src = clear_transparent(src);
    let angle = normalize_angle(angle_deg);
    quarter_turn(&src, angle)
        .unwrap_or_else(|| sample_rotation(src.width, src.height, angle, 1, &src))
}

/// RotSprite rotation of `src` by `angle_deg` (clockwise).
pub(crate) fn rotsprite(src: &Raster, angle_deg: i32) -> Rotated {
    let src = clear_transparent(src);
    let angle = normalize_angle(angle_deg);
    quarter_turn(&src, angle).unwrap_or_else(|| {
        let big = scale2x(&scale2x(&scale2x(&src)));
        sample_rotation(src.width, src.height, angle, 8, &big)
    })
}

/// One Scale2x (EPX) pass: `w × h` → `2w × 2h`. Neighbours outside the
/// raster count as the centre pixel.
pub(crate) fn scale2x(src: &Raster) -> Raster {
    let (w, h) = (i64::from(src.width), i64::from(src.height));
    let mut out = Raster::transparent(src.width * 2, src.height * 2);
    if w == 0 || h == 0 {
        return out;
    }
    let at = |x: i64, y: i64| src.get(x.clamp(0, w - 1) as u32, y.clamp(0, h - 1) as u32);
    for y in 0..h {
        for x in 0..w {
            let e = at(x, y);
            let b = at(x, y - 1);
            let d = at(x - 1, y);
            let f = at(x + 1, y);
            let hh = at(x, y + 1);
            let (e0, e1, e2, e3) = if b != hh && d != f {
                (
                    if d == b { d } else { e },
                    if b == f { f } else { e },
                    if d == hh { d } else { e },
                    if hh == f { f } else { e },
                )
            } else {
                (e, e, e, e)
            };
            let (ox, oy) = (x as u32 * 2, y as u32 * 2);
            out.put(ox, oy, e0);
            out.put(ox + 1, oy, e1);
            out.put(ox, oy + 1, e2);
            out.put(ox + 1, oy + 1, e3);
        }
    }
    out
}

/// Fully transparent pixels become `[0, 0, 0, 0]`, so Scale2x treats every
/// transparent pixel as one colour and edges don't bleed.
fn clear_transparent(src: &Raster) -> Raster {
    let mut out = src.clone();
    for px in out.data.chunks_mut(4) {
        if px[3] == 0 {
            px.copy_from_slice(&[0, 0, 0, 0]);
        }
    }
    out
}

/// The exact path for 0°, ±90° and 180°.
fn quarter_turn(src: &Raster, angle: i32) -> Option<Rotated> {
    let raster = match angle {
        0 => src.clone(),
        90 => src.oriented(Orientation::Rotate90Cw),
        -90 => src.oriented(Orientation::Rotate90Ccw),
        180 => src.oriented(Orientation::Rotate180),
        _ => return None,
    };
    let offset = centred_offset(src, &raster);
    Some(Rotated { raster, offset })
}

/// Top-left of `out` relative to `src` when both share a centre.
fn centred_offset(src: &Raster, out: &Raster) -> (i64, i64) {
    (
        (i64::from(src.width) - i64::from(out.width)) / 2,
        (i64::from(src.height) - i64::from(out.height)) / 2,
    )
}

/// Output side for a rotated extent: covers the extent and keeps the
/// source side's parity, so the output pixel centres line up with the
/// source centre (a lone pixel survives any angle).
fn grown_side(extent: f64, side: u32) -> u32 {
    let n = (extent - 1e-9).ceil().max(1.0) as u32;
    if (n + side) % 2 == 1 { n + 1 } else { n }
}

/// Sample a `w × h` source (stored ×`scale` in `big`) at every output
/// pixel's inverse-rotated centre.
fn sample_rotation(w: u32, h: u32, angle: i32, scale: u32, big: &Raster) -> Rotated {
    if w == 0 || h == 0 {
        return Rotated {
            raster: Raster::transparent(w, h),
            offset: (0, 0),
        };
    }
    let (sin, cos) = f64::from(angle).to_radians().sin_cos();
    let (wf, hf) = (f64::from(w), f64::from(h));
    let nw = grown_side(wf * cos.abs() + hf * sin.abs(), w);
    let nh = grown_side(wf * sin.abs() + hf * cos.abs(), h);
    let mut out = Raster::transparent(nw, nh);
    let (half_nw, half_nh) = (f64::from(nw) / 2.0, f64::from(nh) / 2.0);
    let s = f64::from(scale);
    for oy in 0..nh {
        let dy = f64::from(oy) + 0.5 - half_nh;
        for ox in 0..nw {
            let dx = f64::from(ox) + 0.5 - half_nw;
            let sx = (dx * cos + dy * sin + wf / 2.0) * s;
            let sy = (-dx * sin + dy * cos + hf / 2.0) * s;
            if sx < 0.0 || sy < 0.0 {
                continue;
            }
            let (ix, iy) = (sx.floor() as u64, sy.floor() as u64);
            if ix >= u64::from(big.width) || iy >= u64::from(big.height) {
                continue;
            }
            out.put(ox, oy, big.get(ix as u32, iy as u32));
        }
    }
    let offset = (
        (i64::from(w) - i64::from(nw)) / 2,
        (i64::from(h) - i64::from(nh)) / 2,
    );
    Rotated {
        raster: out,
        offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: [u8; 4] = [255, 0, 0, 255];
    const T: [u8; 4] = [0, 0, 0, 0];

    fn raster(w: u32, h: u32, px: &[[u8; 4]]) -> Raster {
        let mut r = Raster::transparent(w, h);
        for (i, p) in px.iter().enumerate() {
            r.put(i as u32 % w, i as u32 / w, *p);
        }
        r
    }

    /// Deterministic xorshift so the property loops need no extra crate.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// Small palette, transparent included, as pixel art uses it.
    const PALETTE: [[u8; 4]; 5] = [
        [0, 0, 0, 0],
        [20, 12, 28, 255],
        [210, 125, 44, 255],
        [109, 170, 44, 255],
        [89, 125, 206, 128],
    ];

    fn random_raster(rng: &mut Rng, max_side: u64) -> Raster {
        let w = 1 + rng.below(max_side) as u32;
        let h = 1 + rng.below(max_side) as u32;
        let mut r = Raster::transparent(w, h);
        for y in 0..h {
            for x in 0..w {
                r.put(x, y, PALETTE[rng.below(PALETTE.len() as u64) as usize]);
            }
        }
        r
    }

    fn opaque_count(r: &Raster) -> usize {
        r.data.chunks(4).filter(|p| p[3] > 0).count()
    }

    const METHODS: [RotateMethod; 2] = [RotateMethod::Nearest, RotateMethod::RotSprite];

    #[test]
    fn scale2x_matches_reference() {
        // 3×3 diagonal → 6×6, worked out by hand from the EPX rules.
        let src = raster(3, 3, &[R, T, T, T, R, T, T, T, R]);
        #[rustfmt::skip]
        let expected = raster(6, 6, &[
            R, R, T, T, T, T,
            R, T, R, T, T, T,
            T, R, R, R, T, T,
            T, T, R, R, R, T,
            T, T, T, R, T, R,
            T, T, T, T, R, R,
        ]);
        assert_eq!(scale2x(&src), expected);
    }

    #[test]
    fn rotate_by_zero_or_full_turns_is_identity_for_both_methods() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for _ in 0..40 {
            let src = random_raster(&mut rng, 9);
            for method in METHODS {
                for angle in [0, 360, -360, 720] {
                    let out = rotate(&src, angle, method);
                    assert_eq!(out.raster, src, "{method:?} {angle}°");
                    assert_eq!(out.offset, (0, 0), "{method:?} {angle}°");
                }
            }
        }
    }

    #[test]
    fn rotsprite_90_matches_orient_rotate90() {
        let mut rng = Rng(0xD1B5_4A32_D192_ED03);
        let mut sources = vec![raster(3, 3, &[R, T, T, T, R, T, T, T, R])];
        sources.extend((0..30).map(|_| random_raster(&mut rng, 9)));
        for src in sources {
            let out = rotsprite(&src, 90);
            assert_eq!(out.raster, src.oriented(Orientation::Rotate90Cw));
            let (w, h) = (i64::from(src.width), i64::from(src.height));
            assert_eq!(out.offset, ((w - h) / 2, (h - w) / 2));
        }
    }

    #[test]
    fn rotation_keeps_palette_no_new_colors() {
        let mut rng = Rng(0x2545_F491_4F6C_DD1D);
        for _ in 0..12 {
            let src = random_raster(&mut rng, 10);
            let colors: Vec<[u8; 4]> = src
                .data
                .chunks(4)
                .map(|p| [p[0], p[1], p[2], p[3]])
                .collect();
            for method in METHODS {
                for angle in [1, 17, 30, 45, 63, 90, 120, 180, -75, -179] {
                    let out = rotate(&src, angle, method);
                    for p in out.raster.data.chunks(4) {
                        let px = [p[0], p[1], p[2], p[3]];
                        assert!(
                            px[3] == 0 || colors.contains(&px),
                            "{method:?} {angle}°: new colour {px:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn rotation_grows_bounds_and_never_crops() {
        for (w, h) in [(5u32, 3u32), (8, 8), (1, 7), (12, 4)] {
            let src = raster(w, h, &vec![R; (w * h) as usize]);
            for method in METHODS {
                for angle in [10, 30, 45, 60, 89, 135, -45, -120] {
                    let out = rotate(&src, angle, method);
                    let (sin, cos) = f64::from(angle).to_radians().sin_cos();
                    let (cx, cy) = (f64::from(w) / 2.0, f64::from(h) / 2.0);
                    // Every source pixel centre, turned clockwise about the
                    // source centre, lands inside the output box.
                    for y in 0..h {
                        for x in 0..w {
                            let (dx, dy) = (f64::from(x) + 0.5 - cx, f64::from(y) + 0.5 - cy);
                            let rx = dx * cos - dy * sin + cx - out.offset.0 as f64;
                            let ry = dx * sin + dy * cos + cy - out.offset.1 as f64;
                            assert!(
                                rx >= 0.0
                                    && ry >= 0.0
                                    && rx < f64::from(out.raster.width)
                                    && ry < f64::from(out.raster.height),
                                "{method:?} {angle}° {w}x{h}: ({x},{y}) cropped"
                            );
                        }
                    }
                    // Area survives too (a 1-px line legitimately thins
                    // to its diagonal length, so only solid blocks count).
                    let (before, after) =
                        (opaque_count(&src) as f64, opaque_count(&out.raster) as f64);
                    assert!(
                        w.min(h) < 3 || after >= before * 0.8,
                        "{method:?} {angle}° {w}x{h}: {after} of {before} pixels left"
                    );
                }
            }
        }
    }

    #[test]
    fn single_pixel_survives_45_degrees() {
        let src = raster(1, 1, &[R]);
        for method in METHODS {
            let out = rotate(&src, 45, method);
            assert_eq!(opaque_count(&out.raster), 1, "{method:?}");
            let centre = (out.raster.width / 2, out.raster.height / 2);
            assert_eq!(out.raster.get(centre.0, centre.1), R, "{method:?}");
            assert_eq!(
                (
                    out.offset.0 * 2 + i64::from(out.raster.width),
                    out.offset.1 * 2 + i64::from(out.raster.height)
                ),
                (1, 1),
                "{method:?}: centred on the source pixel"
            );
        }
    }

    #[test]
    fn fully_transparent_stays_transparent() {
        let src = Raster::transparent(6, 4);
        for method in METHODS {
            let out = rotate(&src, 33, method);
            assert_eq!(opaque_count(&out.raster), 0, "{method:?}");
        }
    }

    #[test]
    fn angle_180_and_minus_180_match() {
        let mut rng = Rng(0x0BAD_5EED_1234_5678);
        let src = random_raster(&mut rng, 8);
        for method in METHODS {
            assert_eq!(rotate(&src, 180, method), rotate(&src, -180, method));
            assert_eq!(rotate(&src, 390, method), rotate(&src, 30, method));
        }
    }

    #[test]
    fn rotate_method_names_round_trip() {
        for m in METHODS {
            assert_eq!(RotateMethod::from_name(m.name()), Some(m));
        }
        assert_eq!(RotateMethod::from_name("bilinear"), None);
    }
}
