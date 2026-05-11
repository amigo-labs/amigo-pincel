//! `compose()` — the single composition entry point. See `docs/specs/pincel.md` §4.
//!
//! M3 implements the minimum useful path: visible image layers in z-order
//! with the `Normal` blend mode, RGBA color mode only. Tilemap and group
//! layers, indexed color, non-Normal blend modes, linked cels, onion skin,
//! and overlays all return [`RenderError`] for now. The `dirty_hint` field
//! on the request is accepted and currently ignored.

use thiserror::Error;

use crate::document::{
    BlendMode, CelData, CelMap, ColorMode, FrameIndex, Layer, LayerId, LayerKind, PixelBuffer,
    Sprite, TileRef, Tileset, TilesetId,
};
use crate::geometry::Rect;

use super::request::{ComposeRequest, ComposeResult, LayerFilter, Overlays};

/// Maximum supported zoom factor (per spec §4.1).
const MAX_ZOOM: u32 = 64;

/// Errors raised by [`compose`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RenderError {
    /// The sprite uses a color mode the renderer cannot yet handle.
    #[error("unsupported color mode: {mode:?}")]
    UnsupportedColorMode { mode: ColorMode },

    /// `zoom` was outside the supported range `1..=64`.
    #[error("invalid zoom: {zoom} (expected 1..={MAX_ZOOM})")]
    InvalidZoom { zoom: u32 },

    /// `viewport` was empty (zero width or height).
    #[error("empty viewport")]
    EmptyViewport,

    /// `frame` did not refer to a frame in the sprite.
    #[error("unknown frame index: {frame:?}")]
    UnknownFrame { frame: FrameIndex },

    /// A layer's content cannot be composed in this milestone.
    #[error("unsupported layer kind on layer {layer:?}")]
    UnsupportedLayerKind { layer: LayerId },

    /// A layer's blend mode is not yet implemented.
    #[error("unsupported blend mode {mode:?} on layer {layer:?}")]
    UnsupportedBlendMode { layer: LayerId, mode: BlendMode },

    /// A linked cel was encountered. Linked cels share data with another
    /// frame's cel; M3 does not follow links — the loader (M4) is the layer
    /// that resolves linkage.
    #[error("linked cel on layer {layer:?} frame {frame:?} is not yet supported")]
    LinkedCelUnsupported { layer: LayerId, frame: FrameIndex },

    /// A cel's pixel buffer uses a color mode that doesn't match the
    /// sprite's color mode.
    #[error(
        "cel buffer color mode {mode:?} on layer {layer:?} frame {frame:?} \
         doesn't match sprite color mode"
    )]
    CelColorModeMismatch {
        layer: LayerId,
        frame: FrameIndex,
        mode: ColorMode,
    },

    /// A cel's pixel buffer dimensions don't match its byte length.
    #[error("malformed cel buffer on layer {layer:?} frame {frame:?}")]
    MalformedCelBuffer { layer: LayerId, frame: FrameIndex },

    /// A cel's payload type is incompatible with its layer's kind (for
    /// example, tilemap data on an image layer). Indicates a corrupt
    /// document.
    #[error("cel type does not match layer kind on layer {layer:?} frame {frame:?}")]
    CelTypeMismatch { layer: LayerId, frame: FrameIndex },

    /// The request asked for an onion-skin overlay; M3 does not render
    /// onion skin yet.
    #[error("onion skin is not yet supported")]
    OnionSkinUnsupported,

    /// The request asked for one or more decoration overlays; M3 does not
    /// render overlays yet.
    #[error("overlays are not yet supported")]
    OverlaysUnsupported,

    /// A tilemap layer references a tileset id that doesn't exist on the
    /// sprite.
    #[error("tileset {tileset:?} for layer {layer:?} not found")]
    TilesetNotFound { layer: LayerId, tileset: TilesetId },

    /// A tilemap cel references a tile id that's past the end of the
    /// tileset's tile list. Indicates a corrupt document or a stale cel
    /// after tiles were removed.
    #[error("tile id {tile_id} on layer {layer:?} frame {frame:?} is out of range")]
    TileIdOutOfRange {
        layer: LayerId,
        frame: FrameIndex,
        tile_id: u32,
    },

    /// A tile image's dimensions don't match the tileset's declared
    /// `tile_size`.
    #[error("tile {tile_id} dimensions don't match tileset {tileset:?} on layer {layer:?}")]
    TileSizeMismatch {
        layer: LayerId,
        tileset: TilesetId,
        tile_id: u32,
    },

    /// A `TileRef::rotate_90` flag was set on a non-square tileset. Phase 1
    /// only supports 90° rotation on square tiles; non-square rotation is
    /// deferred to Phase 2.
    #[error(
        "rotate_90 on non-square tileset {tileset:?} (tile_size {tile_size:?}) \
         is not yet supported"
    )]
    NonSquareRotateUnsupported {
        layer: LayerId,
        tileset: TilesetId,
        tile_size: (u32, u32),
    },
}

/// Compose a frame of `sprite` into an RGBA8 pixel buffer. See spec §4.
pub fn compose(
    sprite: &Sprite,
    cels: &CelMap,
    request: &ComposeRequest,
) -> Result<ComposeResult, RenderError> {
    if sprite.color_mode != ColorMode::Rgba {
        return Err(RenderError::UnsupportedColorMode {
            mode: sprite.color_mode,
        });
    }
    if request.zoom == 0 || request.zoom > MAX_ZOOM {
        return Err(RenderError::InvalidZoom { zoom: request.zoom });
    }
    if request.viewport.is_empty() {
        return Err(RenderError::EmptyViewport);
    }
    if request.onion_skin.is_some() {
        return Err(RenderError::OnionSkinUnsupported);
    }
    if request.overlays != Overlays::default() {
        return Err(RenderError::OverlaysUnsupported);
    }
    if (request.frame.0 as usize) >= sprite.frames.len() {
        return Err(RenderError::UnknownFrame {
            frame: request.frame,
        });
    }

    let vp = request.viewport;
    let mut buffer = vec![0u8; (vp.width as usize) * (vp.height as usize) * 4];

    for layer in sprite.layers.iter() {
        if !layer_included(layer, &request.include_layers) {
            continue;
        }
        if let LayerKind::Group = layer.kind {
            return Err(RenderError::UnsupportedLayerKind { layer: layer.id });
        }
        if !matches!(layer.blend_mode, BlendMode::Normal) {
            return Err(RenderError::UnsupportedBlendMode {
                layer: layer.id,
                mode: layer.blend_mode,
            });
        }
        let Some(cel) = cels.get(layer.id, request.frame) else {
            continue;
        };
        match (&layer.kind, &cel.data) {
            (LayerKind::Image, CelData::Image(pixels)) => {
                if pixels.color_mode != sprite.color_mode {
                    return Err(RenderError::CelColorModeMismatch {
                        layer: layer.id,
                        frame: request.frame,
                        mode: pixels.color_mode,
                    });
                }
                if !pixels.is_well_formed() {
                    return Err(RenderError::MalformedCelBuffer {
                        layer: layer.id,
                        frame: request.frame,
                    });
                }
                composite_image_cel(
                    &mut buffer,
                    vp,
                    cel.position,
                    pixels,
                    layer.opacity,
                    cel.opacity,
                );
            }
            (
                LayerKind::Tilemap { tileset_id },
                CelData::Tilemap {
                    grid_w,
                    grid_h,
                    tiles,
                },
            ) => {
                let tileset = sprite
                    .tileset(*tileset_id)
                    .ok_or(RenderError::TilesetNotFound {
                        layer: layer.id,
                        tileset: *tileset_id,
                    })?;
                composite_tilemap_cel(
                    &mut buffer,
                    vp,
                    cel.position,
                    *grid_w,
                    *grid_h,
                    tiles,
                    tileset,
                    layer.id,
                    request.frame,
                    sprite.color_mode,
                    layer.opacity,
                    cel.opacity,
                )?;
            }
            (_, CelData::Linked(_)) => {
                return Err(RenderError::LinkedCelUnsupported {
                    layer: layer.id,
                    frame: request.frame,
                });
            }
            (LayerKind::Image, CelData::Tilemap { .. })
            | (LayerKind::Tilemap { .. }, CelData::Image(_)) => {
                return Err(RenderError::CelTypeMismatch {
                    layer: layer.id,
                    frame: request.frame,
                });
            }
            (LayerKind::Group, _) => {
                // Group layers are rejected above before the cel lookup.
                unreachable!("group layers handled before cel lookup");
            }
        }
    }

    let pixels = if request.zoom == 1 {
        buffer
    } else {
        upscale_nearest(&buffer, vp.width, vp.height, request.zoom)
    };

    Ok(ComposeResult {
        pixels,
        width: vp.width * request.zoom,
        height: vp.height * request.zoom,
        generation: 0,
    })
}

/// Nearest-neighbor integer upscale of an RGBA8 image. See spec §4.1: the
/// composer produces the exact pixel grid the UI displays so the GPU just
/// blits and we avoid subpixel sampling artifacts.
fn upscale_nearest(src: &[u8], w: u32, h: u32, zoom: u32) -> Vec<u8> {
    let zoom_us = zoom as usize;
    let w_us = w as usize;
    let h_us = h as usize;
    let zw = w_us * zoom_us;
    let mut out = vec![0u8; zw * h_us * zoom_us * 4];

    for y in 0..h_us {
        // Build the first replicated row for this source row, then memcpy
        // it `zoom - 1` times to fill the remaining vertical replicas.
        let src_row_start = y * w_us * 4;
        let dst_first_row_start = y * zoom_us * zw * 4;
        for x in 0..w_us {
            let s = src_row_start + x * 4;
            let pixel = &src[s..s + 4];
            let dst_x = dst_first_row_start + x * zoom_us * 4;
            for zx in 0..zoom_us {
                let d = dst_x + zx * 4;
                out[d..d + 4].copy_from_slice(pixel);
            }
        }
        let row_bytes = zw * 4;
        let (head, tail) = out.split_at_mut(dst_first_row_start + row_bytes);
        let row = &head[dst_first_row_start..dst_first_row_start + row_bytes];
        for zy in 1..zoom_us {
            let dst_offset = (zy - 1) * row_bytes;
            tail[dst_offset..dst_offset + row_bytes].copy_from_slice(row);
        }
    }

    out
}

fn layer_included(layer: &Layer, filter: &LayerFilter) -> bool {
    match filter {
        LayerFilter::Visible => layer.visible,
        LayerFilter::All => true,
        LayerFilter::Only(ids) => ids.contains(&layer.id),
    }
}

fn composite_image_cel(
    dst: &mut [u8],
    viewport: Rect,
    cel_pos: (i32, i32),
    src: &PixelBuffer,
    layer_opacity: u8,
    cel_opacity: u8,
) {
    let combined_opacity = mul_u8(layer_opacity, cel_opacity);
    if combined_opacity == 0 {
        return;
    }

    let vp_w = i64::from(viewport.width);
    let vp_h = i64::from(viewport.height);
    let vp_x = i64::from(viewport.x);
    let vp_y = i64::from(viewport.y);

    let cel_w = i64::from(src.width);
    let cel_h = i64::from(src.height);
    let cel_x = i64::from(cel_pos.0);
    let cel_y = i64::from(cel_pos.1);

    // Intersection of cel and viewport, in sprite coordinates.
    let x_start = cel_x.max(vp_x);
    let y_start = cel_y.max(vp_y);
    let x_end = (cel_x + cel_w).min(vp_x + vp_w);
    let y_end = (cel_y + cel_h).min(vp_y + vp_h);

    if x_start >= x_end || y_start >= y_end {
        return;
    }

    let src_stride = (src.width as usize) * 4;
    let dst_stride = (viewport.width as usize) * 4;

    for y in y_start..y_end {
        let src_row = (y - cel_y) as usize * src_stride;
        let dst_row = (y - vp_y) as usize * dst_stride;
        for x in x_start..x_end {
            let s = src_row + (x - cel_x) as usize * 4;
            let d = dst_row + (x - vp_x) as usize * 4;
            let sa = mul_u8(src.data[s + 3], combined_opacity);
            blend_normal_into(
                &mut dst[d..d + 4],
                src.data[s],
                src.data[s + 1],
                src.data[s + 2],
                sa,
            );
        }
    }
}

/// Composite a tilemap cel into the viewport buffer. Iterates the grid in
/// row-major order, looks each tile up in `tileset`, and rasterizes the tile
/// (honoring `flip_x`, `flip_y`, and `rotate_90`) at its sprite-coord
/// position. Tile id `0` is the Aseprite empty / transparent tile and is
/// skipped without consulting the tileset.
#[allow(clippy::too_many_arguments)]
fn composite_tilemap_cel(
    dst: &mut [u8],
    viewport: Rect,
    cel_pos: (i32, i32),
    grid_w: u32,
    grid_h: u32,
    tiles: &[TileRef],
    tileset: &Tileset,
    layer_id: LayerId,
    frame: FrameIndex,
    sprite_color_mode: ColorMode,
    layer_opacity: u8,
    cel_opacity: u8,
) -> Result<(), RenderError> {
    let (tile_w, tile_h) = tileset.tile_size;
    if tile_w == 0 || tile_h == 0 {
        return Ok(());
    }
    // Compute the expected tile-vector length in `usize` so the multiply
    // can't overflow on a 64-bit target. Reject corrupt cels rather than
    // index into a wrongly-sized buffer.
    let expected_len = (grid_w as usize) * (grid_h as usize);
    if tiles.len() != expected_len {
        return Err(RenderError::MalformedCelBuffer {
            layer: layer_id,
            frame,
        });
    }
    for j in 0..grid_h {
        for i in 0..grid_w {
            let tile_ref = tiles[(j as usize) * (grid_w as usize) + (i as usize)];
            if tile_ref.tile_id == 0 {
                // Aseprite empty-tile convention.
                continue;
            }
            if tile_ref.rotate_90 && tile_w != tile_h {
                return Err(RenderError::NonSquareRotateUnsupported {
                    layer: layer_id,
                    tileset: tileset.id,
                    tile_size: (tile_w, tile_h),
                });
            }
            let tile = tileset
                .tile(tile_ref.tile_id)
                .ok_or(RenderError::TileIdOutOfRange {
                    layer: layer_id,
                    frame,
                    tile_id: tile_ref.tile_id,
                })?;
            if tile.pixels.color_mode != sprite_color_mode {
                return Err(RenderError::CelColorModeMismatch {
                    layer: layer_id,
                    frame,
                    mode: tile.pixels.color_mode,
                });
            }
            if tile.pixels.width != tile_w || tile.pixels.height != tile_h {
                return Err(RenderError::TileSizeMismatch {
                    layer: layer_id,
                    tileset: tileset.id,
                    tile_id: tile_ref.tile_id,
                });
            }
            if !tile.pixels.is_well_formed() {
                return Err(RenderError::MalformedCelBuffer {
                    layer: layer_id,
                    frame,
                });
            }
            let tile_x = cel_pos.0.saturating_add_unsigned(i.saturating_mul(tile_w));
            let tile_y = cel_pos.1.saturating_add_unsigned(j.saturating_mul(tile_h));
            composite_transformed_tile(
                dst,
                viewport,
                (tile_x, tile_y),
                &tile.pixels,
                tile_ref,
                layer_opacity,
                cel_opacity,
            );
        }
    }
    Ok(())
}

/// Blit one tile into the viewport, applying flip / rotate as a per-pixel
/// source coordinate transform. Pixel iteration drives the destination
/// (sprite-coord) space; for each output pixel we compute the corresponding
/// source pixel in the tile's local frame. Order of operations matches
/// Aseprite: `rotate_90` first (clockwise 90°), then `flip_x` then `flip_y`.
fn composite_transformed_tile(
    dst: &mut [u8],
    viewport: Rect,
    tile_pos: (i32, i32),
    src: &PixelBuffer,
    tile_ref: TileRef,
    layer_opacity: u8,
    cel_opacity: u8,
) {
    let combined_opacity = mul_u8(layer_opacity, cel_opacity);
    if combined_opacity == 0 {
        return;
    }

    let w = src.width;
    let h = src.height;
    // After a 90° rotation a non-square tile would have its dest footprint
    // swapped to (h, w); `composite_tilemap_cel` rejects that case before
    // calling us, so for the rotated path we know w == h.
    let dst_w = if tile_ref.rotate_90 { h } else { w };
    let dst_h = if tile_ref.rotate_90 { w } else { h };

    let vp_w = i64::from(viewport.width);
    let vp_h = i64::from(viewport.height);
    let vp_x = i64::from(viewport.x);
    let vp_y = i64::from(viewport.y);

    let dst_w_i = i64::from(dst_w);
    let dst_h_i = i64::from(dst_h);
    let tx = i64::from(tile_pos.0);
    let ty = i64::from(tile_pos.1);

    let x_start = tx.max(vp_x);
    let y_start = ty.max(vp_y);
    let x_end = (tx + dst_w_i).min(vp_x + vp_w);
    let y_end = (ty + dst_h_i).min(vp_y + vp_h);
    if x_start >= x_end || y_start >= y_end {
        return;
    }

    let src_stride = (w as usize) * 4;
    let dst_stride = (viewport.width as usize) * 4;
    let w_minus_1 = w.saturating_sub(1);
    let h_minus_1 = h.saturating_sub(1);

    for y in y_start..y_end {
        let local_y = (y - ty) as u32;
        let dst_row = (y - vp_y) as usize * dst_stride;
        for x in x_start..x_end {
            let local_x = (x - tx) as u32;
            // Map (local_x, local_y) — in dest space — back to the source
            // tile's pixel grid. Apply the inverse of the requested
            // transformation: undo flip_y → undo flip_x → undo rotate_90.
            let mut sx = local_x;
            let mut sy = local_y;
            if tile_ref.flip_y {
                sy = h_minus_1 - sy;
            }
            if tile_ref.flip_x {
                sx = w_minus_1 - sx;
            }
            if tile_ref.rotate_90 {
                // Inverse of "rotate 90° CW" is "rotate 90° CCW":
                // (sx', sy') = (sy, (w - 1) - sx).
                let new_sx = sy;
                let new_sy = w_minus_1 - sx;
                sx = new_sx;
                sy = new_sy;
            }
            let s = (sy as usize) * src_stride + (sx as usize) * 4;
            let d = dst_row + (x - vp_x) as usize * 4;
            let sa = mul_u8(src.data[s + 3], combined_opacity);
            blend_normal_into(
                &mut dst[d..d + 4],
                src.data[s],
                src.data[s + 1],
                src.data[s + 2],
                sa,
            );
        }
    }
}

/// Source-over (`Normal`) blend, non-premultiplied 8-bit channels.
fn blend_normal_into(dst: &mut [u8], sr: u8, sg: u8, sb: u8, sa: u8) {
    if sa == 0 {
        return;
    }
    if sa == 255 {
        dst[0] = sr;
        dst[1] = sg;
        dst[2] = sb;
        dst[3] = 255;
        return;
    }
    let dr = u32::from(dst[0]);
    let dg = u32::from(dst[1]);
    let db = u32::from(dst[2]);
    let da = u32::from(dst[3]);
    let sa32 = u32::from(sa);
    let inv = 255u32 - sa32;
    // αd · (1 − αs), rounded.
    let blend_a = (da * inv + 127) / 255;
    let oa = sa32 + blend_a;
    if oa == 0 {
        dst[3] = 0;
        return;
    }
    dst[0] = ((u32::from(sr) * sa32 + dr * blend_a) / oa) as u8;
    dst[1] = ((u32::from(sg) * sa32 + dg * blend_a) / oa) as u8;
    dst[2] = ((u32::from(sb) * sa32 + db * blend_a) / oa) as u8;
    dst[3] = oa as u8;
}

#[inline]
fn mul_u8(a: u8, b: u8) -> u8 {
    (((u16::from(a)) * (u16::from(b)) + 127) / 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{
        Cel, CelData, Frame, FrameIndex, Layer, LayerId, PixelBuffer, Sprite, TileImage, TileRef,
        Tileset, TilesetId,
    };

    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> PixelBuffer {
        let mut buf = PixelBuffer::empty(w, h, ColorMode::Rgba);
        for px in buf.data.chunks_exact_mut(4) {
            px.copy_from_slice(&rgba);
        }
        buf
    }

    fn one_layer_sprite(w: u32, h: u32, frames: u32) -> Sprite {
        let mut b = Sprite::builder(w, h).add_layer(Layer::image(LayerId::new(0), "bg"));
        for _ in 0..frames {
            b = b.add_frame(Frame::default());
        }
        b.build().expect("test sprite")
    }

    fn full_req(w: u32, h: u32) -> ComposeRequest {
        ComposeRequest::full(FrameIndex::new(0), w, h)
    }

    #[test]
    fn opaque_cel_matches_source() {
        let sprite = one_layer_sprite(2, 2, 1);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(2, 2, [10, 20, 30, 255]),
        ));

        let r = compose(&sprite, &cels, &full_req(2, 2)).unwrap();
        assert_eq!((r.width, r.height), (2, 2));
        assert_eq!(r.pixels, [10u8, 20, 30, 255].repeat(4));
    }

    #[test]
    fn invisible_layer_is_skipped() {
        let mut layer = Layer::image(LayerId::new(0), "bg");
        layer.visible = false;
        let sprite = Sprite::builder(1, 1)
            .add_layer(layer)
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, [255, 0, 0, 255]),
        ));
        let r = compose(&sprite, &cels, &full_req(1, 1)).unwrap();
        assert_eq!(r.pixels, vec![0, 0, 0, 0]);
    }

    #[test]
    fn opaque_top_layer_overrides_bottom() {
        let sprite = Sprite::builder(1, 1)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_layer(Layer::image(LayerId::new(1), "fg"))
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, [255, 0, 0, 255]),
        ));
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            solid(1, 1, [0, 255, 0, 255]),
        ));
        let r = compose(&sprite, &cels, &full_req(1, 1)).unwrap();
        assert_eq!(r.pixels, vec![0, 255, 0, 255]);
    }

    #[test]
    fn translucent_top_blends_with_bottom() {
        let sprite = Sprite::builder(1, 1)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_layer(Layer::image(LayerId::new(1), "fg"))
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, [255, 0, 0, 255]),
        ));
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            solid(1, 1, [0, 0, 255, 128]),
        ));
        let r = compose(&sprite, &cels, &full_req(1, 1)).unwrap();
        // Hand-derived: sa=128, inv=127, blend_a=(255*127+127)/255=127, oa=255,
        // R = (0*128 + 255*127)/255 = 127
        // B = (255*128 + 0)/255 = 128
        assert_eq!(r.pixels, vec![127, 0, 128, 255]);
    }

    #[test]
    fn layer_opacity_scales_alpha() {
        let mut layer = Layer::image(LayerId::new(0), "bg");
        layer.opacity = 128;
        let sprite = Sprite::builder(1, 1)
            .add_layer(layer)
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, [255, 0, 0, 255]),
        ));
        let r = compose(&sprite, &cels, &full_req(1, 1)).unwrap();
        // mul_u8(128, 255) = 128, so out alpha = 128 over transparent backdrop.
        assert_eq!(r.pixels, vec![255, 0, 0, 128]);
    }

    #[test]
    fn cel_opacity_scales_alpha() {
        let sprite = one_layer_sprite(1, 1, 1);
        let mut cel = Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, [255, 0, 0, 255]),
        );
        cel.opacity = 64;
        let mut cels = CelMap::new();
        cels.insert(cel);
        let r = compose(&sprite, &cels, &full_req(1, 1)).unwrap();
        // mul_u8(255, 64) → (255*64 + 127)/255 = 16447/255 = 64.
        assert_eq!(r.pixels[3], 64);
    }

    #[test]
    fn cel_clipped_to_viewport() {
        let sprite = one_layer_sprite(4, 4, 1);
        let mut cel = Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(2, 2, [100, 100, 100, 255]),
        );
        cel.position = (-1, -1);
        let mut cels = CelMap::new();
        cels.insert(cel);
        let r = compose(&sprite, &cels, &full_req(4, 4)).unwrap();
        // Cel covers sprite coords (-1..1, -1..1); only pixel (0,0) is inside the canvas.
        assert_eq!(&r.pixels[0..4], &[100, 100, 100, 255]);
        assert_eq!(&r.pixels[4..8], &[0, 0, 0, 0]); // (1, 0) is outside cel
    }

    #[test]
    fn viewport_offset_renders_subregion() {
        let sprite = one_layer_sprite(4, 4, 1);
        let mut buf = PixelBuffer::empty(4, 4, ColorMode::Rgba);
        // Mark pixel (2, 2) red, the rest white.
        for px in buf.data.chunks_exact_mut(4) {
            px.copy_from_slice(&[255, 255, 255, 255]);
        }
        let idx = (2 * 4 + 2) * 4;
        buf.data[idx..idx + 4].copy_from_slice(&[255, 0, 0, 255]);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), buf));

        let req = ComposeRequest {
            viewport: Rect::new(2, 2, 2, 2),
            ..ComposeRequest::full(FrameIndex::new(0), 4, 4)
        };
        let r = compose(&sprite, &cels, &req).unwrap();
        assert_eq!((r.width, r.height), (2, 2));
        assert_eq!(&r.pixels[0..4], &[255, 0, 0, 255]);
        assert_eq!(&r.pixels[4..8], &[255, 255, 255, 255]);
    }

    #[test]
    fn rejects_indexed_color_mode() {
        let sprite = Sprite::builder(1, 1)
            .color_mode(ColorMode::Indexed {
                transparent_index: 0,
            })
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let cels = CelMap::new();
        assert_eq!(
            compose(&sprite, &cels, &full_req(1, 1)).unwrap_err(),
            RenderError::UnsupportedColorMode {
                mode: ColorMode::Indexed {
                    transparent_index: 0
                }
            }
        );
    }

    #[test]
    fn rejects_invalid_zoom() {
        let sprite = one_layer_sprite(1, 1, 1);
        let cels = CelMap::new();
        let mut req = full_req(1, 1);
        req.zoom = 0;
        assert!(matches!(
            compose(&sprite, &cels, &req).unwrap_err(),
            RenderError::InvalidZoom { zoom: 0 }
        ));
        req.zoom = 65;
        assert!(matches!(
            compose(&sprite, &cels, &req).unwrap_err(),
            RenderError::InvalidZoom { zoom: 65 }
        ));
    }

    #[test]
    fn rejects_empty_viewport() {
        let sprite = one_layer_sprite(4, 4, 1);
        let cels = CelMap::new();
        let req = ComposeRequest {
            viewport: Rect::new(0, 0, 0, 4),
            ..full_req(4, 4)
        };
        assert_eq!(
            compose(&sprite, &cels, &req).unwrap_err(),
            RenderError::EmptyViewport
        );
    }

    #[test]
    fn rejects_unknown_frame() {
        let sprite = one_layer_sprite(1, 1, 1);
        let cels = CelMap::new();
        let req = ComposeRequest::full(FrameIndex::new(2), 1, 1);
        assert_eq!(
            compose(&sprite, &cels, &req).unwrap_err(),
            RenderError::UnknownFrame {
                frame: FrameIndex::new(2)
            }
        );
    }

    #[test]
    fn rejects_group_layer() {
        let sprite = Sprite::builder(1, 1)
            .add_layer(Layer::group(LayerId::new(7), "grp"))
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let cels = CelMap::new();
        assert_eq!(
            compose(&sprite, &cels, &full_req(1, 1)).unwrap_err(),
            RenderError::UnsupportedLayerKind {
                layer: LayerId::new(7),
            }
        );
    }

    // ---------- M8.2 tilemap compose ----------

    /// Two-tile tileset: tile 0 is the Aseprite empty tile (transparent),
    /// tile 1 is a solid colored tile.
    fn two_tile_tileset(id: u32, tile_size: u32, color: [u8; 4]) -> Tileset {
        let mut ts = Tileset::new(TilesetId::new(id), "tiles", (tile_size, tile_size));
        ts.tiles.push(TileImage {
            pixels: PixelBuffer::empty(tile_size, tile_size, ColorMode::Rgba),
        });
        ts.tiles.push(TileImage {
            pixels: solid(tile_size, tile_size, color),
        });
        ts
    }

    fn tilemap_cel(
        layer: LayerId,
        grid_w: u32,
        grid_h: u32,
        tiles: Vec<TileRef>,
        position: (i32, i32),
    ) -> Cel {
        Cel {
            layer,
            frame: FrameIndex::new(0),
            position,
            opacity: 255,
            data: CelData::Tilemap {
                grid_w,
                grid_h,
                tiles,
            },
        }
    }

    #[test]
    fn tilemap_renders_tile_at_grid_position() {
        // 2x2 grid of 2x2 tiles → 4x4 canvas. Top-left and bottom-right have
        // tile 1 (red); top-right and bottom-left are empty.
        let sprite = Sprite::builder(4, 4)
            .add_layer(Layer::tilemap(LayerId::new(0), "tm", TilesetId::new(0)))
            .add_frame(Frame::default())
            .add_tileset(two_tile_tileset(0, 2, [255, 0, 0, 255]))
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(tilemap_cel(
            LayerId::new(0),
            2,
            2,
            vec![
                TileRef::new(1),
                TileRef::EMPTY,
                TileRef::EMPTY,
                TileRef::new(1),
            ],
            (0, 0),
        ));
        let r = compose(&sprite, &cels, &full_req(4, 4)).unwrap();
        let red = [255, 0, 0, 255];
        let blank = [0, 0, 0, 0];
        // Row 0: red red blank blank
        assert_eq!(&r.pixels[0..4], &red);
        assert_eq!(&r.pixels[4..8], &red);
        assert_eq!(&r.pixels[8..12], &blank);
        assert_eq!(&r.pixels[12..16], &blank);
        // Row 2: blank blank red red
        assert_eq!(&r.pixels[2 * 16..2 * 16 + 4], &blank);
        assert_eq!(&r.pixels[2 * 16 + 4..2 * 16 + 8], &blank);
        assert_eq!(&r.pixels[2 * 16 + 8..2 * 16 + 12], &red);
        assert_eq!(&r.pixels[2 * 16 + 12..2 * 16 + 16], &red);
    }

    #[test]
    fn tilemap_flip_x_mirrors_tile_horizontally() {
        // Tile is asymmetric: top row is red, bottom row is green. With
        // flip_x the rows stay horizontal but each row is mirrored along x.
        // A 2x2 tile mirrored along x is identical to itself (rows have
        // uniform color), so use a 2x2 tile with distinct columns instead.
        let mut tile = PixelBuffer::empty(2, 2, ColorMode::Rgba);
        // (x=0, y=0) red, (x=1, y=0) green, (x=0, y=1) red, (x=1, y=1) green
        tile.data = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255,
        ];
        let mut ts = Tileset::new(TilesetId::new(0), "tiles", (2, 2));
        ts.tiles.push(TileImage {
            pixels: PixelBuffer::empty(2, 2, ColorMode::Rgba),
        });
        ts.tiles.push(TileImage { pixels: tile });

        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::tilemap(LayerId::new(0), "tm", TilesetId::new(0)))
            .add_frame(Frame::default())
            .add_tileset(ts)
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(tilemap_cel(
            LayerId::new(0),
            1,
            1,
            vec![TileRef {
                tile_id: 1,
                flip_x: true,
                flip_y: false,
                rotate_90: false,
            }],
            (0, 0),
        ));
        let r = compose(&sprite, &cels, &full_req(2, 2)).unwrap();
        // Mirrored along x: columns swap.
        // (0, 0) green, (1, 0) red, (0, 1) green, (1, 1) red.
        assert_eq!(&r.pixels[0..4], &[0, 255, 0, 255]);
        assert_eq!(&r.pixels[4..8], &[255, 0, 0, 255]);
        assert_eq!(&r.pixels[8..12], &[0, 255, 0, 255]);
        assert_eq!(&r.pixels[12..16], &[255, 0, 0, 255]);
    }

    #[test]
    fn tilemap_flip_y_mirrors_tile_vertically() {
        // 2x2 tile with distinct rows: top row red, bottom row green. With
        // flip_y rows swap.
        let mut tile = PixelBuffer::empty(2, 2, ColorMode::Rgba);
        tile.data = vec![
            255, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255,
        ];
        let mut ts = Tileset::new(TilesetId::new(0), "tiles", (2, 2));
        ts.tiles.push(TileImage {
            pixels: PixelBuffer::empty(2, 2, ColorMode::Rgba),
        });
        ts.tiles.push(TileImage { pixels: tile });

        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::tilemap(LayerId::new(0), "tm", TilesetId::new(0)))
            .add_frame(Frame::default())
            .add_tileset(ts)
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(tilemap_cel(
            LayerId::new(0),
            1,
            1,
            vec![TileRef {
                tile_id: 1,
                flip_x: false,
                flip_y: true,
                rotate_90: false,
            }],
            (0, 0),
        ));
        let r = compose(&sprite, &cels, &full_req(2, 2)).unwrap();
        // Top row should now be green, bottom row red.
        assert_eq!(&r.pixels[0..4], &[0, 255, 0, 255]);
        assert_eq!(&r.pixels[4..8], &[0, 255, 0, 255]);
        assert_eq!(&r.pixels[8..12], &[255, 0, 0, 255]);
        assert_eq!(&r.pixels[12..16], &[255, 0, 0, 255]);
    }

    #[test]
    fn tilemap_rotate_90_rotates_clockwise() {
        // 2x2 tile:
        //   A B
        //   C D
        // A 90° CW rotation produces:
        //   C A
        //   D B
        let a = [255, 0, 0, 255];
        let b = [0, 255, 0, 255];
        let c = [0, 0, 255, 255];
        let d = [255, 255, 0, 255];
        let mut tile = PixelBuffer::empty(2, 2, ColorMode::Rgba);
        tile.data = [a, b, c, d].concat();
        let mut ts = Tileset::new(TilesetId::new(0), "tiles", (2, 2));
        ts.tiles.push(TileImage {
            pixels: PixelBuffer::empty(2, 2, ColorMode::Rgba),
        });
        ts.tiles.push(TileImage { pixels: tile });

        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::tilemap(LayerId::new(0), "tm", TilesetId::new(0)))
            .add_frame(Frame::default())
            .add_tileset(ts)
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(tilemap_cel(
            LayerId::new(0),
            1,
            1,
            vec![TileRef {
                tile_id: 1,
                flip_x: false,
                flip_y: false,
                rotate_90: true,
            }],
            (0, 0),
        ));
        let r = compose(&sprite, &cels, &full_req(2, 2)).unwrap();
        // Expected: C A / D B
        assert_eq!(&r.pixels[0..4], &c);
        assert_eq!(&r.pixels[4..8], &a);
        assert_eq!(&r.pixels[8..12], &d);
        assert_eq!(&r.pixels[12..16], &b);
    }

    #[test]
    fn tilemap_missing_tileset_errors() {
        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::tilemap(LayerId::new(0), "tm", TilesetId::new(7)))
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(tilemap_cel(
            LayerId::new(0),
            1,
            1,
            vec![TileRef::new(1)],
            (0, 0),
        ));
        assert_eq!(
            compose(&sprite, &cels, &full_req(2, 2)).unwrap_err(),
            RenderError::TilesetNotFound {
                layer: LayerId::new(0),
                tileset: TilesetId::new(7),
            }
        );
    }

    #[test]
    fn tilemap_dangling_tile_id_errors() {
        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::tilemap(LayerId::new(0), "tm", TilesetId::new(0)))
            .add_frame(Frame::default())
            .add_tileset(two_tile_tileset(0, 2, [10, 20, 30, 255]))
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(tilemap_cel(
            LayerId::new(0),
            1,
            1,
            vec![TileRef::new(42)], // tileset only has tiles 0 and 1
            (0, 0),
        ));
        assert_eq!(
            compose(&sprite, &cels, &full_req(2, 2)).unwrap_err(),
            RenderError::TileIdOutOfRange {
                layer: LayerId::new(0),
                frame: FrameIndex::new(0),
                tile_id: 42,
            }
        );
    }

    #[test]
    fn tilemap_image_cel_on_tilemap_layer_errors() {
        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::tilemap(LayerId::new(0), "tm", TilesetId::new(0)))
            .add_frame(Frame::default())
            .add_tileset(two_tile_tileset(0, 2, [10, 20, 30, 255]))
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(2, 2, [255, 0, 0, 255]),
        ));
        assert_eq!(
            compose(&sprite, &cels, &full_req(2, 2)).unwrap_err(),
            RenderError::CelTypeMismatch {
                layer: LayerId::new(0),
                frame: FrameIndex::new(0),
            }
        );
    }

    #[test]
    fn tilemap_rotate_90_on_non_square_errors() {
        // 2x4 tile size — rotate_90 on a non-square tileset is Phase 2.
        let mut ts = Tileset::new(TilesetId::new(0), "tiles", (2, 4));
        ts.tiles.push(TileImage {
            pixels: PixelBuffer::empty(2, 4, ColorMode::Rgba),
        });
        ts.tiles.push(TileImage {
            pixels: solid(2, 4, [10, 20, 30, 255]),
        });
        let sprite = Sprite::builder(4, 4)
            .add_layer(Layer::tilemap(LayerId::new(0), "tm", TilesetId::new(0)))
            .add_frame(Frame::default())
            .add_tileset(ts)
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(tilemap_cel(
            LayerId::new(0),
            1,
            1,
            vec![TileRef {
                tile_id: 1,
                flip_x: false,
                flip_y: false,
                rotate_90: true,
            }],
            (0, 0),
        ));
        assert_eq!(
            compose(&sprite, &cels, &full_req(4, 4)).unwrap_err(),
            RenderError::NonSquareRotateUnsupported {
                layer: LayerId::new(0),
                tileset: TilesetId::new(0),
                tile_size: (2, 4),
            }
        );
    }

    #[test]
    fn tilemap_empty_tile_id_zero_is_skipped_without_consulting_tileset() {
        // An empty tileset (only tile 0 is implied) + cel referencing only
        // tile id 0 should compose to a fully transparent canvas without
        // raising TileIdOutOfRange.
        let mut ts = Tileset::new(TilesetId::new(0), "tiles", (2, 2));
        ts.tiles.push(TileImage {
            pixels: PixelBuffer::empty(2, 2, ColorMode::Rgba),
        });
        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::tilemap(LayerId::new(0), "tm", TilesetId::new(0)))
            .add_frame(Frame::default())
            .add_tileset(ts)
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(tilemap_cel(
            LayerId::new(0),
            1,
            1,
            vec![TileRef::EMPTY],
            (0, 0),
        ));
        let r = compose(&sprite, &cels, &full_req(2, 2)).unwrap();
        assert!(r.pixels.iter().all(|&v| v == 0));
    }

    #[test]
    fn tilemap_malformed_cel_buffer_errors() {
        // Build a tilemap cel whose `tiles` length is inconsistent with the
        // declared `grid_w * grid_h` (3 entries declared, 2 actually
        // present). compose() must refuse rather than panic-index.
        let sprite = Sprite::builder(4, 4)
            .add_layer(Layer::tilemap(LayerId::new(0), "tm", TilesetId::new(0)))
            .add_frame(Frame::default())
            .add_tileset(two_tile_tileset(0, 2, [10, 20, 30, 255]))
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(0),
            position: (0, 0),
            opacity: 255,
            data: CelData::Tilemap {
                grid_w: 2,
                grid_h: 2,
                tiles: vec![TileRef::EMPTY, TileRef::EMPTY], // missing two
            },
        });
        assert_eq!(
            compose(&sprite, &cels, &full_req(4, 4)).unwrap_err(),
            RenderError::MalformedCelBuffer {
                layer: LayerId::new(0),
                frame: FrameIndex::new(0),
            }
        );
    }

    #[test]
    fn tilemap_cel_position_offsets_tile_placement() {
        let sprite = Sprite::builder(4, 4)
            .add_layer(Layer::tilemap(LayerId::new(0), "tm", TilesetId::new(0)))
            .add_frame(Frame::default())
            .add_tileset(two_tile_tileset(0, 2, [10, 20, 30, 255]))
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(tilemap_cel(
            LayerId::new(0),
            1,
            1,
            vec![TileRef::new(1)],
            (2, 2),
        ));
        let r = compose(&sprite, &cels, &full_req(4, 4)).unwrap();
        // Tile is at sprite (2..4, 2..4); rest is transparent.
        let row_bytes = 4 * 4;
        let cell = [10, 20, 30, 255];
        // top-left 2x2 is blank
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(
                    &r.pixels[y * row_bytes + x * 4..y * row_bytes + x * 4 + 4],
                    &[0, 0, 0, 0]
                );
            }
        }
        // bottom-right 2x2 has the tile
        for y in 2..4 {
            for x in 2..4 {
                assert_eq!(
                    &r.pixels[y * row_bytes + x * 4..y * row_bytes + x * 4 + 4],
                    &cell
                );
            }
        }
    }

    #[test]
    fn rejects_non_normal_blend_mode() {
        let mut layer = Layer::image(LayerId::new(3), "bg");
        layer.blend_mode = BlendMode::Multiply;
        let sprite = Sprite::builder(1, 1)
            .add_layer(layer)
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let cels = CelMap::new();
        assert_eq!(
            compose(&sprite, &cels, &full_req(1, 1)).unwrap_err(),
            RenderError::UnsupportedBlendMode {
                layer: LayerId::new(3),
                mode: BlendMode::Multiply,
            }
        );
    }

    #[test]
    fn layer_filter_all_renders_invisible() {
        let mut layer = Layer::image(LayerId::new(0), "bg");
        layer.visible = false;
        let sprite = Sprite::builder(1, 1)
            .add_layer(layer)
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, [10, 20, 30, 255]),
        ));
        let req = ComposeRequest {
            include_layers: LayerFilter::All,
            ..full_req(1, 1)
        };
        let r = compose(&sprite, &cels, &req).unwrap();
        assert_eq!(r.pixels, vec![10, 20, 30, 255]);
    }

    #[test]
    fn layer_filter_only_renders_subset() {
        let sprite = Sprite::builder(1, 1)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_layer(Layer::image(LayerId::new(1), "fg"))
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, [255, 0, 0, 255]),
        ));
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            solid(1, 1, [0, 255, 0, 255]),
        ));
        let req = ComposeRequest {
            include_layers: LayerFilter::Only(vec![LayerId::new(0)]),
            ..full_req(1, 1)
        };
        let r = compose(&sprite, &cels, &req).unwrap();
        assert_eq!(r.pixels, vec![255, 0, 0, 255]);
    }

    #[test]
    fn zoom_duplicates_pixels_horizontally_and_vertically() {
        let sprite = one_layer_sprite(2, 1, 1);
        let mut buf = PixelBuffer::empty(2, 1, ColorMode::Rgba);
        buf.data[..4].copy_from_slice(&[255, 0, 0, 255]);
        buf.data[4..].copy_from_slice(&[0, 0, 255, 255]);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), buf));

        let req = ComposeRequest {
            zoom: 2,
            ..ComposeRequest::full(FrameIndex::new(0), 2, 1)
        };
        let r = compose(&sprite, &cels, &req).unwrap();
        assert_eq!((r.width, r.height), (4, 2));
        // Row 0: R R B B
        let row0: Vec<u8> = [
            [255, 0, 0, 255],
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            [0, 0, 255, 255],
        ]
        .concat();
        assert_eq!(&r.pixels[0..16], row0.as_slice());
        // Row 1 is a copy of row 0 (vertical replication).
        assert_eq!(&r.pixels[16..32], row0.as_slice());
    }

    #[test]
    fn zoom_3_produces_9x_pixel_count() {
        let sprite = one_layer_sprite(2, 2, 1);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(2, 2, [50, 60, 70, 255]),
        ));
        let req = ComposeRequest {
            zoom: 3,
            ..ComposeRequest::full(FrameIndex::new(0), 2, 2)
        };
        let r = compose(&sprite, &cels, &req).unwrap();
        assert_eq!((r.width, r.height), (6, 6));
        assert_eq!(r.pixels.len(), 6 * 6 * 4);
        for px in r.pixels.chunks_exact(4) {
            assert_eq!(px, &[50, 60, 70, 255]);
        }
    }

    #[test]
    fn zoom_at_max_factor_succeeds() {
        let sprite = one_layer_sprite(1, 1, 1);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, [1, 2, 3, 255]),
        ));
        let req = ComposeRequest {
            zoom: 64,
            ..ComposeRequest::full(FrameIndex::new(0), 1, 1)
        };
        let r = compose(&sprite, &cels, &req).unwrap();
        assert_eq!((r.width, r.height), (64, 64));
        assert_eq!(r.pixels.len(), 64 * 64 * 4);
    }

    #[test]
    fn rejects_linked_cel() {
        let sprite = one_layer_sprite(1, 1, 1);
        let mut cels = CelMap::new();
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(0),
            position: (0, 0),
            opacity: 255,
            data: CelData::Linked(FrameIndex::new(0)),
        });
        assert_eq!(
            compose(&sprite, &cels, &full_req(1, 1)).unwrap_err(),
            RenderError::LinkedCelUnsupported {
                layer: LayerId::new(0),
                frame: FrameIndex::new(0),
            }
        );
    }

    #[test]
    fn rejects_cel_with_wrong_color_mode() {
        let sprite = one_layer_sprite(1, 1, 1);
        let mut cels = CelMap::new();
        // 1×1 indexed buffer (1 byte) on an RGBA sprite.
        let bogus = PixelBuffer::empty(
            1,
            1,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        );
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), bogus));
        assert_eq!(
            compose(&sprite, &cels, &full_req(1, 1)).unwrap_err(),
            RenderError::CelColorModeMismatch {
                layer: LayerId::new(0),
                frame: FrameIndex::new(0),
                mode: ColorMode::Indexed {
                    transparent_index: 0,
                },
            }
        );
    }

    #[test]
    fn rejects_malformed_cel_buffer() {
        let sprite = one_layer_sprite(1, 1, 1);
        let mut cels = CelMap::new();
        // Claim 2×2 RGBA (16 bytes) but only ship 4. is_well_formed returns false.
        let mangled = PixelBuffer {
            width: 2,
            height: 2,
            color_mode: ColorMode::Rgba,
            data: vec![0, 0, 0, 255],
        };
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), mangled));
        assert_eq!(
            compose(&sprite, &cels, &full_req(1, 1)).unwrap_err(),
            RenderError::MalformedCelBuffer {
                layer: LayerId::new(0),
                frame: FrameIndex::new(0),
            }
        );
    }

    #[test]
    fn rejects_tilemap_data_on_image_layer() {
        let sprite = one_layer_sprite(1, 1, 1);
        let mut cels = CelMap::new();
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(0),
            position: (0, 0),
            opacity: 255,
            data: CelData::Tilemap {
                grid_w: 1,
                grid_h: 1,
                tiles: vec![crate::document::TileRef::EMPTY],
            },
        });
        assert_eq!(
            compose(&sprite, &cels, &full_req(1, 1)).unwrap_err(),
            RenderError::CelTypeMismatch {
                layer: LayerId::new(0),
                frame: FrameIndex::new(0),
            }
        );
    }

    #[test]
    fn rejects_onion_skin_request() {
        let sprite = one_layer_sprite(1, 1, 1);
        let cels = CelMap::new();
        let req = ComposeRequest {
            onion_skin: Some(super::super::OnionSkin::default()),
            ..full_req(1, 1)
        };
        assert_eq!(
            compose(&sprite, &cels, &req).unwrap_err(),
            RenderError::OnionSkinUnsupported
        );
    }

    #[test]
    fn rejects_overlays_request() {
        let sprite = one_layer_sprite(1, 1, 1);
        let cels = CelMap::new();
        let req = ComposeRequest {
            overlays: Overlays {
                grid: true,
                ..Overlays::default()
            },
            ..full_req(1, 1)
        };
        assert_eq!(
            compose(&sprite, &cels, &req).unwrap_err(),
            RenderError::OverlaysUnsupported
        );
    }
}
