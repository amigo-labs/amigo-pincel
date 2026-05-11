//! Cels: per-(layer, frame) image or tilemap data. See `docs/specs/pincel.md` §3.2.

use super::color_mode::ColorMode;
use super::frame::FrameIndex;
use super::layer::LayerId;

/// A buffer of pixel data backing an image cel or a tile image.
///
/// The byte layout depends on [`ColorMode`]:
/// - `Rgba`: 4 bytes per pixel (R, G, B, A) in row-major order
/// - `Indexed`: 1 byte per pixel, palette index
/// - `Grayscale`: 2 bytes per pixel (V, A) — Phase 2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelBuffer {
    pub width: u32,
    pub height: u32,
    pub color_mode: ColorMode,
    pub data: Vec<u8>,
}

impl PixelBuffer {
    /// Allocate a transparent buffer of the given dimensions and color mode.
    pub fn empty(width: u32, height: u32, color_mode: ColorMode) -> Self {
        let bytes = (width as usize) * (height as usize) * color_mode.bytes_per_pixel();
        Self {
            width,
            height,
            color_mode,
            data: vec![0; bytes],
        }
    }

    /// Bytes per pixel implied by the color mode.
    pub const fn bytes_per_pixel(&self) -> usize {
        self.color_mode.bytes_per_pixel()
    }

    /// `true` if the buffer dimensions are consistent with `data.len()`.
    pub fn is_well_formed(&self) -> bool {
        self.data.len() == (self.width as usize) * (self.height as usize) * self.bytes_per_pixel()
    }
}

/// A reference to a tile inside a tileset, with optional flips and rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileRef {
    pub tile_id: u32,
    pub flip_x: bool,
    pub flip_y: bool,
    pub rotate_90: bool,
}

impl TileRef {
    /// The empty / transparent tile (Aseprite convention: tile id 0).
    pub const EMPTY: Self = Self {
        tile_id: 0,
        flip_x: false,
        flip_y: false,
        rotate_90: false,
    };

    pub const fn new(tile_id: u32) -> Self {
        Self {
            tile_id,
            flip_x: false,
            flip_y: false,
            rotate_90: false,
        }
    }
}

/// The payload of a cel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CelData {
    /// A bitmap image at the cel's position.
    Image(PixelBuffer),
    /// A tile grid referencing a tileset bound to the parent layer.
    Tilemap {
        grid_w: u32,
        grid_h: u32,
        tiles: Vec<TileRef>,
    },
    /// Aseprite optimization: this cel reuses the data of another cel in the
    /// same layer at the referenced frame. Pincel preserves linkage on read;
    /// the writer expands links in Phase 1.
    Linked(FrameIndex),
}

/// A cel: data attached to a `(layer, frame)` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cel {
    pub layer: LayerId,
    pub frame: FrameIndex,
    /// Top-left in sprite coordinates. May be off-canvas.
    pub position: (i32, i32),
    pub opacity: u8,
    pub data: CelData,
}

impl Cel {
    /// Cel containing the given image buffer at the sprite origin.
    pub fn image(layer: LayerId, frame: FrameIndex, buffer: PixelBuffer) -> Self {
        Self {
            layer,
            frame,
            position: (0, 0),
            opacity: 255,
            data: CelData::Image(buffer),
        }
    }

    /// Cel containing a `grid_w × grid_h` tilemap of `TileRef::EMPTY`s at the
    /// sprite origin. Aseprite convention: tile id `0` is the empty /
    /// transparent tile (see [`TileRef::EMPTY`]).
    pub fn tilemap(layer: LayerId, frame: FrameIndex, grid_w: u32, grid_h: u32) -> Self {
        let tile_count = (grid_w as usize) * (grid_h as usize);
        Self {
            layer,
            frame,
            position: (0, 0),
            opacity: 255,
            data: CelData::Tilemap {
                grid_w,
                grid_h,
                tiles: vec![TileRef::EMPTY; tile_count],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_has_correct_byte_count() {
        let b = PixelBuffer::empty(4, 3, ColorMode::Rgba);
        assert_eq!(b.data.len(), 4 * 3 * 4);
        assert!(b.is_well_formed());
        assert!(b.data.iter().all(|&v| v == 0));
    }

    #[test]
    fn empty_indexed_buffer_uses_one_byte_per_pixel() {
        let b = PixelBuffer::empty(
            8,
            8,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        );
        assert_eq!(b.data.len(), 64);
    }

    #[test]
    fn tile_ref_empty_has_id_zero() {
        assert_eq!(TileRef::EMPTY.tile_id, 0);
    }

    #[test]
    fn cel_image_defaults_to_origin_full_opacity() {
        let buf = PixelBuffer::empty(2, 2, ColorMode::Rgba);
        let c = Cel::image(LayerId::new(0), FrameIndex::new(0), buf);
        assert_eq!(c.position, (0, 0));
        assert_eq!(c.opacity, 255);
        assert!(matches!(c.data, CelData::Image(_)));
    }

    #[test]
    fn cel_tilemap_fills_grid_with_empty_tiles() {
        let c = Cel::tilemap(LayerId::new(0), FrameIndex::new(0), 4, 3);
        assert_eq!(c.position, (0, 0));
        assert_eq!(c.opacity, 255);
        match c.data {
            CelData::Tilemap {
                grid_w,
                grid_h,
                tiles,
            } => {
                assert_eq!(grid_w, 4);
                assert_eq!(grid_h, 3);
                assert_eq!(tiles.len(), 12);
                assert!(tiles.iter().all(|t| *t == TileRef::EMPTY));
            }
            _ => panic!("expected tilemap cel"),
        }
    }

    #[test]
    fn cel_tilemap_supports_zero_sized_grid() {
        let c = Cel::tilemap(LayerId::new(0), FrameIndex::new(0), 0, 0);
        match c.data {
            CelData::Tilemap {
                grid_w,
                grid_h,
                tiles,
            } => {
                assert_eq!((grid_w, grid_h), (0, 0));
                assert!(tiles.is_empty());
            }
            _ => panic!("expected tilemap cel"),
        }
    }
}
