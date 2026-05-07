//! Slices: named rectangles overlaid on the sprite. See `docs/specs/pincel.md` §3.5.

use super::color::Rgba;
use super::frame::FrameIndex;
use crate::geometry::Rect;

/// Stable identifier for a slice within a sprite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SliceId(pub u32);

impl SliceId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

/// Per-frame geometry for a slice. A key applies from `frame` until the next
/// key's `frame` (Aseprite semantics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SliceKey {
    pub frame: FrameIndex,
    pub bounds: Rect,
    /// 9-patch inner rectangle, if this slice is a 9-patch.
    pub center: Option<Rect>,
    /// Optional pivot point, in sprite coordinates.
    pub pivot: Option<(i32, i32)>,
}

/// A slice: a named region with optional 9-patch and pivot data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slice {
    pub id: SliceId,
    pub name: String,
    /// Visual color used to draw the slice in the editor overlay.
    pub color: Rgba,
    /// Sorted by `SliceKey::frame`, ascending.
    pub keys: Vec<SliceKey>,
}
