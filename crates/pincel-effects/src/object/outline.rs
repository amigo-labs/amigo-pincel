//! Outline Object (docs/specs/outline-effect.md): a hard pixel-art outline
//! in one colour around the opaque pixels of a layer.

use crate::effect::Effect;
use crate::image::EffectImage;

/// Where the outline goes relative to the opaque pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlinePlace {
    /// Transparent pixels next to the shape.
    Outside,
    /// The shape's own border pixels.
    Inside,
    /// Both of the above.
    Both,
}

/// The neighbourhood one step of the outline reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineShape {
    /// The 4 edge neighbours (Manhattan distance).
    Circle,
    /// All 8 neighbours (Chebyshev distance).
    Square,
}

/// Colours every pixel within `thickness` of the shape's boundary (by the
/// shape's metric) with `color`, on the side(s) `place` names.
///
/// A pixel is opaque when its alpha is above 0; pixels beyond the image's
/// edge count as transparent, and the image never grows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outline {
    /// Outside, inside or both.
    pub place: OutlinePlace,
    /// Circle (4-neighbour) or square (8-neighbour).
    pub shape: OutlineShape,
    /// `1..=8` pixels.
    pub thickness: u32,
    /// Straight-alpha RGBA the outline pixels take exactly.
    pub color: [u8; 4],
}

impl Outline {
    /// Smallest accepted thickness.
    pub const MIN_THICKNESS: u32 = 1;
    /// Largest accepted thickness.
    pub const MAX_THICKNESS: u32 = 8;

    /// Creates an outline; `thickness` is clamped to `1..=8`.
    pub fn new(place: OutlinePlace, shape: OutlineShape, thickness: u32, color: [u8; 4]) -> Self {
        Self {
            place,
            shape,
            thickness: thickness.clamp(Self::MIN_THICKNESS, Self::MAX_THICKNESS),
            color,
        }
    }
}

/// `t` steps of 3×3 dilation of `seed` (cross for a circle, full block for
/// a square): a pixel ends up set when a seed pixel lies within distance
/// `t` by the shape's metric. `outside_set` says whether the pixels beyond
/// the edge belong to the seed.
fn dilate(
    seed: &[bool],
    w: usize,
    h: usize,
    shape: OutlineShape,
    t: u32,
    outside_set: bool,
) -> Vec<bool> {
    const CROSS: [(isize, isize); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];
    const BLOCK: [(isize, isize); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
    let steps: &[(isize, isize)] = match shape {
        OutlineShape::Circle => &CROSS,
        OutlineShape::Square => &BLOCK,
    };
    let mut cur = seed.to_vec();
    for _ in 0..t {
        let prev = cur.clone();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if prev[i] {
                    continue;
                }
                cur[i] = steps.iter().any(|&(dx, dy)| {
                    let (nx, ny) = (x as isize + dx, y as isize + dy);
                    if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                        outside_set
                    } else {
                        prev[ny as usize * w + nx as usize]
                    }
                });
            }
        }
    }
    cur
}

impl Effect for Outline {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let mut out = src.data().to_vec();
        let opaque: Vec<bool> = src
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[3] > 0)
            .collect();
        let outside = matches!(self.place, OutlinePlace::Outside | OutlinePlace::Both);
        let inside = matches!(self.place, OutlinePlace::Inside | OutlinePlace::Both);
        // Transparent pixels near the shape: dilate the opaque set.
        let near_opaque = outside.then(|| dilate(&opaque, w, h, self.shape, self.thickness, false));
        // Opaque pixels near a transparent one (the edge included): dilate
        // the transparent set, with everything beyond the edge in it.
        let near_clear = inside.then(|| {
            let clear: Vec<bool> = opaque.iter().map(|&o| !o).collect();
            dilate(&clear, w, h, self.shape, self.thickness, true)
        });
        for (i, (p, &is_opaque)) in out
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(&opaque)
            .enumerate()
        {
            let hit = if is_opaque {
                near_clear.as_ref().is_some_and(|m| m[i])
            } else {
                near_opaque.as_ref().is_some_and(|m| m[i])
            };
            if hit {
                *p = self.color;
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, factor: f32) -> Self {
        let t = (self.thickness as f32 * factor).round() as u32;
        Self::new(self.place, self.shape, t, self.color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_thickness_outside_range_is_clamped() {
        let o = |t| Outline::new(OutlinePlace::Outside, OutlineShape::Square, t, [0; 4]).thickness;
        assert_eq!((o(0), o(1), o(8), o(9)), (1, 1, 8, 8));
    }

    #[test]
    fn scaled_keeps_at_least_one_pixel() {
        let o = Outline::new(OutlinePlace::Both, OutlineShape::Circle, 4, [1, 2, 3, 4]);
        assert_eq!(o.scaled(0.5).thickness, 2);
        assert_eq!(o.scaled(0.01).thickness, 1);
    }
}
