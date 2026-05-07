//! Frame metadata. Per-frame cel data lives in the cel map, not here.

/// Index of a frame in `Sprite::frames`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameIndex(pub u32);

impl FrameIndex {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

/// A single frame's metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// Per-frame display duration in milliseconds (Aseprite native unit).
    pub duration_ms: u16,
}

impl Frame {
    pub const fn new(duration_ms: u16) -> Self {
        Self { duration_ms }
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self::new(100)
    }
}
