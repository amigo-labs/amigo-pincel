//! Aseprite v1.3 read adapter.
//!
//! Wraps [`aseprite_loader`] and translates its parser output into Pincel's
//! [`Sprite`] / [`CelMap`] pair. See `docs/specs/pincel.md` §7.1.
//!
//! Current scope:
//!
//! - **RGBA color mode only.** Indexed / grayscale return
//!   [`CodecError::UnsupportedColorMode`].
//! - **Image, group, and tilemap layers.** Tilemap layers carry their
//!   `tileset_index` through as [`LayerKind::Tilemap`]; tileset chunks
//!   (`0x2023`) hydrate into [`crate::Tileset`] entries on the sprite, and
//!   compressed-tilemap cels (Cel Type 3) hydrate into [`CelData::Tilemap`]
//!   grids.
//! - **Slice chunks round-trip.** `0x2022` entries hydrate into
//!   [`crate::Slice`] entries on the sprite; per-key 9-patch and pivot
//!   data ride along on each [`crate::SliceKey`]. The on-disk format
//!   does not carry the editor-only [`crate::SliceId`] or overlay color,
//!   so IDs are assigned sequentially by appearance order and colors
//!   default to white.
//! - **Linked cels are preserved** as [`CelData::Linked`] — round-trip via the
//!   M5 writer remains lossless.
//! - **External-file tilesets are not yet supported.** A tileset whose
//!   `EXTERNAL_FILE` flag is set without `TILES` raises
//!   [`CodecError::TilesetUnsupported`]. A tileset that omits *both* flags
//!   is preserved as a zero-tile tileset so the layer's `tileset_index`
//!   still resolves.

use std::collections::BTreeMap;

use aseprite_loader::binary::blend_mode::BlendMode as AseBlendMode;
use aseprite_loader::binary::chunk::Chunk;
use aseprite_loader::binary::chunks::cel::CelContent;
use aseprite_loader::binary::chunks::layer::{LayerFlags, LayerType};
use aseprite_loader::binary::chunks::tags::{AnimationDirection, Tag as AseTag};
use aseprite_loader::binary::chunks::tileset::{TilesetChunk, TilesetFlags};
use aseprite_loader::binary::color_depth::ColorDepth;
use aseprite_loader::binary::file::{File as AseFile, parse_file};
use aseprite_loader::binary::image::Image as AseImage;
use aseprite_loader::binary::raw_file::parse_raw_file;
use aseprite_loader::loader::decompress;

use super::error::CodecError;
use crate::document::{
    BlendMode, Cel, CelData, CelMap, ColorMode, Frame, FrameIndex, Layer, LayerId, LayerKind,
    PaletteEntry, PathRef, PixelBuffer, Rgba, Slice, SliceId, SliceKey, Sprite, Tag, TagDirection,
    TileImage, TileRef, Tileset, TilesetId, UserData,
};
use crate::geometry::Rect;

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
    // We use the low-level `parse_file` rather than the high-level
    // `AsepriteFile::load`. `AsepriteFile::load` (a) filters tilemap
    // *layers* out of its `ase.layers` view (we already worked around this
    // by reading from `ase.file.layers`) and (b) errors out with
    // `"invalid cel"` when it encounters a Cel Type 3 (compressed tilemap)
    // entry — which would block M8.4 entirely. Going one level lower keeps
    // the data the adapter actually needs (header, layers, frames, palette,
    // tags) while exposing tilemap cels for our own decode.
    let file = parse_file(bytes).map_err(|e| CodecError::Parse(e.to_string()))?;

    let color_mode = map_color_mode(file.header.color_depth)?;

    let mut layers = Vec::with_capacity(file.layers.len());
    // `parent_stack[d]` is the most recently opened group at depth `d`. When a
    // layer at depth `d` arrives, its parent is `parent_stack[d - 1]`; deeper
    // entries from prior siblings are dropped before we (re)open this depth.
    let mut parent_stack: Vec<LayerId> = Vec::new();
    for (index, layer_chunk) in file.layers.iter().enumerate() {
        let depth = usize::from(layer_chunk.child_level);
        let parent = if depth == 0 {
            None
        } else {
            parent_stack.get(depth - 1).copied()
        };
        let mut layer = map_layer(index, layer_chunk)?;
        layer.parent = parent;
        if depth < parent_stack.len() {
            parent_stack.truncate(depth);
        }
        if matches!(layer.kind, LayerKind::Group) {
            parent_stack.push(layer.id);
        }
        layers.push(layer);
    }

    let mut frames = Vec::with_capacity(file.frames.len());
    for frame in &file.frames {
        frames.push(Frame::new(frame.duration));
    }

    let tags = file.tags.iter().map(map_tag).collect::<Vec<_>>();
    // `parse_file` discards `Chunk::Tileset` and `Chunk::Slice` entries,
    // and only populates `file.palette` for Indexed color depth — so for
    // the RGBA files this adapter accepts, the palette must also come from
    // the raw pass. A second pass via `parse_raw_file` recovers all three;
    // the raw parser is cheap (no decompression on this pass) and the
    // per-tileset image decode happens lazily inside `build_tileset`.
    let RawChunks {
        tilesets,
        slices,
        palette,
        user_data,
    } = extract_raw_chunks(bytes)?;

    let mut sprite_builder =
        Sprite::builder(u32::from(file.header.width), u32::from(file.header.height))
            .color_mode(color_mode)
            .palette(palette);
    for (index, mut layer) in layers.into_iter().enumerate() {
        layer.user_data = user_data.layers.get(index).cloned().flatten();
        sprite_builder = sprite_builder.add_layer(layer);
    }
    for frame in frames {
        sprite_builder = sprite_builder.add_frame(frame);
    }
    for (index, mut tag) in tags.into_iter().enumerate() {
        tag.user_data = user_data.tags.get(index).cloned().flatten();
        sprite_builder = sprite_builder.add_tag(tag);
    }
    for tileset in tilesets {
        sprite_builder = sprite_builder.add_tileset(tileset);
    }
    for slice in slices {
        sprite_builder = sprite_builder.add_slice(slice);
    }
    let mut sprite = sprite_builder.build()?;
    let h = &file.header;
    sprite.metadata.pixel_ratio = (h.pixel_width, h.pixel_height);
    sprite.metadata.grid = Some(Rect::new(
        i32::from(h.grid_x),
        i32::from(h.grid_y),
        u32::from(h.grid_width),
        u32::from(h.grid_height),
    ));
    sprite.metadata.user_data = user_data.sprite;

    let cels = build_cels(&file, color_mode, &sprite, &user_data.cels)?;

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
        LayerType::Tilemap => {
            let tileset_index =
                layer
                    .tileset_index
                    .ok_or_else(|| CodecError::TilemapLayerMissingTilesetIndex {
                        name: layer.name.to_string(),
                    })?;
            LayerKind::Tilemap {
                tileset_id: TilesetId::new(tileset_index),
            }
        }
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
        // Every bit but visible / editable is kept verbatim (spec §7.1).
        extra_flags: layer.flags.bits()
            & !(LayerFlags::VISIBLE.bits() | LayerFlags::EDITABLE.bits()),
        user_data: None,
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
        // Future Aseprite directions play forward; `map_tag` keeps the
        // raw byte so a save writes it back.
        _ => TagDirection::Forward,
    }
}

// `aseprite-loader` deprecates `Tag::color` because Aseprite ≥1.3 stores
// tag colors in a trailing User Data chunk — but the in-chunk RGB bytes
// are still written (by Aseprite for compatibility and by our own write
// path), so reading them keeps Pincel round-trips lossless.
#[allow(deprecated)]
fn map_tag(tag: &AseTag<'_>) -> Tag {
    let [r, g, b] = tag.color;
    Tag {
        name: tag.name.to_string(),
        from: FrameIndex::new(u32::from(*tag.frames.start())),
        to: FrameIndex::new(u32::from(*tag.frames.end())),
        direction: map_tag_direction(tag.animation_direction),
        color: Rgba { r, g, b, a: 255 },
        repeats: tag.animation_repeat,
        user_data: None,
        raw_direction: match tag.animation_direction {
            AnimationDirection::Unknown(raw) => Some(raw),
            _ => None,
        },
    }
}

fn build_cels(
    file: &AseFile<'_>,
    color_mode: ColorMode,
    sprite: &Sprite,
    cel_user_data: &BTreeMap<(usize, u16), UserData>,
) -> Result<CelMap, CodecError> {
    let mut map = CelMap::new();
    for (frame_index, frame) in file.frames.iter().enumerate() {
        for cel_chunk in frame.cels.iter().filter_map(|c| c.as_ref()) {
            let layer_index = usize::from(cel_chunk.layer_index);
            let layer_id = sprite
                .layers
                .get(layer_index)
                .map(|l| l.id)
                .ok_or(CodecError::LayerIndexOutOfRange { index: layer_index })?;
            let frame_idx = FrameIndex::new(frame_index as u32);

            let data = match &cel_chunk.content {
                CelContent::Image(image) => CelData::Image(decode_image(image, color_mode)?),
                // `parse_file` only structurally parses chunks; it does
                // *not* cross-check that a linked cel's `frame_position`
                // refers to a real frame (the previous high-level
                // `AsepriteFile::load` did that, but we no longer go
                // through it — see the comment at the top of
                // `read_aseprite`). Bounds-check explicitly here so a
                // malformed file surfaces as a structured error rather
                // than as a dangling [`CelData::Linked`] reference.
                CelContent::LinkedCel { frame_position } => {
                    let target = u32::from(*frame_position);
                    if (target as usize) >= file.frames.len() {
                        return Err(CodecError::LinkedFrameNotFound { index: target });
                    }
                    CelData::Linked(FrameIndex::new(target))
                }
                CelContent::CompressedTilemap {
                    width,
                    height,
                    bits_per_tile,
                    bitmask_tile_id,
                    bitmask_x_flip,
                    bitmask_y_flip,
                    bitmask_diagonal_flip,
                    data,
                } => decode_tilemap_cel(
                    (*width, *height),
                    *bits_per_tile,
                    TilemapMasks {
                        tile_id: *bitmask_tile_id,
                        x_flip: *bitmask_x_flip,
                        y_flip: *bitmask_y_flip,
                        diagonal_flip: *bitmask_diagonal_flip,
                    },
                    data,
                )?,
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
                user_data: cel_user_data
                    .get(&(frame_index, cel_chunk.layer_index))
                    .cloned(),
                z_index: cel_chunk.z_index,
            });
        }
    }
    Ok(map)
}

/// Per-cel bitmasks for the four packed fields of a Cel Type 3 entry.
#[derive(Debug, Copy, Clone)]
struct TilemapMasks {
    tile_id: u32,
    x_flip: u32,
    y_flip: u32,
    diagonal_flip: u32,
}

/// Decode a Cel Type 3 (Compressed Tilemap) payload into a [`CelData::Tilemap`].
///
/// The on-disk grid is row-major; each tile is a fixed-width little-endian
/// unsigned integer whose layout is described by the per-cel bitmasks. The
/// Aseprite v1.3 spec pins `bits_per_tile = 32`; anything else is rejected so
/// that the bitmask layout stays well-defined and the per-tile stride matches
/// the buffer math here.
fn decode_tilemap_cel(
    grid: (u16, u16),
    bits_per_tile: u16,
    masks: TilemapMasks,
    data: &[u8],
) -> Result<CelData, CodecError> {
    if bits_per_tile != 32 {
        return Err(CodecError::TilemapBitsPerTileUnsupported {
            bits: bits_per_tile,
        });
    }
    let grid_w = u32::from(grid.0);
    let grid_h = u32::from(grid.1);
    let tile_count = (grid_w as usize) * (grid_h as usize);
    let expected_bytes = tile_count.checked_mul(4).ok_or_else(|| {
        CodecError::TilemapDecode("tilemap byte size overflowed usize".to_string())
    })?;
    let mut buf = vec![0u8; expected_bytes];
    decompress(data, &mut buf).map_err(|e| CodecError::TilemapDecode(format!("{e:?}")))?;
    let mut tiles = Vec::with_capacity(tile_count);
    for chunk in buf.as_chunks::<4>().0 {
        let raw = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        tiles.push(TileRef {
            tile_id: raw & masks.tile_id,
            flip_x: (raw & masks.x_flip) != 0,
            flip_y: (raw & masks.y_flip) != 0,
            rotate_90: (raw & masks.diagonal_flip) != 0,
        });
    }
    Ok(CelData::Tilemap {
        grid_w,
        grid_h,
        tiles,
    })
}

/// Re-parse `bytes` via `parse_raw_file` and pull out every
/// [`Chunk::Tileset`], [`Chunk::Slice`], and [`Chunk::Palette`] across all
/// frames. The high-level [`parse_file`] used by [`AsepriteFile::load`]
/// discards tilesets and slices and only fills `file.palette` for Indexed
/// color depth (see `aseprite-loader` 0.4.2 `file.rs`), so the raw pass is
/// the only path that surfaces them for RGBA files. The order tilesets and
/// slices appeared in the source file is preserved.
///
/// Palette folding: each `0x2019` chunk overwrites the entry range it
/// declares (`first_color_index ..= last_color_index`), later chunks
/// winning — matching the loader's own multi-frame palette semantics.
/// Slots below `first_color_index` that no chunk ever wrote default to
/// transparent. Entry names are recovered here too — the raw chunk is the
/// only source that carries them. Legacy palette chunks (`0x0004` /
/// `0x0011`) are not folded; Aseprite always writes `0x2019` alongside
/// them and our own writer emits only `0x2019`.
/// User data recovered from the raw chunk stream (spec §7.1). Aseprite
/// attaches a User Data chunk to the chunk right before it; after a Tags
/// chunk, consecutive User Data chunks belong to the tags in order.
#[derive(Default)]
struct RawUserData {
    sprite: Option<UserData>,
    /// By layer index (first-frame layer chunk order).
    layers: Vec<Option<UserData>>,
    /// By `(frame index, layer index)`.
    cels: BTreeMap<(usize, u16), UserData>,
    /// By tag index.
    tags: Vec<Option<UserData>>,
}

/// Everything [`extract_raw_chunks`] recovers.
struct RawChunks {
    tilesets: Vec<Tileset>,
    slices: Vec<Slice>,
    palette: crate::document::Palette,
    user_data: RawUserData,
}

/// What the next User Data chunk attaches to.
#[derive(Clone, Copy)]
enum UserDataTarget {
    None,
    Sprite,
    Slice(usize),
    Layer(usize),
    Cel(usize, u16),
    /// The `n`-th tag after a Tags chunk.
    Tag(usize),
}

fn extract_raw_chunks(bytes: &[u8]) -> Result<RawChunks, CodecError> {
    // `parse_raw_file` independently validates the header magic and frame
    // envelopes; we run it after `parse_file` succeeded in the caller so
    // the cost is a second header / chunk walk (no decompression). Any
    // error here means the byte stream changed between the two passes,
    // which shouldn't happen for an in-memory slice — propagate it rather
    // than silently dropping tilesets and slices.
    let raw = parse_raw_file(bytes).map_err(|e| CodecError::Parse(e.to_string()))?;
    // Property maps, by User Data chunk in file order (our own walk: the
    // loader's typed parse mis-reads nested maps, and Pincel only needs
    // the bytes back).
    let mut properties = user_data_property_blocks(bytes).into_iter();
    let mut tilesets = Vec::new();
    let mut slices = Vec::new();
    let mut palette_entries: Vec<PaletteEntry> = Vec::new();
    // Aseprite attaches User Data to the immediately preceding chunk;
    // `target` tracks what that chunk was.
    let mut user_data = RawUserData::default();
    let mut layer_count = 0usize;
    let mut target = UserDataTarget::None;
    for (frame_index, frame) in raw.frames.iter().enumerate() {
        for chunk in &frame.chunks {
            match chunk {
                Chunk::Layer(_) => {
                    target = UserDataTarget::Layer(layer_count);
                    layer_count += 1;
                }
                Chunk::Cel(c) => target = UserDataTarget::Cel(frame_index, c.layer_index),
                Chunk::Tags(_) => target = UserDataTarget::Tag(0),
                Chunk::Tileset(ts) => {
                    tilesets.push(build_tileset(ts)?);
                    target = UserDataTarget::None;
                }
                Chunk::Slice(s) => {
                    // `SliceId` is a `u32`; on 64-bit hosts `slices.len()` is
                    // a `usize` and a bare `as u32` would silently truncate
                    // past `u32::MAX` and assign duplicate ids. Surface the
                    // overflow as a structured error to match the writer's
                    // own `OutOfRange` pre-validation.
                    let id_raw =
                        u32::try_from(slices.len()).map_err(|_| CodecError::OutOfRange {
                            what: "slice count",
                            value: slices.len() as i64,
                        })?;
                    slices.push(build_slice(SliceId::new(id_raw), s)?);
                    target = UserDataTarget::Slice(slices.len() - 1);
                }
                Chunk::UserData(ud) => {
                    let data = UserData {
                        text: ud.text.map(str::to_string),
                        color: ud.color.map(|c| Rgba::new(c.red, c.green, c.blue, c.alpha)),
                        properties: properties.next().flatten(),
                    };
                    let data = (!data.is_empty()).then_some(data);
                    // Each target takes one chunk, except tags: those
                    // consume consecutive chunks in tag order.
                    match target {
                        UserDataTarget::None => {}
                        UserDataTarget::Sprite => user_data.sprite = data,
                        // A slice's User Data color is its overlay color
                        // (otherwise white, see `build_slice`).
                        UserDataTarget::Slice(idx) => {
                            if let Some(c) = data.and_then(|d| d.color) {
                                slices[idx].color = c;
                            }
                        }
                        UserDataTarget::Layer(idx) => {
                            if user_data.layers.len() <= idx {
                                user_data.layers.resize(idx + 1, None);
                            }
                            user_data.layers[idx] = data;
                        }
                        UserDataTarget::Cel(f, l) => {
                            if let Some(d) = data {
                                user_data.cels.insert((f, l), d);
                            }
                        }
                        UserDataTarget::Tag(n) => {
                            if user_data.tags.len() <= n {
                                user_data.tags.resize(n + 1, None);
                            }
                            user_data.tags[n] = data;
                            target = UserDataTarget::Tag(n + 1);
                            continue;
                        }
                    }
                    target = UserDataTarget::None;
                }
                Chunk::Palette(p) => {
                    let first = usize::from(*p.indices.start());
                    let needed = first + p.entries.len();
                    if palette_entries.len() < needed {
                        palette_entries.resize(needed, PaletteEntry::new(Rgba::TRANSPARENT));
                    }
                    for (offset, entry) in p.entries.iter().enumerate() {
                        let rgba = Rgba::new(
                            entry.color.red,
                            entry.color.green,
                            entry.color.blue,
                            entry.color.alpha,
                        );
                        palette_entries[first + offset] = match entry.name {
                            Some(name) => PaletteEntry::with_name(rgba, name),
                            None => PaletteEntry::new(rgba),
                        };
                    }
                    // Aseprite v1.3: User Data right after the first
                    // frame's palette belongs to the sprite.
                    target = if frame_index == 0 {
                        UserDataTarget::Sprite
                    } else {
                        UserDataTarget::None
                    };
                }
                _ => target = UserDataTarget::None,
            }
        }
    }
    Ok(RawChunks {
        tilesets,
        slices,
        palette: crate::document::Palette::from_entries(palette_entries),
        user_data,
    })
}

/// The property-map block (flag `0x4`) of every User Data chunk
/// (`0x2020`), in file order; `None` for a chunk without one. Walks the
/// frame / chunk envelopes directly (header 128 bytes; frame header 16
/// bytes with its byte length first; chunk = `DWORD size`, `WORD type`,
/// body). A truncated envelope ends the walk: the loader has already
/// validated the file, so that only happens on malformed property
/// blocks, which are then dropped rather than failing the open.
fn user_data_property_blocks(bytes: &[u8]) -> Vec<Option<Vec<u8>>> {
    fn dword(b: &[u8], at: usize) -> Option<usize> {
        let s = b.get(at..at + 4)?;
        Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize)
    }
    fn word(b: &[u8], at: usize) -> Option<usize> {
        let s = b.get(at..at + 2)?;
        Some(usize::from(u16::from_le_bytes([s[0], s[1]])))
    }
    // One User Data body → its property block, if flagged and well-formed.
    fn props(body: &[u8]) -> Option<Vec<u8>> {
        let flags = dword(body, 0)?;
        if flags & 0x4 == 0 {
            return None;
        }
        let mut at = 4;
        if flags & 0x1 != 0 {
            at += 2 + word(body, at)?;
        }
        if flags & 0x2 != 0 {
            at += 4;
        }
        let size = dword(body, at)?;
        if size < 8 {
            return None;
        }
        body.get(at..at + size).map(<[u8]>::to_vec)
    }

    let mut out = Vec::new();
    let mut frame_at = 128;
    while let Some(frame_len) = dword(bytes, frame_at) {
        let frame_end = frame_at.saturating_add(frame_len).min(bytes.len());
        if frame_len < 16 {
            break;
        }
        let mut chunk_at = frame_at + 16;
        while chunk_at + 6 <= frame_end {
            let (Some(size), Some(kind)) = (dword(bytes, chunk_at), word(bytes, chunk_at + 4))
            else {
                break;
            };
            if size < 6 || chunk_at + size > frame_end {
                break;
            }
            if kind == 0x2020 {
                out.push(props(&bytes[chunk_at + 6..chunk_at + size]));
            }
            chunk_at += size;
        }
        frame_at = frame_end;
        if frame_at >= bytes.len() {
            break;
        }
    }
    out
}

/// Hydrate a single loader [`SliceChunkRef`] (see
/// `aseprite-loader::binary::chunks::slice`) into a document [`Slice`].
/// The on-disk slice chunk carries no overlay color or stable id; both
/// are editor-only metadata. The id is assigned by the caller from the
/// slice's appearance order. The color defaults to opaque white here and
/// is overwritten by [`extract_tilesets_and_slices`] when a trailing
/// User Data chunk (`0x2020`) supplies one.
fn build_slice(
    id: SliceId,
    chunk: &aseprite_loader::binary::chunks::slice::SliceChunk<'_>,
) -> Result<Slice, CodecError> {
    let keys = chunk.slice_keys.iter().map(map_slice_key).collect();
    Ok(Slice {
        id,
        name: chunk.name.to_string(),
        color: Rgba::WHITE,
        keys,
    })
}

fn map_slice_key(key: &aseprite_loader::binary::chunks::slice::SliceKey) -> SliceKey {
    SliceKey {
        frame: FrameIndex::new(key.frame_number),
        bounds: Rect::new(key.x, key.y, key.width, key.height),
        center: key
            .nine_patch
            .as_ref()
            .map(|np| Rect::new(np.x, np.y, np.width, np.height)),
        pivot: key.pivot.as_ref().map(|p| (p.x, p.y)),
    }
}

/// Hydrate a single [`TilesetChunk`] into [`Tileset`].
///
/// Inline tile data (`TilesetFlags::TILES`) is decompressed; the on-disk
/// layout is `tile_w × (tile_h × number_of_tiles)` RGBA8, with tiles stacked
/// vertically. Tile id `0` is the empty / transparent tile by Aseprite
/// convention; whether the file stores an explicit tile-0 entry is
/// preserved verbatim so the [`Tileset::tiles`] indices line up with
/// [`TileRef::tile_id`].
///
/// External-file tilesets (`TilesetFlags::EXTERNAL_FILE` without
/// `TilesetFlags::TILES`) raise [`CodecError::TilesetUnsupported`]: Phase 1
/// supports inline tile data only.
fn build_tileset(chunk: &TilesetChunk<'_>) -> Result<Tileset, CodecError> {
    let id = chunk.id;
    let tile_w = u32::from(chunk.width);
    let tile_h = u32::from(chunk.height);
    let n_tiles = chunk.number_of_tiles as usize;

    let tiles = if chunk.flags.contains(TilesetFlags::TILES) {
        let tiles_block = chunk.tiles.as_ref().ok_or(CodecError::TilesetUnsupported {
            id,
            what: "TILES flag set but no inline tile data",
        })?;
        let bytes_per_tile = (tile_w as usize)
            .checked_mul(tile_h as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(CodecError::TilesetDecode {
                id,
                message: "per-tile byte size overflowed usize".into(),
            })?;
        let total = bytes_per_tile
            .checked_mul(n_tiles)
            .ok_or(CodecError::TilesetDecode {
                id,
                message: "tileset byte size overflowed usize".into(),
            })?;
        let mut buf = vec![0u8; total];
        decompress(tiles_block.data, &mut buf).map_err(|e| CodecError::TilesetDecode {
            id,
            message: format!("{e:?}"),
        })?;
        buf.chunks_exact(bytes_per_tile)
            .map(|tile_bytes| TileImage {
                pixels: PixelBuffer {
                    width: tile_w,
                    height: tile_h,
                    color_mode: ColorMode::Rgba,
                    data: tile_bytes.to_vec(),
                },
            })
            .collect()
    } else if chunk.flags.contains(TilesetFlags::EXTERNAL_FILE) {
        return Err(CodecError::TilesetUnsupported {
            id,
            what: "external-file tilesets are not yet supported",
        });
    } else {
        // No inline tiles and no external file reference — Aseprite still
        // emits a tileset chunk in this corner case; keep it as a
        // zero-tile tileset so the layer's `tileset_index` resolves.
        Vec::new()
    };

    Ok(Tileset {
        id: TilesetId::new(id),
        name: chunk.name.to_string(),
        tile_size: (tile_w, tile_h),
        tiles,
        base_index: i32::from(chunk.base_index),
        // The external_file_id in the chunk is an opaque pointer into the
        // External Files Chunk (0x2008); resolving it to a `PathRef` is
        // deferred to a follow-up. Keep the slot empty rather than guess.
        external_file: None::<PathRef>,
    })
}

/// Decode a low-level `aseprite-loader` image straight from its `CelContent`
/// payload, decompressing inline when the cel was zlib-encoded. Avoids the
/// detour through `AsepriteFile::load_image` (which requires correlating the
/// low-level cel chunk with the high-level frame table — a mapping that is
/// awkward to bounds-check and historically caused a misleading
/// "layer index out of range" error on lookup mismatch).
fn decode_image(image: &AseImage<'_>, color_mode: ColorMode) -> Result<PixelBuffer, CodecError> {
    // M4 is RGBA-only; the RGBA branch is the only one map_color_mode allows
    // through, so we don't need to dispatch on color mode here.
    debug_assert_eq!(color_mode, ColorMode::Rgba);
    let width = u32::from(image.width);
    let height = u32::from(image.height);
    let target_size = (width as usize) * (height as usize) * color_mode.bytes_per_pixel();
    let mut data = vec![0u8; target_size];
    if image.compressed {
        decompress(image.data, &mut data).map_err(|e| CodecError::Image(format!("{e:?}")))?;
    } else {
        if image.data.len() < target_size {
            return Err(CodecError::Image(format!(
                "raw cel pixel payload {} bytes < expected {} bytes",
                image.data.len(),
                target_size
            )));
        }
        data.copy_from_slice(&image.data[..target_size]);
    }
    Ok(PixelBuffer {
        width,
        height,
        color_mode,
        data,
    })
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
