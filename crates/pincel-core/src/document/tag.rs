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
    /// Tag user data (spec §7.1).
    pub user_data: Option<super::UserData>,
    /// The on-disk direction byte when it is not one of the four Pincel
    /// knows (a newer Aseprite). Playback treats such a tag as `direction`
    /// (`Forward`); the writer puts the byte back. Cleared by any edit of
    /// the direction.
    pub raw_direction: Option<u8>,
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
            user_data: None,
            raw_direction: None,
        }
    }
}
