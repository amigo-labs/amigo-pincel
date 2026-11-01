//! Colour values of the trace API.

/// An opaque sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rgb {
    /// Red, 0 – 255.
    pub r: u8,
    /// Green, 0 – 255.
    pub g: u8,
    /// Blue, 0 – 255.
    pub b: u8,
}

impl Rgb {
    /// A colour from its channels.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses `#rrggbb` or `rrggbb` (case-insensitive); `None` otherwise.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let h = hex.trim();
        let h = h.strip_prefix('#').unwrap_or(h);
        if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let v = u32::from_str_radix(h, 16).ok()?;
        Some(Self::new((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }

    /// The colour as lower-case `#rrggbb`.
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// An sRGB colour with straight (not premultiplied) alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rgba {
    /// Red, 0 – 255.
    pub r: u8,
    /// Green, 0 – 255.
    pub g: u8,
    /// Blue, 0 – 255.
    pub b: u8,
    /// Alpha, 0 (transparent) – 255 (opaque).
    pub a: u8,
}

impl Rgba {
    /// A colour from its channels.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// The colour without its alpha.
    pub const fn rgb(self) -> Rgb {
        Rgb::new(self.r, self.g, self.b)
    }

    /// An opaque colour.
    pub const fn opaque(c: Rgb) -> Self {
        Self::new(c.r, c.g, c.b, 255)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_hex_round_trips_and_rejects_garbage() {
        assert_eq!(Rgb::from_hex("#2e86ab"), Some(Rgb::new(46, 134, 171)));
        assert_eq!(Rgb::from_hex("F4EFE6"), Some(Rgb::new(244, 239, 230)));
        assert_eq!(Rgb::new(192, 57, 43).hex(), "#c0392b");
        assert_eq!(Rgb::from_hex("kaputt"), None);
        assert_eq!(Rgb::from_hex("#fff"), None);
    }
}
