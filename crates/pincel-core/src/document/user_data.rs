//! Aseprite user data (text note + colour) attached to the sprite, a
//! layer, a cel or a tag. Pincel does not edit it; the codec round-trips
//! it so a file opened and saved in Pincel keeps the notes and colours
//! other tools (and Aseprite's properties dialogs) put there. See
//! `docs/specs/pincel.md` §7.1.

use super::color::Rgba;

/// User data: an optional free-form note and an optional colour.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserData {
    pub text: Option<String>,
    pub color: Option<Rgba>,
}

impl UserData {
    /// `true` when neither a note nor a colour is set.
    pub fn is_empty(&self) -> bool {
        self.text.is_none() && self.color.is_none()
    }
}
