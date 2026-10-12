//! Aseprite user data (text note, colour, property maps) attached to the
//! sprite, a layer, a cel or a tag. Pincel does not edit it; the codec
//! round-trips it so a file opened and saved in Pincel keeps the notes,
//! colours and extension properties other tools (and Aseprite's
//! properties dialogs) put there. See `docs/specs/pincel.md` §7.1.

use super::color::Rgba;

/// User data: an optional free-form note, colour and property maps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserData {
    pub text: Option<String>,
    pub color: Option<Rgba>,
    /// Property maps as the opaque on-disk block (from its leading
    /// "size of all maps" `DWORD` on). Kept byte for byte; Pincel never
    /// interprets it.
    pub properties: Option<Vec<u8>>,
}

impl UserData {
    /// `true` when no note, colour or property block is set.
    pub fn is_empty(&self) -> bool {
        self.text.is_none() && self.color.is_none() && self.properties.is_none()
    }
}
