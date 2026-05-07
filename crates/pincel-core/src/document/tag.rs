//! Animation tags. See `docs/specs/pincel.md` §3.6.

use super::color::Rgba;
use super::frame::FrameIndex;

/// Playback direction for an animation tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagDirection {
    #[default]
    Forward,
    Reverse,
    Pingpong,
    PingpongReverse,
}

/// A named range of frames with playback metadata. The state machine in
/// `amigo_animation` identifies states by `Tag::name`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    pub from: FrameIndex,
    pub to: FrameIndex,
    pub direction: TagDirection,
    pub color: Rgba,
    /// `0` means infinite repeats.
    pub repeats: u16,
}

impl Tag {
    /// Forward-playing tag with default color and infinite repeats.
    pub fn new(name: impl Into<String>, from: FrameIndex, to: FrameIndex) -> Self {
        Self {
            name: name.into(),
            from,
            to,
            direction: TagDirection::Forward,
            color: Rgba::WHITE,
            repeats: 0,
        }
    }
}
