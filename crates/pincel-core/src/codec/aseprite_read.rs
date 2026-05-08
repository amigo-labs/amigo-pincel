//! Aseprite v1.3 read adapter.
//!
//! Wraps [`aseprite_loader`] and translates its parser output into Pincel's
//! [`Sprite`] / [`CelMap`] pair. See `docs/specs/pincel.md` §7.1.
//!
//! M4 scope (this milestone):
//!
//! - **RGBA color mode only.** Indexed / grayscale return
//!   [`CodecError::UnsupportedColorMode`].
//! - **Image and group layers only.** Tilemap layers raise
//!   [`CodecError::UnsupportedLayerKind`]; tileset chunks are dropped on read
//!   (M8 will preserve them).
//! - **Slice chunks are dropped.** M9 will round-trip them.
//! - **Linked cels are preserved** as [`CelData::Linked`] — round-trip via the
//!   M5 writer remains lossless.

use aseprite_loader::binary::blend_mode::BlendMode as AseBlendMode;
use aseprite_loader::binary::chunks::cel::CelContent;
use aseprite_loader::binary::chunks::layer::{LayerFlags, LayerType};
use aseprite_loader::binary::chunks::tags::AnimationDirection;
use aseprite_loader::binary::color_depth::ColorDepth;
use aseprite_loader::loader::AsepriteFile;

use super::error::CodecError;
use crate::document::{
    BlendMode, Cel, CelData, CelMap, ColorMode, Frame, FrameIndex, Layer, LayerId, LayerKind,
    PaletteEntry, PixelBuffer, Rgba, Sprite, Tag, TagDirection,
};

/// Result of reading a `.aseprite` byte slice. The cel store is returned
/// alongside the [`Sprite`] because Pincel keeps cel data outside the document
/// (see `cel_map`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsepriteReadOutput {
    pub sprite: Sprite,
    pub cels: CelMap,
}

/// Parse an Aseprite v1.3 byte stream into a Pincel document.
///
/// `bytes` is the full file content as it would appear on disk. The input is
/// borrowed only for the duration of the call; the returned [`Sprite`] owns
/// all its data.
pub fn read_aseprite(bytes: &[u8]) -> Result<AsepriteReadOutput, CodecError> {
    let ase = AsepriteFile::load(bytes).map_err(|e| CodecError::Parse(e.to_string()))?;

    let color_mode = map_color_mode(ase.file.header.color_depth)?;

    let mut layers = Vec::with_capacity(ase.file.layers.len());
    for (index, layer_chunk) in ase.file.layers.iter().enumerate() {
        layers.push(map_layer(index, layer_chunk)?);
    }

    let mut frames = Vec::with_capacity(ase.file.frames.len());
    for frame in &ase.file.frames {
        frames.push(Frame::new(frame.duration));
    }

    let palette = build_palette(&ase);
    let tags = ase.tags.iter().map(map_tag).collect::<Vec<_>>();

    let mut sprite_builder = Sprite::builder(
        u32::from(ase.file.header.width),
        u32::from(ase.file.header.height),
    )
    .color_mode(color_mode)
    .palette(palette);
    for layer in layers {
        sprite_builder = sprite_builder.add_layer(layer);
    }
    for frame in frames {
        sprite_builder = sprite_builder.add_frame(frame);
    }
    for tag in tags {
        sprite_builder = sprite_builder.add_tag(tag);
    }
    let sprite = sprite_builder.build()?;

    let cels = build_cels(&ase, color_mode, &sprite)?;

    Ok(AsepriteReadOutput { sprite, cels })
}

fn map_color_mode(depth: ColorDepth) -> Result<ColorMode, CodecError> {
    match depth {
        ColorDepth::Rgba => Ok(ColorMode::Rgba),
        ColorDepth::Indexed | ColorDepth::Grayscale | ColorDepth::Unknown(_) => {
            Err(CodecError::UnsupportedColorMode)
        }
    }
}

fn map_layer(
    index: usize,
    layer: &aseprite_loader::binary::chunks::layer::LayerChunk<'_>,
) -> Result<Layer, CodecError> {
    let id = LayerId::new(index as u32);
    let kind = match layer.layer_type {
        LayerType::Normal => LayerKind::Image,
        LayerType::Group => LayerKind::Group,
        LayerType::Tilemap => return Err(CodecError::UnsupportedLayerKind { kind: 2 }),
        LayerType::Unknown(n) => return Err(CodecError::UnsupportedLayerKind { kind: n }),
    };
    let blend_mode = map_blend_mode(layer.blend_mode)?;
    Ok(Layer {
        id,
        name: layer.name.to_string(),
        kind,
        visible: layer.flags.contains(LayerFlags::VISIBLE),
        editable: layer.flags.contains(LayerFlags::EDITABLE),
        blend_mode,
        opacity: layer.opacity,
        parent: None,
    })
}

fn map_blend_mode(mode: AseBlendMode) -> Result<BlendMode, CodecError> {
    Ok(match mode {
        AseBlendMode::Normal => BlendMode::Normal,
        AseBlendMode::Multiply => BlendMode::Multiply,
        AseBlendMode::Screen => BlendMode::Screen,
        AseBlendMode::Overlay => BlendMode::Overlay,
        AseBlendMode::Darken => BlendMode::Darken,
        AseBlendMode::Lighten => BlendMode::Lighten,
        AseBlendMode::ColorDodge => BlendMode::ColorDodge,
        AseBlendMode::ColorBurn => BlendMode::ColorBurn,
        AseBlendMode::HardLight => BlendMode::HardLight,
        AseBlendMode::SoftLight => BlendMode::SoftLight,
        AseBlendMode::Difference => BlendMode::Difference,
        AseBlendMode::Exclusion => BlendMode::Exclusion,
        AseBlendMode::Hue => BlendMode::Hue,
        AseBlendMode::Saturation => BlendMode::Saturation,
        AseBlendMode::Color => BlendMode::Color,
        AseBlendMode::Luminosity => BlendMode::Luminosity,
        AseBlendMode::Addition => BlendMode::Addition,
        AseBlendMode::Subtract => BlendMode::Subtract,
        AseBlendMode::Divide => BlendMode::Divide,
        AseBlendMode::Unknown(n) => return Err(CodecError::UnsupportedBlendMode { mode: n }),
    })
}

fn map_tag_direction(direction: AnimationDirection) -> TagDirection {
    match direction {
        AnimationDirection::Forward => TagDirection::Forward,
        AnimationDirection::Reverse => TagDirection::Reverse,
        AnimationDirection::PingPong => TagDirection::Pingpong,
        AnimationDirection::PingPongReverse => TagDirection::PingpongReverse,
        // Future Aseprite directions fall back to the closest analogue. The
        // round-trip writer (M5) will preserve unknown tags as opaque chunks.
        _ => TagDirection::Forward,
    }
}

fn map_tag(tag: &aseprite_loader::loader::Tag) -> Tag {
    Tag {
        name: tag.name.clone(),
        from: FrameIndex::new(u32::from(*tag.range.start())),
        to: FrameIndex::new(u32::from(*tag.range.end())),
        direction: map_tag_direction(tag.direction),
        color: Rgba::WHITE,
        repeats: tag.repeat.unwrap_or(0),
    }
}

fn build_palette(ase: &AsepriteFile<'_>) -> crate::document::Palette {
    let Some(parsed) = ase.file.palette.as_ref() else {
        return crate::document::Palette::default();
    };
    let entries = parsed
        .colors
        .iter()
        .map(|c| {
            PaletteEntry::new(Rgba {
                r: c.red,
                g: c.green,
                b: c.blue,
                a: c.alpha,
            })
        })
        .collect();
    crate::document::Palette::from_entries(entries)
}

fn build_cels(
    ase: &AsepriteFile<'_>,
    color_mode: ColorMode,
    sprite: &Sprite,
) -> Result<CelMap, CodecError> {
    let mut map = CelMap::new();
    for (frame_index, frame) in ase.file.frames.iter().enumerate() {
        for cel_chunk in frame.cels.iter().filter_map(|c| c.as_ref()) {
            let layer_index = usize::from(cel_chunk.layer_index);
            let layer_id = sprite
                .layers
                .get(layer_index)
                .map(|l| l.id)
                .ok_or(CodecError::LayerIndexOutOfRange { index: layer_index })?;
            let frame_idx = FrameIndex::new(frame_index as u32);

            let data = match &cel_chunk.content {
                CelContent::Image(image) => {
                    let width = u32::from(image.width);
                    let height = u32::from(image.height);
                    let frame_cel = ase.frames[frame_index]
                        .cels
                        .iter()
                        .find(|fc| fc.layer_index == layer_index)
                        .ok_or(CodecError::LayerIndexOutOfRange { index: layer_index })?;
                    let bytes_per_pixel = color_mode.bytes_per_pixel();
                    let mut data =
                        vec![0u8; (width as usize) * (height as usize) * bytes_per_pixel];
                    ase.load_image(frame_cel.image_index, &mut data)
                        .map_err(|e| CodecError::Image(e.to_string()))?;
                    CelData::Image(PixelBuffer {
                        width,
                        height,
                        color_mode,
                        data,
                    })
                }
                CelContent::LinkedCel { frame_position } => {
                    CelData::Linked(FrameIndex::new(u32::from(*frame_position)))
                }
                CelContent::CompressedTilemap { .. } => {
                    return Err(CodecError::UnsupportedCelKind { kind: 3 });
                }
                CelContent::Unknown { cel_type, .. } => {
                    return Err(CodecError::UnsupportedCelKind { kind: *cel_type });
                }
            };

            map.insert(Cel {
                layer: layer_id,
                frame: frame_idx,
                position: (i32::from(cel_chunk.x), i32::from(cel_chunk.y)),
                opacity: cel_chunk.opacity,
                data,
            });
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_byte_slice_returns_parse_error() {
        let err = read_aseprite(&[]).unwrap_err();
        assert!(
            matches!(err, CodecError::Parse(_)),
            "expected parse error, got {err:?}"
        );
    }

    #[test]
    fn map_color_mode_only_accepts_rgba() {
        assert_eq!(map_color_mode(ColorDepth::Rgba).unwrap(), ColorMode::Rgba);
        assert!(matches!(
            map_color_mode(ColorDepth::Indexed),
            Err(CodecError::UnsupportedColorMode)
        ));
        assert!(matches!(
            map_color_mode(ColorDepth::Grayscale),
            Err(CodecError::UnsupportedColorMode)
        ));
        assert!(matches!(
            map_color_mode(ColorDepth::Unknown(99)),
            Err(CodecError::UnsupportedColorMode)
        ));
    }

    #[test]
    fn map_blend_mode_round_trips_known_values() {
        assert_eq!(
            map_blend_mode(AseBlendMode::Normal).unwrap(),
            BlendMode::Normal
        );
        assert_eq!(
            map_blend_mode(AseBlendMode::Divide).unwrap(),
            BlendMode::Divide
        );
        assert!(matches!(
            map_blend_mode(AseBlendMode::Unknown(0x1337)),
            Err(CodecError::UnsupportedBlendMode { mode: 0x1337 })
        ));
    }

    #[test]
    fn map_tag_direction_maps_known_variants() {
        assert_eq!(
            map_tag_direction(AnimationDirection::Forward),
            TagDirection::Forward
        );
        assert_eq!(
            map_tag_direction(AnimationDirection::Reverse),
            TagDirection::Reverse
        );
        assert_eq!(
            map_tag_direction(AnimationDirection::PingPong),
            TagDirection::Pingpong
        );
        assert_eq!(
            map_tag_direction(AnimationDirection::PingPongReverse),
            TagDirection::PingpongReverse
        );
    }
}
