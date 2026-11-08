//! The `.traineddata` container: Tesseract's `TessdataManager`
//! (`src/ccutil/tessdatamanager.cpp`, `LoadMemBuffer`).
//!
//! An `u32` entry count, an `i64` offset table (−1 for absent components),
//! then the components back to back. A component ends where the next
//! present one starts, the last one at the end of the data.

use crate::error::LoadError;
use crate::reader::Reader;

/// Tesseract's `kMaxNumTessdataEntries`.
const MAX_ENTRIES: u32 = 1000;
/// Tesseract's `TESSDATA_NUM_ENTRIES` (version 5).
const NUM_TYPES: usize = 24;

/// The component indices the recognizer reads (`TessdataType`).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Component {
    Lstm = 17,
    LstmUnicharset = 21,
    LstmRecoder = 22,
    Version = 23,
}

/// The components of a `.traineddata` archive, borrowed from its bytes.
pub(crate) struct Tessdata<'a> {
    entries: [Option<&'a [u8]>; NUM_TYPES],
}

impl<'a> Tessdata<'a> {
    /// Splits `data` into its components.
    pub(crate) fn parse(data: &'a [u8]) -> Result<Self, LoadError> {
        let mut r = Reader::new(data);
        let num_entries = r.u32().map_err(|_| LoadError::NotTraineddata)?;
        // A big-endian file reads as a huge count; Tesseract would swap it,
        // but every published model is little-endian.
        if num_entries == 0 || num_entries > MAX_ENTRIES {
            return Err(LoadError::NotTraineddata);
        }
        let n = num_entries as usize;
        if n * 8 > r.remaining() {
            return Err(LoadError::Truncated);
        }
        let mut offsets = Vec::with_capacity(n);
        for _ in 0..n {
            offsets.push(r.i64()?);
        }
        let size = data.len() as i64;
        let mut entries = [None; NUM_TYPES];
        for i in 0..n.min(NUM_TYPES) {
            let start = offsets[i];
            if start < 0 {
                continue;
            }
            if start > size {
                return Err(LoadError::Truncated);
            }
            let mut end = size;
            if let Some(&next) = offsets[i + 1..].iter().find(|&&o| o != -1) {
                if next < 0 || next > size {
                    return Err(LoadError::Truncated);
                }
                end = next;
            }
            if end < start {
                return Err(LoadError::NotTraineddata);
            }
            entries[i] = Some(&data[start as usize..end as usize]);
        }
        Ok(Self { entries })
    }

    /// The bytes of a component, if present and non-empty.
    pub(crate) fn get(&self, component: Component) -> Option<&'a [u8]> {
        self.entries[component as usize].filter(|e| !e.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(offsets: &[i64], payload: &[u8]) -> Vec<u8> {
        let mut out = (offsets.len() as u32).to_le_bytes().to_vec();
        for o in offsets {
            out.extend_from_slice(&o.to_le_bytes());
        }
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn parse_splits_components_at_next_offset() {
        let header = 4 + 8 * 24;
        let mut offsets = vec![-1i64; 24];
        offsets[17] = header;
        offsets[21] = header + 2;
        let data = archive(&offsets, b"abcde");
        let t = Tessdata::parse(&data).expect("valid archive");
        assert_eq!(t.get(Component::Lstm), Some(&b"ab"[..]));
        assert_eq!(t.get(Component::LstmUnicharset), Some(&b"cde"[..]));
        assert_eq!(t.get(Component::LstmRecoder), None);
    }

    #[test]
    fn parse_offset_past_end_returns_truncated() {
        let mut offsets = vec![-1i64; 24];
        offsets[17] = 1 << 40;
        let data = archive(&offsets, b"");
        assert_eq!(Tessdata::parse(&data).err(), Some(LoadError::Truncated));
    }

    #[test]
    fn parse_text_file_returns_not_traineddata() {
        assert_eq!(
            Tessdata::parse(b"hello world").err(),
            Some(LoadError::NotTraineddata)
        );
        assert_eq!(Tessdata::parse(b"").err(), Some(LoadError::NotTraineddata));
    }
}
