//! What a command changed in the composite, so a render cache can update
//! only that part (docs/specs/performance-image-cache.md).

use crate::geometry::Rect;

/// Rect lists longer than this collapse into their bounding rect, so the
/// damage of a long stroke stays a bounded amount of memory.
const MAX_RECTS: usize = 256;

/// The part of a document's composite a command may have changed.
///
/// Reported by [`Command::damage`](super::Command::damage) for both apply and
/// revert, and collected by [`CommandBus`](super::CommandBus) until a render
/// cache takes it ([`CommandBus::take_damage`](super::CommandBus::take_damage)).
/// It is conservative: a reported pixel may be unchanged, but a changed pixel
/// is always reported.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Damage {
    /// No composite pixel changed (selection, layer names, lock flags).
    #[default]
    None,
    /// Only composite pixels inside these canvas rects may have changed; the
    /// canvas size and the layer stack's compose-relevant state did not.
    Rects(Vec<Rect>),
    /// Anything may have changed, including the canvas size.
    Full,
}

impl Damage {
    /// Damage covering one canvas rect; an empty rect is no damage.
    pub fn rect(rect: Rect) -> Self {
        if rect.w == 0 || rect.h == 0 {
            Damage::None
        } else {
            Damage::Rects(vec![rect])
        }
    }

    /// Adds `other` to this damage (set union, conservatively widened).
    pub fn add(&mut self, other: Damage) {
        match (&mut *self, other) {
            (Damage::Full, _) | (_, Damage::None) => {}
            (_, Damage::Full) => *self = Damage::Full,
            (Damage::None, other) => *self = other,
            (Damage::Rects(rects), Damage::Rects(more)) => {
                for r in more {
                    if !rects.iter().any(|known| contains(*known, r)) {
                        rects.push(r);
                    }
                }
                if rects.len() > MAX_RECTS {
                    let bounds = rects.iter().copied().reduce(bounding).unwrap_or(rects[0]);
                    *rects = vec![bounds];
                }
            }
        }
    }

    /// Whether nothing is damaged.
    pub fn is_none(&self) -> bool {
        matches!(self, Damage::None)
    }
}

/// Whether `outer` covers all of `inner`.
fn contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.right() <= outer.right()
        && inner.bottom() <= outer.bottom()
}

/// The smallest rect covering both `a` and `b`.
fn bounding(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = a.right().max(b.right());
    let y1 = a.bottom().max(b.bottom());
    Rect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_full_absorbs_everything() {
        let mut d = Damage::rect(Rect::new(0, 0, 2, 2));
        d.add(Damage::Full);
        d.add(Damage::rect(Rect::new(5, 5, 1, 1)));
        assert_eq!(d, Damage::Full);
    }

    #[test]
    fn add_rects_skips_covered_rects() {
        let mut d = Damage::rect(Rect::new(0, 0, 10, 10));
        d.add(Damage::rect(Rect::new(2, 2, 3, 3)));
        d.add(Damage::rect(Rect::new(20, 0, 1, 1)));
        assert_eq!(
            d,
            Damage::Rects(vec![Rect::new(0, 0, 10, 10), Rect::new(20, 0, 1, 1)])
        );
    }

    #[test]
    fn add_many_rects_collapses_to_their_bounds() {
        let mut d = Damage::None;
        for i in 0..=MAX_RECTS as i32 {
            d.add(Damage::rect(Rect::new(i * 2, 0, 1, 1)));
        }
        assert_eq!(
            d,
            Damage::Rects(vec![Rect::new(0, 0, MAX_RECTS as u32 * 2 + 1, 1)])
        );
    }

    #[test]
    fn rect_with_zero_area_is_no_damage() {
        assert!(Damage::rect(Rect::new(3, 3, 0, 4)).is_none());
    }
}
