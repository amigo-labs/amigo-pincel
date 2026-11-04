//! How a cel's or tile's stored pixels become RGBA8 at composite time —
//! `docs/specs/pincel.md` §4.2 ("convert indexed pixels to RGBA at composite
//! time") and feature spec `indexed-grayscale-compose.md`.
//!
//! - RGBA: the bytes as stored.
//! - Grayscale: value V and alpha A render as (V, V, V, A).
//! - Indexed: palette lookup through a 256-entry table built once per
//!   compose; the transparent index is empty except on a background layer,
//!   and an index past the palette's end is empty.

use crate::document::{BlendMode, ColorMode, Layer, Palette};

/// Background-layer bit of [`Layer::extra_flags`] (Aseprite layer flag `8`).
const BACKGROUND_FLAG: u16 = 0x8;

/// RGBA8 for each of the 256 possible palette indexes.
pub(crate) type IndexLut = [[u8; 4]; 256];

/// The decoding of one pixel buffer's bytes into RGBA8.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PixelDecode<'a> {
    /// 4 bytes per pixel, used as stored.
    Rgba,
    /// 2 bytes per pixel (value, alpha).
    Grayscale,
    /// 1 byte per pixel, looked up in `lut`.
    Indexed {
        transparent_index: u8,
        lut: &'a IndexLut,
    },
}

impl PixelDecode<'_> {
    /// The color mode a buffer must use to be decoded this way.
    pub(crate) fn color_mode(&self) -> ColorMode {
        match self {
            Self::Rgba => ColorMode::Rgba,
            Self::Grayscale => ColorMode::Grayscale,
            Self::Indexed {
                transparent_index, ..
            } => ColorMode::Indexed {
                transparent_index: *transparent_index,
            },
        }
    }

    /// RGBA8 of pixel number `pixel` (row-major) in `data`. `data` must be
    /// a well-formed buffer of [`Self::color_mode`].
    #[inline(always)]
    pub(crate) fn rgba(&self, data: &[u8], pixel: usize) -> [u8; 4] {
        match self {
            Self::Rgba => {
                let s = pixel * 4;
                [data[s], data[s + 1], data[s + 2], data[s + 3]]
            }
            Self::Grayscale => {
                let s = pixel * 2;
                let v = data[s];
                [v, v, v, data[s + 1]]
            }
            Self::Indexed { lut, .. } => lut[usize::from(data[pixel])],
        }
    }
}

/// How one layer's pixels go onto the destination: the decoding of its
/// source bytes, its combined layer × cel opacity and its blend mode.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Paint<'a> {
    pub(crate) decode: PixelDecode<'a>,
    pub(crate) opacity: u8,
    pub(crate) blend_mode: BlendMode,
}

impl Paint<'static> {
    /// RGBA source pixels under `opacity` and `blend_mode`.
    pub(crate) fn rgba(opacity: u8, blend_mode: BlendMode) -> Self {
        Self {
            decode: PixelDecode::Rgba,
            opacity,
            blend_mode,
        }
    }
}

/// The decoders of one sprite: for an indexed sprite the palette tables of
/// non-background and background layers, built once per compose.
pub(crate) struct SpriteDecoders {
    mode: ColorMode,
    /// `[non-background, background]`; only filled for indexed sprites.
    luts: Option<[IndexLut; 2]>,
}

impl SpriteDecoders {
    /// Build the decoders for a sprite in `mode` with `palette`.
    pub(crate) fn new(mode: ColorMode, palette: &Palette) -> Self {
        let luts = match mode {
            ColorMode::Indexed { transparent_index } => Some([
                index_lut(palette, Some(transparent_index)),
                index_lut(palette, None),
            ]),
            ColorMode::Rgba | ColorMode::Grayscale => None,
        };
        Self { mode, luts }
    }

    /// The decoding for the pixels of `layer`.
    pub(crate) fn for_layer(&self, layer: &Layer) -> PixelDecode<'_> {
        match (self.mode, &self.luts) {
            (ColorMode::Indexed { transparent_index }, Some(luts)) => {
                let background = layer.extra_flags & BACKGROUND_FLAG != 0;
                PixelDecode::Indexed {
                    transparent_index,
                    lut: &luts[usize::from(background)],
                }
            }
            (ColorMode::Grayscale, _) => PixelDecode::Grayscale,
            _ => PixelDecode::Rgba,
        }
    }
}

/// The 256-entry table for `palette`: an index past the palette's end and
/// `transparent` (when given) map to fully transparent.
fn index_lut(palette: &Palette, transparent: Option<u8>) -> IndexLut {
    let mut lut = [[0u8; 4]; 256];
    for (slot, entry) in lut.iter_mut().zip(&palette.colors) {
        let c = entry.rgba;
        *slot = [c.r, c.g, c.b, c.a];
    }
    if let Some(t) = transparent {
        lut[usize::from(t)] = [0; 4];
    }
    lut
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{LayerId, PaletteEntry, Rgba};

    fn pal() -> Palette {
        Palette::from_entries(vec![
            PaletteEntry::new(Rgba::new(1, 2, 3, 255)),
            PaletteEntry::new(Rgba::new(4, 5, 6, 7)),
        ])
    }

    #[test]
    fn for_layer_indexed_background_keeps_transparent_entry() {
        let dec = SpriteDecoders::new(
            ColorMode::Indexed {
                transparent_index: 0,
            },
            &pal(),
        );
        let mut layer = Layer::image(LayerId::new(0), "a");
        let data = [0u8, 1, 2];
        let normal = dec.for_layer(&layer);
        assert_eq!(normal.rgba(&data, 0), [0; 4]);
        assert_eq!(normal.rgba(&data, 1), [4, 5, 6, 7]);
        assert_eq!(normal.rgba(&data, 2), [0; 4]);
        layer.extra_flags = BACKGROUND_FLAG;
        let background = dec.for_layer(&layer);
        assert_eq!(background.rgba(&data, 0), [1, 2, 3, 255]);
        assert_eq!(background.rgba(&data, 2), [0; 4]);
    }

    #[test]
    fn rgba_grayscale_expands_value_to_three_channels() {
        let data = [9u8, 200, 30, 0];
        assert_eq!(PixelDecode::Grayscale.rgba(&data, 0), [9, 9, 9, 200]);
        assert_eq!(PixelDecode::Grayscale.rgba(&data, 1), [30, 30, 30, 0]);
        assert_eq!(PixelDecode::Rgba.rgba(&data, 0), [9, 200, 30, 0]);
    }
}
