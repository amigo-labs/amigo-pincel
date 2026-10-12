//! Raw RGBA8 pixel storage.

use crate::color::Color;
use crate::error::DocumentError;
use crate::geometry::Rect;
use serde::{Deserialize, Serialize};

/// A rectangular block of straight-alpha RGBA8 pixels.
///
/// Data is stored row-major, 4 bytes per pixel, length `width * height * 4`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageBuffer {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl ImageBuffer {
    /// Creates a fully transparent buffer of the given size.
    pub fn new_transparent(width: u32, height: u32) -> Self {
        let len = width as usize * height as usize * 4;
        Self {
            width,
            height,
            data: vec![0; len],
        }
    }

    /// Creates a buffer from raw RGBA8 bytes.
    ///
    /// Returns `RegionOutOfBounds` if `data.len() != width * height * 4`.
    pub fn from_raw(width: u32, height: u32, data: Vec<u8>) -> Result<Self, DocumentError> {
        if data.len() != width as usize * height as usize * 4 {
            return Err(DocumentError::RegionOutOfBounds);
        }
        Ok(Self {
            width,
            height,
            data,
        })
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Read-only view of the raw RGBA8 bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Mutable view of the raw RGBA8 bytes.
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Consumes the buffer, returning the raw RGBA8 bytes.
    pub fn into_raw(self) -> Vec<u8> {
        self.data
    }

    /// Byte offset of pixel `(x, y)`, or `None` if out of bounds.
    fn offset(&self, x: u32, y: u32) -> Option<usize> {
        if x < self.width && y < self.height {
            Some((y as usize * self.width as usize + x as usize) * 4)
        } else {
            None
        }
    }

    /// Returns the color at `(x, y)`, or `None` if out of bounds.
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Color> {
        let o = self.offset(x, y)?;
        Some(Color::rgba(
            self.data[o],
            self.data[o + 1],
            self.data[o + 2],
            self.data[o + 3],
        ))
    }

    /// Sets the color at `(x, y)`. Out-of-bounds writes are ignored.
    pub fn set_pixel(&mut self, x: u32, y: u32, c: Color) {
        if let Some(o) = self.offset(x, y) {
            self.data[o] = c.r;
            self.data[o + 1] = c.g;
            self.data[o + 2] = c.b;
            self.data[o + 3] = c.a;
        }
    }

    /// Copies this buffer into a fresh transparent `w` × `h` buffer with its
    /// origin at `(dx, dy)`, clipping anything that falls outside.
    ///
    /// Shared by canvas resize (anchored placement) and layer 90°-rotation
    /// (center fit).
    pub fn offset_copy(&self, w: u32, h: u32, dx: i32, dy: i32) -> ImageBuffer {
        let mut out = ImageBuffer::new_transparent(w, h);
        for sy in 0..self.height {
            let ty = sy as i32 + dy;
            if ty < 0 || ty >= h as i32 {
                continue;
            }
            for sx in 0..self.width {
                let tx = sx as i32 + dx;
                if tx < 0 || tx >= w as i32 {
                    continue;
                }
                if let Some(c) = self.get_pixel(sx, sy) {
                    out.set_pixel(tx as u32, ty as u32, c);
                }
            }
        }
        out
    }

    /// Extracts a sub-region as a new buffer.
    ///
    /// The region must lie fully within the buffer, otherwise
    /// `RegionOutOfBounds` is returned.
    pub fn copy_region(&self, region: Rect) -> Result<ImageBuffer, DocumentError> {
        if region.x < 0
            || region.y < 0
            || region.right() > self.width as i32
            || region.bottom() > self.height as i32
        {
            return Err(DocumentError::RegionOutOfBounds);
        }
        let mut out = ImageBuffer::new_transparent(region.w, region.h);
        for row in 0..region.h {
            let src_y = region.y as u32 + row;
            let src_start = (src_y as usize * self.width as usize + region.x as usize) * 4;
            let len = region.w as usize * 4;
            let dst_start = row as usize * region.w as usize * 4;
            out.data[dst_start..dst_start + len]
                .copy_from_slice(&self.data[src_start..src_start + len]);
        }
        Ok(out)
    }

    /// Pastes `src` into this buffer with its top-left at `(region.x, region.y)`.
    ///
    /// `src` dimensions must match `region`. The region must lie fully within
    /// this buffer, otherwise `RegionOutOfBounds` is returned.
    pub fn paste_region(&mut self, region: Rect, src: &ImageBuffer) -> Result<(), DocumentError> {
        if src.width != region.w || src.height != region.h {
            return Err(DocumentError::RegionOutOfBounds);
        }
        if region.x < 0
            || region.y < 0
            || region.right() > self.width as i32
            || region.bottom() > self.height as i32
        {
            return Err(DocumentError::RegionOutOfBounds);
        }
        for row in 0..region.h {
            let dst_y = region.y as u32 + row;
            let dst_start = (dst_y as usize * self.width as usize + region.x as usize) * 4;
            let len = region.w as usize * 4;
            let src_start = row as usize * region.w as usize * 4;
            self.data[dst_start..dst_start + len]
                .copy_from_slice(&src.data[src_start..src_start + len]);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_transparent_is_all_zero() {
        let buf = ImageBuffer::new_transparent(2, 3);
        assert_eq!(buf.data().len(), 2 * 3 * 4);
        assert!(buf.data().iter().all(|&b| b == 0));
    }

    #[test]
    fn from_raw_rejects_wrong_length() {
        assert_eq!(
            ImageBuffer::from_raw(2, 2, vec![0; 3]),
            Err(DocumentError::RegionOutOfBounds)
        );
    }

    #[test]
    fn set_then_get_pixel_round_trips() {
        let mut buf = ImageBuffer::new_transparent(4, 4);
        let c = Color::rgba(10, 20, 30, 40);
        buf.set_pixel(1, 2, c);
        assert_eq!(buf.get_pixel(1, 2), Some(c));
    }

    #[test]
    fn get_pixel_out_of_bounds_is_none() {
        let buf = ImageBuffer::new_transparent(2, 2);
        assert_eq!(buf.get_pixel(2, 0), None);
        assert_eq!(buf.get_pixel(0, 2), None);
    }

    #[test]
    fn copy_and_paste_region_round_trips() {
        let mut buf = ImageBuffer::new_transparent(4, 4);
        buf.set_pixel(1, 1, Color::WHITE);
        let region = Rect::new(1, 1, 2, 2);
        let patch = buf.copy_region(region).unwrap();
        assert_eq!(patch.get_pixel(0, 0), Some(Color::WHITE));

        let mut dst = ImageBuffer::new_transparent(4, 4);
        dst.paste_region(region, &patch).unwrap();
        assert_eq!(dst.get_pixel(1, 1), Some(Color::WHITE));
    }

    #[test]
    fn copy_region_out_of_bounds_errors() {
        let buf = ImageBuffer::new_transparent(4, 4);
        assert_eq!(
            buf.copy_region(Rect::new(3, 3, 2, 2)),
            Err(DocumentError::RegionOutOfBounds)
        );
    }
}
