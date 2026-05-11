//! Aseprite v1.3 write adapter.
//!
//! Translates a Pincel [`Sprite`] / [`CelMap`] pair into the byte stream
//! produced by [`aseprite_writer::write`]. See `docs/specs/pincel.md` §7.1.
//!
//! M5 scope (this milestone):
//!
//! - **RGBA color mode only.** Indexed / grayscale return
//!   [`CodecError::UnsupportedColorMode`].
//! - **Image and group layers only.** Tilemap layers raise
//!   [`CodecError::UnsupportedLayerKind`].
//! - **Image and linked cels only.** Tilemap cels raise
//!   [`CodecError::UnsupportedCelKind`].
//! - Slices and tilesets on the document are dropped, matching
//!   [`super::aseprite_read`] (M9 will round-trip slices).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

use aseprite_writer::{
    AnimationDirection as AseDirection, AseFile, BlendMode as AseBlendMode, CelChunk, CelContent,
    Color as AseColor, ColorDepth as AseColorDepth, Frame as AseFrame, Header, LayerChunk,
    LayerFlags, LayerType, PaletteChunk, PaletteEntry as AsePaletteEntry, Tag as AseTag,
};

use super::error::CodecError;
use crate::document::{
    BlendMode, Cel, CelData, CelMap, ColorMode, Layer, LayerId, LayerKind, Palette, Sprite, Tag,
    TagDirection,
};

/// Serialize a Pincel [`Sprite`] / [`CelMap`] pair to the Aseprite v1.3
/// byte format.
///
/// Errors surface either as adapter-level pre-validation failures (out
/// of u16 range, unknown parent layer, …) or as transparent
/// [`aseprite_writer::WriteError`] values for the writer's own
/// post-validation and I/O failures.
pub fn write_aseprite<W: Write>(
    sprite: &Sprite,
    cels: &CelMap,
    out: &mut W,
) -> Result<(), CodecError> {
    if sprite.color_mode != ColorMode::Rgba {
        return Err(CodecError::UnsupportedColorMode);
    }
    let file = build_ase_file(sprite, cels)?;
    aseprite_writer::write(&file, out)?;
    Ok(())
}

fn build_ase_file(sprite: &Sprite, cels: &CelMap) -> Result<AseFile, CodecError> {
    let header = build_header(sprite)?;
    let layers = build_layers(&sprite.layers)?;
    let palette = build_palette(&sprite.palette);
    let tags = build_tags(&sprite.tags)?;
    let frames = build_frames(sprite, cels)?;
    Ok(AseFile {
        header,
        layers,
        palette,
        tags,
        tilesets: Vec::new(),
        frames,
    })
}

fn build_header(sprite: &Sprite) -> Result<Header, CodecError> {
    let width = u16::try_from(sprite.width).map_err(|_| CodecError::OutOfRange {
        what: "canvas width",
        value: i64::from(sprite.width),
    })?;
    let height = u16::try_from(sprite.height).map_err(|_| CodecError::OutOfRange {
        what: "canvas height",
        value: i64::from(sprite.height),
    })?;
    Ok(Header::new(width, height, AseColorDepth::Rgba))
}

fn build_layers(layers: &[Layer]) -> Result<Vec<LayerChunk>, CodecError> {
    let mut by_id: BTreeMap<LayerId, &Layer> = BTreeMap::new();
    for layer in layers {
        by_id.insert(layer.id, layer);
    }
    validate_parent_kinds(layers, &by_id)?;
    let depths = compute_depths(layers, &by_id)?;
    validate_layer_order(layers, &depths)?;

    let mut out = Vec::with_capacity(layers.len());
    for (layer, child_level) in layers.iter().zip(depths) {
        out.push(map_layer(layer, child_level)?);
    }
    Ok(out)
}

fn map_layer(layer: &Layer, child_level: u16) -> Result<LayerChunk, CodecError> {
    let layer_type = match &layer.kind {
        LayerKind::Image => LayerType::Normal,
        LayerKind::Group => LayerType::Group,
        LayerKind::Tilemap { .. } => return Err(CodecError::UnsupportedLayerKind { kind: 2 }),
    };
    let mut flags = LayerFlags::empty();
    if layer.visible {
        flags |= LayerFlags::VISIBLE;
    }
    if layer.editable {
        flags |= LayerFlags::EDITABLE;
    }
    Ok(LayerChunk {
        flags,
        layer_type,
        child_level,
        blend_mode: map_blend_mode(layer.blend_mode),
        opacity: layer.opacity,
        name: layer.name.clone(),
        tileset_index: None,
    })
}

/// Verify that every layer with a `parent` points at a [`LayerKind::Group`].
/// Aseprite only nests under groups, so this is the structural minimum for
/// a lossless round-trip via [`super::aseprite_read`].
fn validate_parent_kinds(
    layers: &[Layer],
    by_id: &BTreeMap<LayerId, &Layer>,
) -> Result<(), CodecError> {
    for layer in layers {
        if let Some(parent_id) = layer.parent {
            let parent = by_id
                .get(&parent_id)
                .copied()
                .ok_or(CodecError::LayerParentNotFound { id: parent_id.0 })?;
            if !matches!(parent.kind, LayerKind::Group) {
                return Err(CodecError::LayerParentNotGroup {
                    child: layer.id.0,
                    parent: parent_id.0,
                });
            }
        }
    }
    Ok(())
}

fn compute_depths(
    layers: &[Layer],
    by_id: &BTreeMap<LayerId, &Layer>,
) -> Result<Vec<u16>, CodecError> {
    layers
        .iter()
        .map(|l| compute_child_level(l, by_id))
        .collect()
}

/// Walk a layer's parent chain to the root, counting hops. Detects
/// cycles defensively (the document model does not enforce acyclicity).
fn compute_child_level(
    target: &Layer,
    by_id: &BTreeMap<LayerId, &Layer>,
) -> Result<u16, CodecError> {
    let mut depth: u16 = 0;
    let mut seen = BTreeSet::new();
    seen.insert(target.id);
    let mut current = target;
    while let Some(parent_id) = current.parent {
        if !seen.insert(parent_id) {
            return Err(CodecError::LayerCycle { id: parent_id.0 });
        }
        depth = depth.checked_add(1).ok_or(CodecError::OutOfRange {
            what: "layer depth",
            value: i64::from(u16::MAX) + 1,
        })?;
        current = by_id
            .get(&parent_id)
            .copied()
            .ok_or(CodecError::LayerParentNotFound { id: parent_id.0 })?;
    }
    Ok(depth)
}

/// Simulate the read adapter's parent reconstruction (a stack of group
/// layers walked in order) on the about-to-be-emitted child_level
/// sequence and reject any layer whose parent would change after a
/// write→read round-trip. This catches the cases where the parent
/// appears after the child in `sprite.layers`, where a sibling group at
/// the same depth shadows the intended parent, or where the parent is
/// reachable via the parent chain but not via Aseprite's stack walk.
fn validate_layer_order(layers: &[Layer], depths: &[u16]) -> Result<(), CodecError> {
    let mut stack: Vec<LayerId> = Vec::new();
    for (layer, &depth) in layers.iter().zip(depths) {
        let depth_usize = usize::from(depth);
        if depth_usize < stack.len() {
            stack.truncate(depth_usize);
        }
        let reconstructed = if depth == 0 {
            None
        } else {
            stack.get(depth_usize - 1).copied()
        };
        if reconstructed != layer.parent {
            return Err(CodecError::LayerOrderingInconsistent {
                child: layer.id.0,
                expected: layer.parent.map(|p| p.0),
                reconstructed: reconstructed.map(|p| p.0),
            });
        }
        if matches!(layer.kind, LayerKind::Group) {
            stack.push(layer.id);
        }
    }
    Ok(())
}

fn map_blend_mode(mode: BlendMode) -> AseBlendMode {
    match mode {
        BlendMode::Normal => AseBlendMode::Normal,
        BlendMode::Multiply => AseBlendMode::Multiply,
        BlendMode::Screen => AseBlendMode::Screen,
        BlendMode::Overlay => AseBlendMode::Overlay,
        BlendMode::Darken => AseBlendMode::Darken,
        BlendMode::Lighten => AseBlendMode::Lighten,
        BlendMode::ColorDodge => AseBlendMode::ColorDodge,
        BlendMode::ColorBurn => AseBlendMode::ColorBurn,
        BlendMode::HardLight => AseBlendMode::HardLight,
        BlendMode::SoftLight => AseBlendMode::SoftLight,
        BlendMode::Difference => AseBlendMode::Difference,
        BlendMode::Exclusion => AseBlendMode::Exclusion,
        BlendMode::Hue => AseBlendMode::Hue,
        BlendMode::Saturation => AseBlendMode::Saturation,
        BlendMode::Color => AseBlendMode::Color,
        BlendMode::Luminosity => AseBlendMode::Luminosity,
        BlendMode::Addition => AseBlendMode::Addition,
        BlendMode::Subtract => AseBlendMode::Subtract,
        BlendMode::Divide => AseBlendMode::Divide,
    }
}

fn build_palette(palette: &Palette) -> Option<PaletteChunk> {
    if palette.is_empty() {
        return None;
    }
    let entries = palette
        .colors
        .iter()
        .map(|e| AsePaletteEntry {
            color: AseColor::rgba(e.rgba.r, e.rgba.g, e.rgba.b, e.rgba.a),
            name: e.name.clone(),
        })
        .collect();
    Some(PaletteChunk {
        first_color: 0,
        entries,
    })
}

fn build_tags(tags: &[Tag]) -> Result<Vec<AseTag>, CodecError> {
    tags.iter().map(map_tag).collect()
}

fn map_tag(tag: &Tag) -> Result<AseTag, CodecError> {
    let from = u16::try_from(tag.from.0).map_err(|_| CodecError::OutOfRange {
        what: "tag from-frame",
        value: i64::from(tag.from.0),
    })?;
    let to = u16::try_from(tag.to.0).map_err(|_| CodecError::OutOfRange {
        what: "tag to-frame",
        value: i64::from(tag.to.0),
    })?;
    Ok(AseTag {
        from_frame: from,
        to_frame: to,
        direction: map_tag_direction(tag.direction),
        repeat: tag.repeats,
        color: [tag.color.r, tag.color.g, tag.color.b],
        name: tag.name.clone(),
    })
}

fn map_tag_direction(d: TagDirection) -> AseDirection {
    match d {
        TagDirection::Forward => AseDirection::Forward,
        TagDirection::Reverse => AseDirection::Reverse,
        TagDirection::Pingpong => AseDirection::PingPong,
        TagDirection::PingpongReverse => AseDirection::PingPongReverse,
    }
}

fn build_frames(sprite: &Sprite, cels: &CelMap) -> Result<Vec<AseFrame>, CodecError> {
    let id_to_index = build_layer_index_map(&sprite.layers)?;

    let mut frames: Vec<AseFrame> = sprite
        .frames
        .iter()
        .map(|f| AseFrame::new(f.duration_ms))
        .collect();

    for (_, cel) in cels.iter() {
        let frame_idx = cel.frame.0;
        let frame = frames
            .get_mut(frame_idx as usize)
            .ok_or(CodecError::CelFrameNotFound { index: frame_idx })?;
        let chunk = build_cel_chunk(cel, &id_to_index, sprite, cels)?;
        frame.cels.push(chunk);
    }
    Ok(frames)
}

fn build_layer_index_map(layers: &[Layer]) -> Result<BTreeMap<LayerId, u16>, CodecError> {
    let mut map = BTreeMap::new();
    for (i, layer) in layers.iter().enumerate() {
        let idx = u16::try_from(i).map_err(|_| CodecError::OutOfRange {
            what: "layer index",
            value: i as i64,
        })?;
        map.insert(layer.id, idx);
    }
    Ok(map)
}

fn build_cel_chunk(
    cel: &Cel,
    id_to_index: &BTreeMap<LayerId, u16>,
    sprite: &Sprite,
    cels: &CelMap,
) -> Result<CelChunk, CodecError> {
    let layer_index = id_to_index
        .get(&cel.layer)
        .copied()
        .ok_or(CodecError::CelLayerNotFound { id: cel.layer.0 })?;
    let x = i16::try_from(cel.position.0).map_err(|_| CodecError::OutOfRange {
        what: "cel x",
        value: i64::from(cel.position.0),
    })?;
    let y = i16::try_from(cel.position.1).map_err(|_| CodecError::OutOfRange {
        what: "cel y",
        value: i64::from(cel.position.1),
    })?;
    let content = match &cel.data {
        CelData::Image(buf) => {
            // M5 is RGBA-only at the sprite level; an indexed/grayscale
            // PixelBuffer would be written into a header that says 4
            // bytes per pixel and produce a corrupt file.
            if buf.color_mode != ColorMode::Rgba {
                return Err(CodecError::CelImageNotRgba {
                    layer: cel.layer.0,
                    frame: cel.frame.0,
                });
            }
            if !buf.is_well_formed() {
                return Err(CodecError::CelImageBufferMalformed {
                    layer: cel.layer.0,
                    frame: cel.frame.0,
                });
            }
            let width = u16::try_from(buf.width).map_err(|_| CodecError::OutOfRange {
                what: "cel width",
                value: i64::from(buf.width),
            })?;
            let height = u16::try_from(buf.height).map_err(|_| CodecError::OutOfRange {
                what: "cel height",
                value: i64::from(buf.height),
            })?;
            CelContent::Image {
                width,
                height,
                data: buf.data.clone(),
            }
        }
        CelData::Linked(frame) => {
            if (frame.0 as usize) >= sprite.frames.len() {
                return Err(CodecError::LinkedFrameNotFound { index: frame.0 });
            }
            // `aseprite-loader` requires the link target to be a real
            // image cel on the same layer; chained links and missing
            // targets surface as `Parse` errors there. Validate up
            // front so the failure mode is structured.
            let target = cels
                .get(cel.layer, *frame)
                .ok_or(CodecError::LinkedCelTargetMissing {
                    layer: cel.layer.0,
                    from_frame: cel.frame.0,
                    target: frame.0,
                })?;
            if !matches!(target.data, CelData::Image(_)) {
                return Err(CodecError::LinkedCelTargetNotImage {
                    layer: cel.layer.0,
                    from_frame: cel.frame.0,
                    target: frame.0,
                });
            }
            let frame_position = u16::try_from(frame.0).map_err(|_| CodecError::OutOfRange {
                what: "linked frame index",
                value: i64::from(frame.0),
            })?;
            CelContent::Linked { frame_position }
        }
        CelData::Tilemap { .. } => return Err(CodecError::UnsupportedCelKind { kind: 3 }),
    };
    Ok(CelChunk {
        layer_index,
        x,
        y,
        opacity: cel.opacity,
        z_index: 0,
        content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{
        Cel, ColorMode, Frame, FrameIndex, Layer, LayerId, PaletteEntry, PixelBuffer, Rgba, TileRef,
    };

    fn rgba_sprite(layers: Vec<Layer>, frames: Vec<Frame>) -> Sprite {
        let mut b = Sprite::builder(4, 4);
        for layer in layers {
            b = b.add_layer(layer);
        }
        for frame in frames {
            b = b.add_frame(frame);
        }
        b.build().expect("rgba sprite builds")
    }

    #[test]
    fn write_rejects_indexed_color_mode() {
        let sprite = Sprite::builder(2, 2)
            .color_mode(ColorMode::Indexed {
                transparent_index: 0,
            })
            .build()
            .unwrap();
        let cels = CelMap::new();
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(err, CodecError::UnsupportedColorMode));
    }

    #[test]
    fn write_rejects_canvas_wider_than_u16() {
        let sprite = Sprite::builder(70_000, 1).build().unwrap();
        let cels = CelMap::new();
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(
            err,
            CodecError::OutOfRange {
                what: "canvas width",
                value: 70_000,
            }
        ));
    }

    #[test]
    fn write_rejects_tilemap_layer() {
        let sprite = Sprite::builder(4, 4)
            .add_layer(Layer::tilemap(
                LayerId::new(0),
                "tiles",
                crate::TilesetId::new(0),
            ))
            .build()
            .unwrap();
        let cels = CelMap::new();
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(err, CodecError::UnsupportedLayerKind { kind: 2 }));
    }

    #[test]
    fn write_rejects_tilemap_cel() {
        let sprite = rgba_sprite(
            vec![Layer::image(LayerId::new(0), "L0")],
            vec![Frame::new(100)],
        );
        let mut cels = CelMap::new();
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(0),
            position: (0, 0),
            opacity: 255,
            data: CelData::Tilemap {
                grid_w: 1,
                grid_h: 1,
                tiles: vec![TileRef::EMPTY],
            },
        });
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(err, CodecError::UnsupportedCelKind { kind: 3 }));
    }

    #[test]
    fn write_rejects_cel_position_out_of_i16() {
        let sprite = rgba_sprite(
            vec![Layer::image(LayerId::new(0), "L0")],
            vec![Frame::new(100)],
        );
        let mut cels = CelMap::new();
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(0),
            position: (50_000, 0),
            opacity: 255,
            data: CelData::Image(PixelBuffer::empty(1, 1, ColorMode::Rgba)),
        });
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(
            err,
            CodecError::OutOfRange {
                what: "cel x",
                value: 50_000,
            }
        ));
    }

    #[test]
    fn write_rejects_cel_for_unknown_layer() {
        let sprite = rgba_sprite(
            vec![Layer::image(LayerId::new(0), "L0")],
            vec![Frame::new(100)],
        );
        let mut cels = CelMap::new();
        // Inject a cel referencing a layer id that the sprite doesn't carry.
        cels.insert(Cel {
            layer: LayerId::new(99),
            frame: FrameIndex::new(0),
            position: (0, 0),
            opacity: 255,
            data: CelData::Image(PixelBuffer::empty(1, 1, ColorMode::Rgba)),
        });
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(err, CodecError::CelLayerNotFound { id: 99 }));
    }

    #[test]
    fn write_rejects_linked_cel_past_frames() {
        let sprite = rgba_sprite(
            vec![Layer::image(LayerId::new(0), "L0")],
            vec![Frame::new(100)],
        );
        let mut cels = CelMap::new();
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(0),
            position: (0, 0),
            opacity: 255,
            data: CelData::Linked(FrameIndex::new(7)),
        });
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(err, CodecError::LinkedFrameNotFound { index: 7 }));
    }

    #[test]
    fn write_detects_layer_parent_cycle() {
        // a -> b -> a: cycle through `parent`. Both must be groups so
        // we exercise the cycle-detection path inside `compute_child_level`
        // rather than the "parent not a group" pre-check.
        let mut a = Layer::group(LayerId::new(0), "a");
        let mut b = Layer::group(LayerId::new(1), "b");
        a.parent = Some(LayerId::new(1));
        b.parent = Some(LayerId::new(0));
        let sprite = Sprite::builder(2, 2)
            .add_layer(a)
            .add_layer(b)
            .build()
            .unwrap();
        let cels = CelMap::new();
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(err, CodecError::LayerCycle { .. }));
    }

    #[test]
    fn write_detects_unknown_parent() {
        let mut layer = Layer::image(LayerId::new(0), "orphan");
        layer.parent = Some(LayerId::new(42));
        let sprite = Sprite::builder(2, 2).add_layer(layer).build().unwrap();
        let cels = CelMap::new();
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(err, CodecError::LayerParentNotFound { id: 42 }));
    }

    #[test]
    fn write_rejects_layer_parent_that_is_not_a_group() {
        // The reader only pushes Group layers onto its parent stack, so a
        // child whose parent is an image layer would round-trip with the
        // wrong parent (or `None`).
        let bg = Layer::image(LayerId::new(0), "bg");
        let mut child = Layer::image(LayerId::new(1), "child");
        child.parent = Some(LayerId::new(0));
        let sprite = Sprite::builder(2, 2)
            .add_layer(bg)
            .add_layer(child)
            .build()
            .unwrap();
        let cels = CelMap::new();
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(
            err,
            CodecError::LayerParentNotGroup {
                child: 1,
                parent: 0,
            }
        ));
    }

    #[test]
    fn write_rejects_layer_whose_parent_appears_after_it() {
        // [child(parent=g), g] — the reader walks layers in order with a
        // group stack; `g` isn't pushed until after `child` is processed,
        // so reconstructed parent would be `None`, not `g`.
        let mut child = Layer::image(LayerId::new(0), "child");
        child.parent = Some(LayerId::new(1));
        let group = Layer::group(LayerId::new(1), "g");
        let sprite = Sprite::builder(2, 2)
            .add_layer(child)
            .add_layer(group)
            .build()
            .unwrap();
        let cels = CelMap::new();
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(
            err,
            CodecError::LayerOrderingInconsistent {
                child: 0,
                expected: Some(1),
                reconstructed: None,
            }
        ));
    }

    #[test]
    fn write_rejects_layer_whose_parent_is_shadowed_by_sibling_group() {
        // [g_a, g_b, child(parent=g_a)] — both groups are at depth 0.
        // The reader's stack at child's processing has g_b at depth 0
        // (g_a was popped/replaced), so reconstructed parent would be
        // g_b, not g_a.
        let g_a = Layer::group(LayerId::new(0), "g_a");
        let g_b = Layer::group(LayerId::new(1), "g_b");
        let mut child = Layer::image(LayerId::new(2), "child");
        child.parent = Some(LayerId::new(0));
        let sprite = Sprite::builder(2, 2)
            .add_layer(g_a)
            .add_layer(g_b)
            .add_layer(child)
            .build()
            .unwrap();
        let cels = CelMap::new();
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(
            err,
            CodecError::LayerOrderingInconsistent {
                child: 2,
                expected: Some(0),
                reconstructed: Some(1),
            }
        ));
    }

    #[test]
    fn write_rejects_image_cel_with_indexed_buffer() {
        let sprite = rgba_sprite(
            vec![Layer::image(LayerId::new(0), "L0")],
            vec![Frame::new(100)],
        );
        let mut cels = CelMap::new();
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(0),
            position: (0, 0),
            opacity: 255,
            data: CelData::Image(PixelBuffer::empty(
                1,
                1,
                ColorMode::Indexed {
                    transparent_index: 0,
                },
            )),
        });
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(
            err,
            CodecError::CelImageNotRgba { layer: 0, frame: 0 }
        ));
    }

    #[test]
    fn write_rejects_image_cel_with_malformed_buffer() {
        let sprite = rgba_sprite(
            vec![Layer::image(LayerId::new(0), "L0")],
            vec![Frame::new(100)],
        );
        let mut cels = CelMap::new();
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(0),
            position: (0, 0),
            opacity: 255,
            // 2x2 RGBA expects 16 bytes; supply 8.
            data: CelData::Image(PixelBuffer {
                width: 2,
                height: 2,
                color_mode: ColorMode::Rgba,
                data: vec![0; 8],
            }),
        });
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(
            err,
            CodecError::CelImageBufferMalformed { layer: 0, frame: 0 }
        ));
    }

    #[test]
    fn write_rejects_linked_cel_with_missing_target() {
        let sprite = rgba_sprite(
            vec![Layer::image(LayerId::new(0), "L0")],
            vec![Frame::new(100), Frame::new(100)],
        );
        let mut cels = CelMap::new();
        // Frame 0 has no cel for layer 0; frame 1 links to it.
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(1),
            position: (0, 0),
            opacity: 255,
            data: CelData::Linked(FrameIndex::new(0)),
        });
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(
            err,
            CodecError::LinkedCelTargetMissing {
                layer: 0,
                from_frame: 1,
                target: 0,
            }
        ));
    }

    #[test]
    fn write_rejects_chained_linked_cel() {
        // f0 = linked(f2) — illegal: linked target must itself be image.
        let sprite = rgba_sprite(
            vec![Layer::image(LayerId::new(0), "L0")],
            vec![Frame::new(100), Frame::new(100), Frame::new(100)],
        );
        let mut cels = CelMap::new();
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(2),
            position: (0, 0),
            opacity: 255,
            data: CelData::Image(PixelBuffer::empty(1, 1, ColorMode::Rgba)),
        });
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(1),
            position: (0, 0),
            opacity: 255,
            // points at frame 0 — but frame 0 has nothing yet.
            // Make it a chain: link from frame 0 to frame 1.
            data: CelData::Linked(FrameIndex::new(2)),
        });
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(0),
            position: (0, 0),
            opacity: 255,
            data: CelData::Linked(FrameIndex::new(1)),
        });
        let mut buf = Vec::new();
        let err = write_aseprite(&sprite, &cels, &mut buf).unwrap_err();
        assert!(matches!(
            err,
            CodecError::LinkedCelTargetNotImage {
                layer: 0,
                from_frame: 0,
                target: 1,
            }
        ));
    }

    #[test]
    fn write_then_read_round_trips_image_cel() {
        // Build a sprite with one RGBA image cel, write, read, compare.
        let mut buf_data = vec![0u8; 4 * 4 * 4];
        for (i, byte) in buf_data.iter_mut().enumerate() {
            *byte = (i * 7) as u8;
        }
        let pixels = PixelBuffer {
            width: 4,
            height: 4,
            color_mode: ColorMode::Rgba,
            data: buf_data,
        };
        let sprite = rgba_sprite(
            vec![Layer::image(LayerId::new(0), "Background")],
            vec![Frame::new(120)],
        );
        let mut cels = CelMap::new();
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(0),
            position: (0, 0),
            opacity: 255,
            data: CelData::Image(pixels.clone()),
        });

        let mut bytes = Vec::new();
        write_aseprite(&sprite, &cels, &mut bytes).expect("writer succeeds");
        let out = super::super::aseprite_read::read_aseprite(&bytes).expect("reader succeeds");

        assert_eq!(out.sprite.width, 4);
        assert_eq!(out.sprite.height, 4);
        assert_eq!(out.sprite.layers.len(), 1);
        assert_eq!(out.sprite.frames.len(), 1);
        assert_eq!(out.sprite.frames[0].duration_ms, 120);
        let cel = out
            .cels
            .get(LayerId::new(0), FrameIndex::new(0))
            .expect("round-tripped cel present");
        match &cel.data {
            CelData::Image(read_buf) => {
                assert_eq!(read_buf.width, pixels.width);
                assert_eq!(read_buf.height, pixels.height);
                assert_eq!(read_buf.data, pixels.data);
            }
            other => panic!("expected image cel, got {other:?}"),
        }
    }

    #[test]
    fn build_palette_drops_empty_palette() {
        let p = Palette::default();
        assert!(build_palette(&p).is_none());
    }

    #[test]
    fn build_palette_preserves_named_entries() {
        let p = Palette::from_entries(vec![
            PaletteEntry::new(Rgba::TRANSPARENT),
            PaletteEntry::with_name(Rgba::new(10, 20, 30, 40), "ink"),
        ]);
        let chunk = build_palette(&p).expect("non-empty palette");
        assert_eq!(chunk.first_color, 0);
        assert_eq!(chunk.entries.len(), 2);
        assert_eq!(chunk.entries[1].color.red, 10);
        assert_eq!(chunk.entries[1].name.as_deref(), Some("ink"));
    }

    #[test]
    fn map_blend_mode_covers_all_variants() {
        assert!(matches!(
            map_blend_mode(BlendMode::Normal),
            AseBlendMode::Normal
        ));
        assert!(matches!(
            map_blend_mode(BlendMode::Divide),
            AseBlendMode::Divide
        ));
        assert!(matches!(
            map_blend_mode(BlendMode::Color),
            AseBlendMode::Color
        ));
    }

    #[test]
    fn map_tag_direction_covers_all_variants() {
        assert!(matches!(
            map_tag_direction(TagDirection::Forward),
            AseDirection::Forward
        ));
        assert!(matches!(
            map_tag_direction(TagDirection::PingpongReverse),
            AseDirection::PingPongReverse
        ));
    }
}
