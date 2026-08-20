//! `compose()` — the single composition entry point. See `docs/specs/pincel.md` §4.
//!
//! M3 implements the minimum useful path: visible image layers in z-order
//! with the `Normal` blend mode, RGBA color mode only. Tilemap and group
//! layers, indexed color, non-Normal blend modes, linked cels, onion skin,
//! and overlays all return [`RenderError`] for now. The `dirty_hint` field
//! on the request is accepted and currently ignored.

use crate::document::{CelData, CelMap, ColorMode, Layer, LayerKind, Sprite};

use super::blend::{BlendParams, is_implemented};
use super::error::RenderError;
use super::image_layer::composite_image_cel;
use super::request::{ComposeRequest, ComposeResult, LayerFilter, Overlays};
use super::tilemap_layer::composite_tilemap_cel;

/// Maximum supported zoom factor (per spec §4.1).
pub(super) const MAX_ZOOM: u32 = 64;

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
        if !is_implemented(layer.blend_mode) {
            return Err(RenderError::UnsupportedBlendMode {
                layer: layer.id,
                mode: layer.blend_mode,
            });
        }
        let Some(cel) = cels.get(layer.id, request.frame) else {
            continue;
        };
        let blend = BlendParams::new(layer.blend_mode, layer.opacity, cel.opacity);
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
                composite_image_cel(&mut buffer, vp, cel.position, pixels, blend);
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
                    blend,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{
        BlendMode, Cel, CelData, Frame, FrameIndex, Layer, LayerId, PixelBuffer, Sprite,
    };
    use crate::geometry::Rect;
    use crate::render::test_support::{full_req, one_layer_sprite, solid};

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

    #[test]
    fn rejects_not_yet_implemented_blend_mode() {
        // Names a mode `blend::is_implemented` still rejects, so this keeps
        // covering the error path while modes land group by group. Retarget it
        // when `Luminosity` lands; delete it only once every mode is in.
        let mut layer = Layer::image(LayerId::new(3), "bg");
        layer.blend_mode = BlendMode::Luminosity;
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
                mode: BlendMode::Luminosity,
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
