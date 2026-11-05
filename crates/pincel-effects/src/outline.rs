//! Outline (`docs/specs/outline-effect.md`): a hard pixel-art outline in one
//! colour around the opaque pixels of a layer.
//!
//! A pixel is opaque when its alpha is above 0; pixels beyond the buffer's
//! edge count as transparent (AC-02). *Outside* colours the transparent
//! pixels within `thickness` of an opaque one, *Inside* the opaque pixels
//! within `thickness` of a transparent one, *Both* the union of the two.
//! Distances use the shape's metric: Manhattan for [`OutlineShape::Circle`]
//! (4-neighbour), Chebyshev for [`OutlineShape::Square`] (8-neighbour).

use crate::effect::Effect;
use crate::image::EffectImage;

/// Smallest outline thickness in pixels.
pub const MIN_THICKNESS: u32 = 1;
/// Largest outline thickness in pixels.
pub const MAX_THICKNESS: u32 = 8;

/// Which side of the sprite's edge the outline is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutlinePlace {
    /// Transparent pixels next to the sprite.
    #[default]
    Outside,
    /// The sprite's own edge pixels.
    Inside,
    /// Both sides of the edge.
    Both,
}

/// The neighbourhood that measures the outline's distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutlineShape {
    /// 4-neighbour, Manhattan distance: a diamond around a single pixel.
    Circle,
    /// 8-neighbour, Chebyshev distance: a square around a single pixel.
    #[default]
    Square,
}

/// The Outline effect: `place`, `shape`, `thickness` (clamped to
/// [`MIN_THICKNESS`]..=[`MAX_THICKNESS`]) and a straight-alpha RGBA colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outline {
    place: OutlinePlace,
    shape: OutlineShape,
    thickness: u32,
    color: [u8; 4],
}

impl Outline {
    /// Builds an outline; `thickness` is clamped to 1–8.
    pub fn new(place: OutlinePlace, shape: OutlineShape, thickness: u32, color: [u8; 4]) -> Self {
        Self {
            place,
            shape,
            thickness: thickness.clamp(MIN_THICKNESS, MAX_THICKNESS),
            color,
        }
    }

    /// The clamped thickness in pixels.
    pub fn thickness(&self) -> u32 {
        self.thickness
    }

    /// The outline colour as straight-alpha `[r, g, b, a]`.
    pub fn color(&self) -> [u8; 4] {
        self.color
    }
}

/// A neighbour offset `(dx, dy)` of the chamfer transform.
type Tap = (isize, isize);

/// Distance cap: anything farther than the largest thickness is "far".
const FAR: u8 = MAX_THICKNESS as u8 + 1;

/// Distance from every pixel to the nearest pixel with `source[i] == true`,
/// capped at [`FAR`]. Two-pass chamfer transform with unit weights, exact
/// for the city-block (`eight == false`) and chessboard (`eight == true`)
/// metrics. With `edge_is_source` the pixels beyond the buffer's edge count
/// as sources (a one-pixel source border is enough to make that exact).
fn distances(source: &[bool], w: usize, h: usize, eight: bool, edge_is_source: bool) -> Vec<u8> {
    let mut d: Vec<u8> = source.iter().map(|&s| if s { 0 } else { FAR }).collect();
    let edge = if edge_is_source { 0 } else { FAR };
    let at = |d: &[u8], x: isize, y: isize| -> u8 {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            edge
        } else {
            d[y as usize * w + x as usize]
        }
    };
    let relax = |d: &mut Vec<u8>, x: usize, y: usize, taps: &[Tap]| {
        let i = y * w + x;
        let (xi, yi) = (x as isize, y as isize);
        let best = taps
            .iter()
            .map(|&(dx, dy)| at(d, xi + dx, yi + dy))
            .min()
            .unwrap_or(FAR);
        d[i] = d[i].min(best.saturating_add(1)).min(FAR);
    };
    let (fwd, bwd): (&[Tap], &[Tap]) = if eight {
        (
            &[(-1, 0), (-1, -1), (0, -1), (1, -1)],
            &[(1, 0), (1, 1), (0, 1), (-1, 1)],
        )
    } else {
        (&[(-1, 0), (0, -1)], &[(1, 0), (0, 1)])
    };
    for y in 0..h {
        for x in 0..w {
            relax(&mut d, x, y, fwd);
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            relax(&mut d, x, y, bwd);
        }
    }
    d
}

impl Effect for Outline {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let px = src.data().as_chunks::<4>().0;
        let opaque: Vec<bool> = px.iter().map(|p| p[3] > 0).collect();
        let eight = self.shape == OutlineShape::Square;
        // `thickness` is clamped to 1..=8, so it fits a u8.
        let t = self.thickness as u8;
        let outside = matches!(self.place, OutlinePlace::Outside | OutlinePlace::Both)
            .then(|| distances(&opaque, w, h, eight, false));
        let inside = matches!(self.place, OutlinePlace::Inside | OutlinePlace::Both).then(|| {
            let transparent: Vec<bool> = opaque.iter().map(|&o| !o).collect();
            distances(&transparent, w, h, eight, true)
        });
        let mut data = src.data().to_vec();
        for (i, (dst, &is_opaque)) in data
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(&opaque)
            .enumerate()
        {
            let hit = if is_opaque {
                inside.as_ref().is_some_and(|d| d[i] <= t)
            } else {
                outside.as_ref().is_some_and(|d| d[i] <= t)
            };
            if hit {
                *dst = self.color;
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), data)
            .expect("a copy of src's buffer has src's size")
    }

    /// The outline is a pixel-exact edge effect: its thickness is not
    /// scaled for a preview.
    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: [u8; 4] = [255, 0, 0, 255];
    const INK: [u8; 4] = [10, 20, 30, 128];

    /// `w × h` transparent image with `RED` at each of `opaque`.
    fn image(w: u32, h: u32, opaque: &[(u32, u32)]) -> EffectImage {
        let mut data = vec![0u8; (w * h * 4) as usize];
        for &(x, y) in opaque {
            let i = ((y * w + x) * 4) as usize;
            data[i..i + 4].copy_from_slice(&RED);
        }
        EffectImage::from_rgba8(w, h, data).expect("image")
    }

    /// Coordinates whose pixel differs between `a` and `b`.
    fn changed(a: &EffectImage, b: &EffectImage) -> Vec<(u32, u32)> {
        let w = a.width();
        a.data()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(b.data().as_chunks::<4>().0)
            .enumerate()
            .filter(|(_, (p, q))| p != q)
            .map(|(i, _)| (i as u32 % w, i as u32 / w))
            .collect()
    }

    fn px(img: &EffectImage, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * img.width() + x) * 4) as usize;
        let d = img.data();
        [d[i], d[i + 1], d[i + 2], d[i + 3]]
    }

    fn manhattan(a: (u32, u32), b: (u32, u32)) -> u32 {
        a.0.abs_diff(b.0) + a.1.abs_diff(b.1)
    }

    fn chebyshev(a: (u32, u32), b: (u32, u32)) -> u32 {
        a.0.abs_diff(b.0).max(a.1.abs_diff(b.1))
    }

    /// Every coordinate of a `w × h` image matching `pred`, row-major.
    fn coords(w: u32, h: u32, pred: impl Fn((u32, u32)) -> bool) -> Vec<(u32, u32)> {
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&p| pred(p))
            .collect()
    }

    /// Brute-force reference: the pixels an outline changes, straight from
    /// the AC definitions (beyond-edge pixels are transparent).
    fn reference(
        w: u32,
        h: u32,
        opaque: &[(u32, u32)],
        place: OutlinePlace,
        shape: OutlineShape,
        t: u32,
    ) -> Vec<(u32, u32)> {
        let dist = |a, b| match shape {
            OutlineShape::Circle => manhattan(a, b),
            OutlineShape::Square => chebyshev(a, b),
        };
        let is_opaque = |p: (u32, u32)| opaque.contains(&p);
        let outside = |p| !is_opaque(p) && opaque.iter().any(|&o| dist(p, o) <= t);
        // The nearest beyond-edge pixel lies straight across the nearest
        // edge, at the same distance in both metrics.
        let inside = |p: (u32, u32)| {
            is_opaque(p)
                && ((p.0 + 1).min(p.1 + 1).min(w - p.0).min(h - p.1) <= t
                    || coords(w, h, |q| !is_opaque(q))
                        .into_iter()
                        .any(|q| dist(p, q) <= t))
        };
        coords(w, h, |p| match place {
            OutlinePlace::Outside => outside(p),
            OutlinePlace::Inside => inside(p),
            OutlinePlace::Both => outside(p) || inside(p),
        })
    }

    #[test]
    fn apply_outside_square_single_pixel_colours_its_8_neighbours() {
        let src = image(5, 5, &[(2, 2)]);
        let out = Outline::new(OutlinePlace::Outside, OutlineShape::Square, 1, INK).apply(&src);
        let expected = coords(5, 5, |p| chebyshev(p, (2, 2)) == 1);
        assert_eq!(changed(&src, &out), expected);
        for (x, y) in expected {
            assert_eq!(px(&out, x, y), INK, "exact colour incl. alpha (AC-06)");
        }
        assert_eq!(px(&out, 2, 2), RED);
    }

    #[test]
    fn apply_outside_circle_single_pixel_colours_its_4_edge_neighbours() {
        let src = image(5, 5, &[(2, 2)]);
        let out = Outline::new(OutlinePlace::Outside, OutlineShape::Circle, 1, INK).apply(&src);
        assert_eq!(changed(&src, &out), vec![(2, 1), (1, 2), (3, 2), (2, 3)]);
    }

    #[test]
    fn apply_outside_square_thickness_3_colours_a_7x7_block() {
        let src = image(9, 9, &[(4, 4)]);
        let out = Outline::new(OutlinePlace::Outside, OutlineShape::Square, 3, INK).apply(&src);
        let expected = coords(9, 9, |p| p != (4, 4) && chebyshev(p, (4, 4)) <= 3);
        assert_eq!(expected.len(), 48);
        assert_eq!(changed(&src, &out), expected);
    }

    /// An irregular sprite touching the left edge, checked against the
    /// brute-force definition for every place, shape and thickness.
    #[test]
    fn apply_matches_the_distance_definition_for_every_option() {
        let opaque = [
            (0, 3),
            (1, 3),
            (2, 3),
            (2, 4),
            (3, 4),
            (6, 1),
            (6, 2),
            (7, 2),
            (5, 6),
            (5, 7),
            (6, 6),
            (6, 7),
            (7, 7),
        ];
        let (w, h) = (11, 10);
        let src = image(w, h, &opaque);
        for place in [
            OutlinePlace::Outside,
            OutlinePlace::Inside,
            OutlinePlace::Both,
        ] {
            for shape in [OutlineShape::Circle, OutlineShape::Square] {
                for t in 1..=4 {
                    let out = Outline::new(place, shape, t, INK).apply(&src);
                    assert_eq!(
                        changed(&src, &out),
                        reference(w, h, &opaque, place, shape, t),
                        "{place:?} {shape:?} t={t}"
                    );
                }
            }
        }
    }

    #[test]
    fn apply_both_is_the_union_of_outside_and_inside() {
        let opaque = coords(8, 8, |(x, y)| (2..6).contains(&x) && (1..7).contains(&y));
        let src = image(8, 8, &opaque);
        for shape in [OutlineShape::Circle, OutlineShape::Square] {
            for t in 1..=3 {
                let run = |place| changed(&src, &Outline::new(place, shape, t, INK).apply(&src));
                let mut union = run(OutlinePlace::Outside);
                union.extend(run(OutlinePlace::Inside));
                union.sort_by_key(|&(x, y)| (y, x));
                assert_eq!(run(OutlinePlace::Both), union, "{shape:?} t={t}");
            }
        }
    }

    #[test]
    fn apply_inside_fully_opaque_thickness_1_recolours_the_border_ring() {
        let all = coords(5, 4, |_| true);
        let src = image(5, 4, &all);
        let out = Outline::new(OutlinePlace::Inside, OutlineShape::Square, 1, INK).apply(&src);
        let ring = coords(5, 4, |(x, y)| x == 0 || y == 0 || x == 4 || y == 3);
        assert_eq!(changed(&src, &out), ring);
    }

    #[test]
    fn apply_outside_fully_opaque_or_empty_layer_changes_nothing() {
        let full = image(4, 4, &coords(4, 4, |_| true));
        let out = Outline::new(OutlinePlace::Outside, OutlineShape::Square, 2, INK).apply(&full);
        assert_eq!(out, full, "no transparent pixel to colour");
        let empty = image(4, 4, &[]);
        for place in [
            OutlinePlace::Outside,
            OutlinePlace::Inside,
            OutlinePlace::Both,
        ] {
            let out = Outline::new(place, OutlineShape::Square, 2, INK).apply(&empty);
            assert_eq!(out, empty, "{place:?}: no opaque pixel, nothing changes");
        }
    }

    #[test]
    fn apply_outside_at_the_edge_writes_only_inside_the_buffer() {
        let src = image(3, 3, &[(0, 0)]);
        let out = Outline::new(OutlinePlace::Outside, OutlineShape::Square, 1, INK).apply(&src);
        assert_eq!(changed(&src, &out), vec![(1, 0), (0, 1), (1, 1)]);
        assert_eq!(out.data().len(), src.data().len());
    }

    #[test]
    fn apply_alpha_1_counts_as_opaque() {
        let mut data = vec![0u8; 3 * 3 * 4];
        let c = (3 + 1) * 4;
        data[c..c + 4].copy_from_slice(&[9, 9, 9, 1]);
        let src = EffectImage::from_rgba8(3, 3, data).expect("image");
        let out = Outline::new(OutlinePlace::Outside, OutlineShape::Square, 1, INK).apply(&src);
        assert_eq!(px(&out, 1, 1), [9, 9, 9, 1], "Outside does not cover it");
        assert_eq!(changed(&src, &out).len(), 8);
        let inside = Outline::new(OutlinePlace::Inside, OutlineShape::Square, 1, INK).apply(&src);
        assert_eq!(px(&inside, 1, 1), INK, "Inside recolours it");
    }

    #[test]
    fn new_clamps_thickness_to_1_through_8() {
        assert_eq!(
            Outline::new(OutlinePlace::Outside, OutlineShape::Square, 0, INK).thickness(),
            1
        );
        assert_eq!(
            Outline::new(OutlinePlace::Outside, OutlineShape::Square, 99, INK).thickness(),
            8
        );
    }
}
