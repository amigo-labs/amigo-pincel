//! `compose()` — the single composition entry point. See `docs/specs/pincel.md` §4.
//!
//! M3 implements the minimum useful path: visible image layers in z-order
//! with the `Normal` blend mode, RGBA color mode only. Tilemap and group
//! layers, indexed color, non-Normal blend modes, overlays, and onion skin
//! all return [`RenderError`] for now.

use thiserror::Error;

use crate::document::{
    BlendMode, CelData, CelMap, ColorMode, FrameIndex, Layer, LayerId, LayerKind, PixelBuffer,
    Sprite,
};
use crate::geometry::Rect;

use super::request::{ComposeRequest, ComposeResult, LayerFilter};

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
        match layer.kind {
            LayerKind::Image => {}
            LayerKind::Tilemap { .. } | LayerKind::Group => {
                return Err(RenderError::UnsupportedLayerKind { layer: layer.id });
            }
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
        let pixels = match &cel.data {
            CelData::Image(buffer) => buffer,
            // Tilemap layers were rejected above, so this is a Linked cel.
            // M3 does not chase linkage; treat it as transparent.
            CelData::Tilemap { .. } | CelData::Linked(_) => continue,
        };
        composite_image_cel(
            &mut buffer,
            vp,
            cel.position,
            pixels,
            layer.opacity,
            cel.opacity,
        );
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
    use crate::document::{Cel, Frame, FrameIndex, Layer, LayerId, PixelBuffer, Sprite, TilesetId};

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
    fn rejects_tilemap_layer() {
        let sprite = Sprite::builder(1, 1)
            .add_layer(Layer::tilemap(LayerId::new(7), "tiles", TilesetId::new(0)))
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
}
