//! Bounds-checked little-endian reader over a byte slice: the subset of
//! Tesseract's `TFile` the recognizer needs (`src/ccutil/serialis.cpp`).
//!
//! Every length read from the data is checked against the bytes that are
//! left before anything is allocated, so a corrupt size field yields
//! [`LoadError::Truncated`] instead of a huge allocation.

use crate::error::LoadError;

/// Reads values from a byte slice, failing with [`LoadError::Truncated`] at
/// the end of the data.
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub(crate) fn bytes(&mut self, n: usize) -> Result<&'a [u8], LoadError> {
        if n > self.remaining() {
            return Err(LoadError::Truncated);
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], LoadError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.bytes(N)?);
        Ok(out)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, LoadError> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn i8(&mut self) -> Result<i8, LoadError> {
        Ok(i8::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, LoadError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn i32(&mut self) -> Result<i32, LoadError> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    pub(crate) fn i64(&mut self) -> Result<i64, LoadError> {
        Ok(i64::from_le_bytes(self.array()?))
    }

    pub(crate) fn f32(&mut self) -> Result<f32, LoadError> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    pub(crate) fn f64(&mut self) -> Result<f64, LoadError> {
        Ok(f64::from_le_bytes(self.array()?))
    }

    /// A `std::string`: a `u32` length, then the bytes (`TFile::DeSerialize`,
    /// which rejects lengths above 50 000 000).
    pub(crate) fn string(&mut self) -> Result<String, LoadError> {
        let len = self.u32()? as usize;
        if len > 50_000_000 {
            return Err(LoadError::Truncated);
        }
        Ok(String::from_utf8_lossy(self.bytes(len)?).into_owned())
    }

    /// A count of `elem_size`-byte elements that must fit in the remaining
    /// data.
    pub(crate) fn count(&mut self, elem_size: usize) -> Result<usize, LoadError> {
        let n = self.u32()? as usize;
        if n.checked_mul(elem_size)
            .is_none_or(|b| b > self.remaining())
        {
            return Err(LoadError::Truncated);
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_larger_than_data_returns_truncated() {
        let data = [0xff, 0xff, 0xff, 0x7f, 1, 2];
        let mut r = Reader::new(&data);
        assert_eq!(r.count(1), Err(LoadError::Truncated));
    }

    #[test]
    fn string_reads_length_prefixed_bytes() {
        let data = [3, 0, 0, 0, b'a', b'b', b'c'];
        let mut r = Reader::new(&data);
        assert_eq!(r.string().as_deref(), Ok("abc"));
        assert_eq!(r.remaining(), 0);
    }
}
