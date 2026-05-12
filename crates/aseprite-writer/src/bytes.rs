//! Little-endian write helpers for Aseprite scalar types.
//!
//! The Aseprite file format is little-endian throughout. These helpers
//! mirror the reader-side scalar parsers in
//! `aseprite-loader::binary::scalars`.

use std::io::Write;

use crate::error::WriteError;

#[inline]
pub fn write_byte<W: Write>(w: &mut W, v: u8) -> Result<(), WriteError> {
    w.write_all(&[v])?;
    Ok(())
}

#[inline]
pub fn write_word<W: Write>(w: &mut W, v: u16) -> Result<(), WriteError> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}

#[inline]
pub fn write_short<W: Write>(w: &mut W, v: i16) -> Result<(), WriteError> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}

#[inline]
pub fn write_dword<W: Write>(w: &mut W, v: u32) -> Result<(), WriteError> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}

#[inline]
pub fn write_long<W: Write>(w: &mut W, v: i32) -> Result<(), WriteError> {
    w.write_all(&v.to_le_bytes())?;
    Ok(())
}

#[inline]
pub fn write_zeros<W: Write>(w: &mut W, n: usize) -> Result<(), WriteError> {
    const CHUNK: [u8; 32] = [0u8; 32];
    let mut remaining = n;
    while remaining > 0 {
        let take = remaining.min(CHUNK.len());
        w.write_all(&CHUNK[..take])?;
        remaining -= take;
    }
    Ok(())
}

/// Writes a u16-prefixed UTF-8 string (the spec's `STRING` type).
pub fn write_string<W: Write>(w: &mut W, s: &str) -> Result<(), WriteError> {
    let bytes = s.as_bytes();
    let len: u16 = bytes
        .len()
        .try_into()
        .map_err(|_| WriteError::StringTooLong {
            preview: s.chars().take(32).collect(),
            len: bytes.len(),
        })?;
    write_word(w, len)?;
    w.write_all(bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_word_emits_le_bytes() {
        let mut buf = Vec::new();
        write_word(&mut buf, 0xA5E0).unwrap();
        assert_eq!(buf, [0xE0, 0xA5]);
    }

    #[test]
    fn write_short_emits_two_complement_le() {
        let mut buf = Vec::new();
        write_short(&mut buf, -2).unwrap();
        assert_eq!(buf, [0xFE, 0xFF]);
    }

    #[test]
    fn write_dword_emits_le_bytes() {
        let mut buf = Vec::new();
        write_dword(&mut buf, 0x0102_0304).unwrap();
        assert_eq!(buf, [0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn write_long_emits_two_complement_le() {
        let mut buf = Vec::new();
        write_long(&mut buf, -2).unwrap();
        assert_eq!(buf, [0xFE, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn write_string_prefixes_utf8_length() {
        let mut buf = Vec::new();
        write_string(&mut buf, "Layer 1").unwrap();
        assert_eq!(buf, [0x07, 0x00, b'L', b'a', b'y', b'e', b'r', b' ', b'1']);
    }

    #[test]
    fn write_zeros_emits_exactly_n_bytes() {
        let mut buf = Vec::new();
        write_zeros(&mut buf, 100).unwrap();
        assert_eq!(buf.len(), 100);
        assert!(buf.iter().all(|&b| b == 0));
    }
}
