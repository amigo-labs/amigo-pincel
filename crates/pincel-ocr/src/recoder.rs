//! The recoder: Tesseract's `UnicharCompress` (`src/ccutil/unicharcompress.cpp`).
//!
//! Each unichar id is encoded as a short sequence of network output codes.
//! The beam search builds codes step by step, so the recoder answers which
//! codes may follow a prefix and which unichar a full code is.

use std::collections::HashMap;

use crate::error::LoadError;
use crate::reader::Reader;

/// Tesseract's `RecodedCharID::kMaxCodeLen`.
pub(crate) const MAX_CODE_LEN: usize = 9;

/// A code sequence of at most [`MAX_CODE_LEN`] network outputs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub(crate) struct Code {
    len: u8,
    codes: [i32; MAX_CODE_LEN],
}

impl Code {
    /// Sets the code at `index`, extending the length as needed.
    pub(crate) fn set(&mut self, index: usize, value: i32) {
        if index >= MAX_CODE_LEN {
            return;
        }
        self.codes[index] = value;
        if usize::from(self.len) <= index {
            self.len = index as u8 + 1;
        }
    }

    pub(crate) fn len(&self) -> usize {
        usize::from(self.len)
    }

    fn truncated(&self, len: usize) -> Self {
        let mut out = *self;
        out.len = len as u8;
        out.codes[len..].fill(0);
        out
    }
}

/// The code ↔ unichar mapping of a model.
#[derive(Debug, Clone)]
pub(crate) struct Recoder {
    encoder: Vec<Code>,
    decoder: HashMap<Code, usize>,
    next_codes: HashMap<Code, Vec<i32>>,
    final_codes: HashMap<Code, Vec<i32>>,
    code_range: usize,
}

impl Recoder {
    /// Reads the serialized encoder (`UnicharCompress::DeSerialize`).
    pub(crate) fn parse(data: &[u8]) -> Result<Self, LoadError> {
        let unreadable = |_| LoadError::CharsetUnreadable;
        let mut r = Reader::new(data);
        // Each entry is at least an i8 and a u32.
        let n = r.count(5).map_err(unreadable)?;
        let mut encoder = Vec::with_capacity(n);
        for _ in 0..n {
            let _self_normalized = r.i8().map_err(unreadable)?;
            let len = r.u32().map_err(unreadable)? as usize;
            if len > MAX_CODE_LEN {
                return Err(LoadError::CharsetUnreadable);
            }
            let mut code = Code::default();
            for i in 0..len {
                let v = r.i32().map_err(unreadable)?;
                if !(0..i32::from(u16::MAX)).contains(&v) {
                    return Err(LoadError::CharsetUnreadable);
                }
                code.set(i, v);
            }
            code.len = len as u8;
            encoder.push(code);
        }
        let code_range = encoder
            .iter()
            .flat_map(|c| c.codes[..c.len()].iter().copied())
            .max()
            .map_or(0, |m| m as usize + 1);
        let mut out = Self {
            encoder,
            decoder: HashMap::new(),
            next_codes: HashMap::new(),
            final_codes: HashMap::new(),
            code_range,
        };
        out.setup_decoder();
        Ok(out)
    }

    /// `UnicharCompress::SetupDecoder`: the lists keep Tesseract's insertion
    /// order, which the beam search iterates in.
    fn setup_decoder(&mut self) {
        for (c, code) in self.encoder.iter().enumerate() {
            self.decoder.insert(*code, c);
            let mut len = code.len();
            if len == 0 {
                continue;
            }
            len -= 1;
            let prefix = code.truncated(len);
            if let Some(list) = self.final_codes.get_mut(&prefix) {
                if !list.contains(&code.codes[len]) {
                    list.push(code.codes[len]);
                }
                continue;
            }
            self.final_codes.insert(prefix, vec![code.codes[len]]);
            while len > 0 {
                len -= 1;
                let prefix = code.truncated(len);
                if let Some(list) = self.next_codes.get_mut(&prefix) {
                    if !list.contains(&code.codes[len]) {
                        list.push(code.codes[len]);
                    }
                    break;
                }
                self.next_codes.insert(prefix, vec![code.codes[len]]);
            }
        }
    }

    /// Number of entries the encoder has (one per unichar id).
    pub(crate) fn len(&self) -> usize {
        self.encoder.len()
    }

    /// One more than the largest code value.
    pub(crate) fn code_range(&self) -> usize {
        self.code_range
    }

    /// The unichar a full code sequence stands for.
    pub(crate) fn decode(&self, code: &Code) -> Option<usize> {
        if code.len() == 0 {
            return None;
        }
        self.decoder.get(&code.truncated(code.len())).copied()
    }

    /// Codes that complete a unichar after `prefix`.
    pub(crate) fn final_codes(&self, prefix: &Code) -> Option<&[i32]> {
        self.final_codes.get(prefix).map(Vec::as_slice)
    }

    /// Codes that continue `prefix` without completing a unichar.
    pub(crate) fn next_codes(&self, prefix: &Code) -> Option<&[i32]> {
        self.next_codes.get(prefix).map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serialize(codes: &[&[i32]]) -> Vec<u8> {
        let mut out = (codes.len() as u32).to_le_bytes().to_vec();
        for c in codes {
            out.push(1);
            out.extend_from_slice(&(c.len() as u32).to_le_bytes());
            for v in *c {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out
    }

    #[test]
    fn parse_builds_final_and_next_codes() {
        let r = Recoder::parse(&serialize(&[&[0], &[1, 2], &[1, 3]])).expect("valid recoder");
        assert_eq!(r.code_range(), 4);
        assert_eq!(r.final_codes(&Code::default()), Some(&[0][..]));
        assert_eq!(r.next_codes(&Code::default()), Some(&[1][..]));
        let mut prefix = Code::default();
        prefix.set(0, 1);
        assert_eq!(r.final_codes(&prefix), Some(&[2, 3][..]));
        prefix.set(1, 3);
        assert_eq!(r.decode(&prefix), Some(2));
    }

    #[test]
    fn parse_negative_code_returns_charset_unreadable() {
        assert_eq!(
            Recoder::parse(&serialize(&[&[-1]])).err(),
            Some(LoadError::CharsetUnreadable)
        );
    }
}
