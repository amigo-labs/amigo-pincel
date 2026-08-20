//! `compose()` — the single composition entry point. See `docs/specs/pincel.md` §4.
//!
//! Composites visible image and tilemap layers in z-order, RGBA color mode
//! only. Per-layer blending lives in [`super::blend`], the two cel paths in
//! [`super::image_layer`] and [`super::tilemap_layer`]. Group layers hold
//! no pixels and are skipped (their visibility gates their children — see
//! [`effectively_visible`]). Indexed color, linked cels, onion skin, and
//! overlays still return [`RenderError`].

use crate::document::{CelData, CelMap, ColorMode, Layer, LayerKind, Sprite};
use crate::geometry::Rect;

use super::blend::mul_u8;
use super::error::RenderError;
use super::image_layer::composite_image_cel;
use super::request::{ComposeRequest, ComposeResult, LayerFilter, Overlays};
use super::tilemap_layer::composite_tilemap_cel;

/// Maximum supported zoom factor (per spec §4.1).
pub(super) const MAX_ZOOM: u32 = 64;

/// Compose a frame of `sprite` into the caller-owned RGBA8 pixel buffer
/// `out`. See spec §4 for the contract. `out` is resized and overwritten
/// to hold exactly `result.width * result.height * 4` non-premultiplied
/// RGBA8 bytes in row-major order.
///
/// Reusing the same `out` across calls keeps the `zoom == 1` hot path
/// allocation-free (spec §4.1). For `zoom > 1` a single
/// `pre_zoom_len`-sized intermediate `Vec` is still allocated per call
/// — the nearest-neighbor upscale writes into `out` from that
/// intermediate. Threading a second caller-owned scratch through the
/// upscale path is a follow-up (a `Canvas2D` adapter rendering at the
/// composed pixel-grid keeps `zoom == 1`, so the present API covers
/// the production hot path).
pub fn compose(
    sprite: &Sprite,
    cels: &CelMap,
    request: &ComposeRequest,
    out: &mut Vec<u8>,
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

    let viewport = request.viewport;
    // The effective render region is the intersection of the viewport
    // with `dirty_hint`. When `dirty_hint` is `None` we fall back to the
    // full viewport, preserving the historical full-frame contract.
    let dirty_rect = match request.dirty_hint {
        Some(hint) => viewport.intersect(hint),
        None => viewport,
    };

    if dirty_rect.is_empty() {
        // Caller asked for a dirty region that doesn't overlap the
        // viewport — nothing to render. Clear `out` and report the empty
        // rect; the caller can use `dirty_rect.is_empty()` to skip the
        // upload step entirely.
        out.clear();
        return Ok(ComposeResult {
            width: 0,
            height: 0,
            dirty_rect,
            generation: 0,
        });
    }

    let pre_zoom_len = (dirty_rect.width as usize) * (dirty_rect.height as usize) * 4;
    let out_len = pre_zoom_len * (request.zoom as usize) * (request.zoom as usize);

    // For zoom == 1 we composite directly into `out`. For zoom > 1 we
    // composite into a small intermediate `Vec`, then nearest-neighbor
    // upscale into `out`. Either way `out` ends up sized to `out_len`.
    if request.zoom == 1 {
        out.clear();
        out.resize(out_len, 0);
        composite_visible_layers(out.as_mut_slice(), sprite, cels, request, dirty_rect)?;
    } else {
        let mut intermediate = vec![0u8; pre_zoom_len];
        composite_visible_layers(
            intermediate.as_mut_slice(),
            sprite,
            cels,
            request,
            dirty_rect,
        )?;
        out.clear();
        out.resize(out_len, 0);
        upscale_nearest_into(
            out.as_mut_slice(),
            &intermediate,
            dirty_rect.width,
            dirty_rect.height,
            request.zoom,
        );
    }

    Ok(ComposeResult {
        width: dirty_rect.width * request.zoom,
        height: dirty_rect.height * request.zoom,
        dirty_rect,
        generation: 0,
    })
}

/// Composite every selected layer at `request.frame` into `dst`, which is
/// sized to `viewport.width * viewport.height * 4` bytes. Each layer blends
/// under its own blend mode (see
/// [`blend_pixel_into`](super::blend::blend_pixel_into)); linked cels and
/// mismatched cel kinds raise [`RenderError`] — see the per-arm comments
/// below.
fn composite_visible_layers(
    dst: &mut [u8],
    sprite: &Sprite,
    cels: &CelMap,
    request: &ComposeRequest,
    viewport: Rect,
) -> Result<(), RenderError> {
    for layer in sprite.layers.iter() {
        if !layer_included(layer, sprite, &request.include_layers) {
            continue;
        }
        if let LayerKind::Group = layer.kind {
            // A group holds no pixels of its own — its children are separate
            // entries in the flat z-ordered Vec and composite on their own.
            // Group visibility gates children via `effectively_visible`;
            // group opacity / blend mode are NOT folded into children in
            // Phase 1 (spec §4 defers the fold-into-temp-buffer behavior).
            continue;
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
                    dst,
                    viewport,
                    cel.position,
                    pixels,
                    mul_u8(layer.opacity, cel.opacity),
                    layer.blend_mode,
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
                    dst,
                    viewport,
                    cel.position,
                    *grid_w,
                    *grid_h,
                    tiles,
                    tileset,
                    layer.id,
                    request.frame,
                    sprite.color_mode,
                    mul_u8(layer.opacity, cel.opacity),
                    layer.blend_mode,
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
                // Group layers are skipped above before the cel lookup.
                unreachable!("group layers handled before cel lookup");
            }
        }
    }
    Ok(())
}

/// Nearest-neighbor integer upscale of an RGBA8 image. See spec §4.1: the
/// composer produces the exact pixel grid the UI displays so the GPU just
/// blits and we avoid subpixel sampling artifacts. `out` is sized to
/// `w * zoom * h * zoom * 4` bytes by the caller.
fn upscale_nearest_into(out: &mut [u8], src: &[u8], w: u32, h: u32, zoom: u32) {
    let zoom_us = zoom as usize;
    let w_us = w as usize;
    let h_us = h as usize;
    let zw = w_us * zoom_us;

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
}

fn layer_included(layer: &Layer, sprite: &Sprite, filter: &LayerFilter) -> bool {
    match filter {
        LayerFilter::Visible => effectively_visible(sprite, layer),
        LayerFilter::All => true,
        LayerFilter::Only(ids) => ids.contains(&layer.id),
    }
}

/// A layer renders under [`LayerFilter::Visible`] only when it *and every
/// ancestor group* are visible — hiding a group hides its whole subtree,
/// matching Aseprite. Only visibility propagates; group opacity and blend
/// mode are not folded into children in Phase 1. `LayerFilter::All` and
/// `LayerFilter::Only` bypass this walk (All means "everything", Only is an
/// explicit override).
///
/// The walk is capped at `sprite.layers.len()` hops so a corrupt parent
/// cycle terminates; a dangling parent id ends the chain (the layer still
/// renders — structural inconsistency shouldn't silently hide content).
fn effectively_visible(sprite: &Sprite, layer: &Layer) -> bool {
    if !layer.visible {
        return false;
    }
    let mut parent = layer.parent;
    let mut hops = 0usize;
    while let Some(pid) = parent {
        let Some(p) = sprite.layer(pid) else {
            return true;
        };
        if !p.visible {
            return false;
        }
        hops += 1;
        if hops >= sprite.layers.len() {
            return true;
        }
        parent = p.parent;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{
        Cel, CelData, Frame, FrameIndex, Layer, LayerId, PixelBuffer, TilesetId,
    };

    use super::super::test_support::{
        compose_owned, full_req, one_layer_sprite, solid, two_tile_tileset,
    };

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
        let r = compose_owned(&sprite, &cels, &full_req(1, 1)).unwrap();
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
        let r = compose_owned(&sprite, &cels, &full_req(1, 1)).unwrap();
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
        let r = compose_owned(&sprite, &cels, &full_req(1, 1)).unwrap();
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
            compose_owned(&sprite, &cels, &full_req(1, 1)).unwrap_err(),
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
            compose_owned(&sprite, &cels, &req).unwrap_err(),
            RenderError::InvalidZoom { zoom: 0 }
        ));
        req.zoom = 65;
        assert!(matches!(
            compose_owned(&sprite, &cels, &req).unwrap_err(),
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
            compose_owned(&sprite, &cels, &req).unwrap_err(),
            RenderError::EmptyViewport
        );
    }

    #[test]
    fn rejects_unknown_frame() {
        let sprite = one_layer_sprite(1, 1, 1);
        let cels = CelMap::new();
        let req = ComposeRequest::full(FrameIndex::new(2), 1, 1);
        assert_eq!(
            compose_owned(&sprite, &cels, &req).unwrap_err(),
            RenderError::UnknownFrame {
                frame: FrameIndex::new(2)
            }
        );
    }

    /// Builds a 1×1 sprite with a group (id 0), a child image layer (id 1)
    /// parented to it, and a red cel on the child.
    fn group_child_sprite(group_visible: bool, child_visible: bool) -> (Sprite, CelMap) {
        let mut group = Layer::group(LayerId::new(0), "grp");
        group.visible = group_visible;
        let mut child = Layer::image(LayerId::new(1), "fg");
        child.visible = child_visible;
        child.parent = Some(LayerId::new(0));
        let sprite = Sprite::builder(1, 1)
            .add_layer(group)
            .add_layer(child)
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            solid(1, 1, [255, 0, 0, 255]),
        ));
        (sprite, cels)
    }

    #[test]
    fn group_layer_is_skipped_and_children_render() {
        let (sprite, cels) = group_child_sprite(true, true);
        let r = compose_owned(&sprite, &cels, &full_req(1, 1)).unwrap();
        assert_eq!(r.pixels, vec![255, 0, 0, 255]);
    }

    #[test]
    fn hidden_group_hides_child_layers() {
        let (sprite, cels) = group_child_sprite(false, true);
        let r = compose_owned(&sprite, &cels, &full_req(1, 1)).unwrap();
        assert_eq!(r.pixels, vec![0, 0, 0, 0]);
    }

    #[test]
    fn hidden_grandparent_hides_nested_child() {
        let mut outer = Layer::group(LayerId::new(0), "outer");
        outer.visible = false;
        let mut inner = Layer::group(LayerId::new(1), "inner");
        inner.parent = Some(LayerId::new(0));
        let mut child = Layer::image(LayerId::new(2), "fg");
        child.parent = Some(LayerId::new(1));
        let sprite = Sprite::builder(1, 1)
            .add_layer(outer)
            .add_layer(inner)
            .add_layer(child)
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(2),
            FrameIndex::new(0),
            solid(1, 1, [255, 0, 0, 255]),
        ));
        let r = compose_owned(&sprite, &cels, &full_req(1, 1)).unwrap();
        assert_eq!(r.pixels, vec![0, 0, 0, 0]);
    }

    #[test]
    fn layer_filter_all_ignores_hidden_group() {
        let (sprite, cels) = group_child_sprite(false, true);
        let mut req = full_req(1, 1);
        req.include_layers = LayerFilter::All;
        let r = compose_owned(&sprite, &cels, &req).unwrap();
        assert_eq!(r.pixels, vec![255, 0, 0, 255]);
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
            compose_owned(&sprite, &cels, &full_req(2, 2)).unwrap_err(),
            RenderError::CelTypeMismatch {
                layer: LayerId::new(0),
                frame: FrameIndex::new(0),
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
        let r = compose_owned(&sprite, &cels, &req).unwrap();
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
        let r = compose_owned(&sprite, &cels, &req).unwrap();
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
        let r = compose_owned(&sprite, &cels, &req).unwrap();
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
        let r = compose_owned(&sprite, &cels, &req).unwrap();
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
        let r = compose_owned(&sprite, &cels, &req).unwrap();
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
            compose_owned(&sprite, &cels, &full_req(1, 1)).unwrap_err(),
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
            compose_owned(&sprite, &cels, &full_req(1, 1)).unwrap_err(),
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
            compose_owned(&sprite, &cels, &full_req(1, 1)).unwrap_err(),
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
            compose_owned(&sprite, &cels, &full_req(1, 1)).unwrap_err(),
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
            compose_owned(&sprite, &cels, &req).unwrap_err(),
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
            compose_owned(&sprite, &cels, &req).unwrap_err(),
            RenderError::OverlaysUnsupported
        );
    }

    /// 4×4 sprite filled with a recognizable per-pixel pattern so the
    /// dirty-hint tests can assert on exact sub-rect contents.
    fn rainbow_sprite_4x4() -> (Sprite, CelMap) {
        let sprite = one_layer_sprite(4, 4, 1);
        let mut buf = PixelBuffer::empty(4, 4, ColorMode::Rgba);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let idx = ((y * 4 + x) * 4) as usize;
                // Encode (x, y) into RGB so the sub-rect read is unambiguous.
                buf.data[idx..idx + 4].copy_from_slice(&[
                    (x * 60) as u8,
                    (y * 60) as u8,
                    ((x + y) * 30) as u8,
                    255,
                ]);
            }
        }
        let mut cels = CelMap::new();
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), buf));
        (sprite, cels)
    }

    #[test]
    fn no_dirty_hint_reports_full_viewport_as_dirty_rect() {
        let (sprite, cels) = rainbow_sprite_4x4();
        let r = compose_owned(&sprite, &cels, &full_req(4, 4)).unwrap();
        assert_eq!(r.pixels.len(), 4 * 4 * 4);
        // Without a hint, dirty_rect spans the whole viewport. We don't expose
        // dirty_rect via OwnedFrame, so check it via the direct API below.
        let mut out = Vec::new();
        let req = full_req(4, 4);
        let info = compose(&sprite, &cels, &req, &mut out).unwrap();
        assert_eq!(info.dirty_rect, Rect::new(0, 0, 4, 4));
        assert_eq!((info.width, info.height), (4, 4));
    }

    #[test]
    fn dirty_hint_inside_viewport_renders_only_intersection() {
        let (sprite, cels) = rainbow_sprite_4x4();
        // 2×2 sub-rect at (1, 1).
        let req = ComposeRequest {
            dirty_hint: Some(Rect::new(1, 1, 2, 2)),
            ..full_req(4, 4)
        };
        let mut out = Vec::new();
        let info = compose(&sprite, &cels, &req, &mut out).unwrap();

        assert_eq!(info.dirty_rect, Rect::new(1, 1, 2, 2));
        assert_eq!((info.width, info.height), (2, 2));
        assert_eq!(out.len(), 2 * 2 * 4);
        // Expected: (1,1), (2,1), (1,2), (2,2) in row-major order.
        let expected = [
            [60, 60, 60, 255], // x=1, y=1
            [120, 60, 90, 255],
            [60, 120, 90, 255],
            [120, 120, 120, 255], // x=2, y=2
        ]
        .concat();
        assert_eq!(out, expected);
    }

    #[test]
    fn dirty_hint_partially_outside_viewport_clamps_to_overlap() {
        let (sprite, cels) = rainbow_sprite_4x4();
        // Hint extends past the canvas — overlap is the bottom-right 2×2.
        let req = ComposeRequest {
            dirty_hint: Some(Rect::new(2, 2, 10, 10)),
            ..full_req(4, 4)
        };
        let mut out = Vec::new();
        let info = compose(&sprite, &cels, &req, &mut out).unwrap();

        assert_eq!(info.dirty_rect, Rect::new(2, 2, 2, 2));
        assert_eq!(out.len(), 2 * 2 * 4);
    }

    #[test]
    fn dirty_hint_disjoint_from_viewport_returns_empty_buffer() {
        let (sprite, cels) = rainbow_sprite_4x4();
        let req = ComposeRequest {
            dirty_hint: Some(Rect::new(100, 100, 4, 4)),
            ..full_req(4, 4)
        };
        let mut out = vec![0xAB; 16]; // pre-fill so we can see it's cleared
        let info = compose(&sprite, &cels, &req, &mut out).unwrap();

        assert!(info.dirty_rect.is_empty());
        assert_eq!((info.width, info.height), (0, 0));
        assert!(out.is_empty());
    }

    #[test]
    fn dirty_hint_equal_to_viewport_matches_full_render() {
        let (sprite, cels) = rainbow_sprite_4x4();
        let mut hinted_out = Vec::new();
        let mut full_out = Vec::new();
        let hinted = ComposeRequest {
            dirty_hint: Some(Rect::new(0, 0, 4, 4)),
            ..full_req(4, 4)
        };
        let full = full_req(4, 4);
        let hinted_info = compose(&sprite, &cels, &hinted, &mut hinted_out).unwrap();
        let full_info = compose(&sprite, &cels, &full, &mut full_out).unwrap();

        assert_eq!(hinted_info.dirty_rect, full_info.dirty_rect);
        assert_eq!(hinted_out, full_out);
    }

    #[test]
    fn dirty_hint_with_zoom_upscales_only_intersection() {
        let (sprite, cels) = rainbow_sprite_4x4();
        let req = ComposeRequest {
            dirty_hint: Some(Rect::new(1, 1, 2, 2)),
            zoom: 3,
            ..full_req(4, 4)
        };
        let mut out = Vec::new();
        let info = compose(&sprite, &cels, &req, &mut out).unwrap();

        assert_eq!(info.dirty_rect, Rect::new(1, 1, 2, 2));
        assert_eq!((info.width, info.height), (6, 6));
        assert_eq!(out.len(), 6 * 6 * 4);
    }
}
