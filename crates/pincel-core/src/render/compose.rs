//! `compose()` — the single composition entry point. See `docs/specs/pincel.md` §4.
//!
//! Composites visible image and tilemap layers in z-order, in every color
//! mode: indexed and grayscale pixels turn into RGBA as they are read (see
//! [`super::source`]). Per-layer blending lives in [`super::blend`], the two
//! cel paths in [`super::image_layer`] and [`super::tilemap_layer`]. Group
//! layers hold no pixels: their visibility gates their children (see
//! [`effectively_visible`]) and their opacity and blend mode fold the
//! children's composite (see [`composite_visible_layers`]). Onion skin
//! (spec §4.2) composites the
//! neighbouring frames under the current one — see [`super::onion`].
//! Linked cels render their target frame's cel ([`resolve_cel`]); overlays
//! still return [`RenderError`].

use crate::document::{BlendMode, Cel, CelData, CelMap, FrameIndex, Layer, LayerKind, Sprite};
use crate::geometry::Rect;

use super::blend::{blend_pixel_into, mul_u8};
use super::error::RenderError;
use super::image_layer::composite_image_cel;
use super::request::{ComposeRequest, ComposeResult, LayerFilter, Overlays};
use super::source::{Paint, SpriteDecoders};
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
    if request.zoom == 0 || request.zoom > MAX_ZOOM {
        return Err(RenderError::InvalidZoom { zoom: request.zoom });
    }
    if request.viewport.is_empty() {
        return Err(RenderError::EmptyViewport);
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
        composite_frame(out.as_mut_slice(), sprite, cels, request, dirty_rect)?;
    } else {
        let mut intermediate = vec![0u8; pre_zoom_len];
        composite_frame(
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

/// Composite the requested frame into `dst` — the onion-skin neighbours
/// first (when requested), then the frame itself on top.
fn composite_frame(
    dst: &mut [u8],
    sprite: &Sprite,
    cels: &CelMap,
    request: &ComposeRequest,
    viewport: Rect,
) -> Result<(), RenderError> {
    let decoders = SpriteDecoders::new(sprite.color_mode, &sprite.palette);
    let scene = Scene {
        sprite,
        cels,
        decoders: &decoders,
    };
    if let Some(onion) = request.onion_skin {
        super::onion::composite_onion_skin(dst, &scene, request, onion, viewport)?;
    }
    composite_visible_layers(
        dst,
        &scene,
        &request.include_layers,
        request.frame,
        viewport,
    )
}

/// What one compose reads: the document plus the pixel decoders built
/// for it once per call.
pub(super) struct Scene<'a> {
    pub(super) sprite: &'a Sprite,
    pub(super) cels: &'a CelMap,
    pub(super) decoders: &'a SpriteDecoders,
}

/// Composite every selected layer at `frame` into `dst`, which is
/// sized to `viewport.width * viewport.height * 4` bytes. Each layer blends
/// under its own blend mode (see
/// [`blend_pixel_into`](super::blend::blend_pixel_into)); broken links and
/// mismatched cel kinds raise [`RenderError`] — see [`composite_layer_cel`].
///
/// Group layers fold (spec §4.2, Decision Log 2026-10-29): a group at
/// opacity 255 with blend mode Normal passes its children through onto the
/// layers below it; any other group composites its children into a cleared
/// viewport-sized scratch buffer and blends that onto the layers below with
/// its own opacity and blend mode. Which groups fold follows the filter —
/// see [`LayerTree`].
pub(super) fn composite_visible_layers(
    dst: &mut [u8],
    scene: &Scene<'_>,
    filter: &LayerFilter,
    frame: FrameIndex,
    viewport: Rect,
) -> Result<(), RenderError> {
    let tree = LayerTree::new(scene.sprite, filter);
    let mut pass = Pass {
        scene,
        tree: &tree,
        frame,
        viewport,
        scratch: Vec::new(),
    };
    pass.children(dst, None, 0)
}

/// The drawn layers of one compose as a tree: a layer the filter draws
/// hangs under its nearest *drawn* ancestor group, or at the top when it
/// has none. Under [`LayerFilter::Visible`] and [`LayerFilter::All`] that is
/// the layer's own parent group; under [`LayerFilter::Only`] a group folds
/// only when it is selected itself, so a selected child of an unselected
/// group renders as if it had no parent group.
///
/// A parent id that names no layer ends the walk (the layer is top-level),
/// and the walk is capped at the layer count so a parent cycle terminates;
/// groups that parent each other are never reached from the top.
struct LayerTree<'a> {
    sprite: &'a Sprite,
    filter: &'a LayerFilter,
    /// Per layer index: whether the filter draws it. Empty when the
    /// sprite has no group layer — then nothing is precomputed (no
    /// allocation) and every layer is top-level.
    included: Vec<bool>,
    /// Per layer index: its nearest drawn ancestor group. Empty as above.
    parent: Vec<Option<usize>>,
}

impl<'a> LayerTree<'a> {
    fn new(sprite: &'a Sprite, filter: &'a LayerFilter) -> Self {
        let layers = &sprite.layers;
        if !layers.iter().any(|l| matches!(l.kind, LayerKind::Group)) {
            return Self {
                sprite,
                filter,
                included: Vec::new(),
                parent: Vec::new(),
            };
        }
        let included: Vec<bool> = layers
            .iter()
            .map(|l| layer_included(l, sprite, filter))
            .collect();
        let parent = (0..layers.len())
            .map(|i| folding_parent(layers, &included, i))
            .collect();
        Self {
            sprite,
            filter,
            included,
            parent,
        }
    }

    fn included(&self, index: usize) -> bool {
        match self.included.get(index) {
            Some(&included) => included,
            None => layer_included(&self.sprite.layers[index], self.sprite, self.filter),
        }
    }

    fn parent(&self, index: usize) -> Option<usize> {
        self.parent.get(index).copied().flatten()
    }
}

/// The index of the nearest ancestor of `layers[index]` that is a drawn
/// group, or `None` for a top-level layer — see [`LayerTree`].
fn folding_parent(layers: &[Layer], included: &[bool], index: usize) -> Option<usize> {
    let mut next = layers[index].parent;
    for _ in 0..layers.len() {
        let id = next?;
        let at = layers.iter().position(|l| l.id == id)?;
        if included[at] && matches!(layers[at].kind, LayerKind::Group) {
            return Some(at);
        }
        next = layers[at].parent;
    }
    None
}

/// One walk of a [`LayerTree`] for one frame, with the scratch buffers of
/// isolating groups (one per nesting level, reused across siblings).
struct Pass<'a> {
    scene: &'a Scene<'a>,
    tree: &'a LayerTree<'a>,
    frame: FrameIndex,
    viewport: Rect,
    scratch: Vec<Vec<u8>>,
}

impl Pass<'_> {
    /// Composite the drawn children of `parent` (the top level for `None`)
    /// into `dst`, bottom-first in sprite order.
    fn children(
        &mut self,
        dst: &mut [u8],
        parent: Option<usize>,
        depth: usize,
    ) -> Result<(), RenderError> {
        let (scene, tree) = (self.scene, self.tree);
        let layers = &scene.sprite.layers;
        for (index, layer) in layers.iter().enumerate() {
            if tree.parent(index) != parent || !tree.included(index) {
                continue;
            }
            if !matches!(layer.kind, LayerKind::Group) {
                composite_layer_cel(dst, scene, layer, self.frame, self.viewport)?;
                continue;
            }
            // The layers reachable from the top form a tree, so `depth`
            // stays below the layer count; the guard only stops a corrupt
            // hierarchy from recursing without end.
            if depth >= layers.len() {
                continue;
            }
            if layer.opacity == 255 && layer.blend_mode == BlendMode::Normal {
                self.children(dst, Some(index), depth + 1)?;
            } else if layer.opacity > 0 {
                self.isolated_group(dst, index, depth)?;
            }
        }
        Ok(())
    }

    /// Composite the group at `index` in isolation: its children into the
    /// cleared scratch buffer of `depth`, then that buffer onto `dst` under
    /// the group's opacity and blend mode.
    fn isolated_group(
        &mut self,
        dst: &mut [u8],
        index: usize,
        depth: usize,
    ) -> Result<(), RenderError> {
        if self.scratch.len() <= depth {
            self.scratch.resize_with(depth + 1, Vec::new);
        }
        let mut buf = std::mem::take(&mut self.scratch[depth]);
        buf.clear();
        buf.resize(dst.len(), 0);
        let result = self.children(&mut buf, Some(index), depth + 1);
        if result.is_ok() {
            let group = &self.scene.sprite.layers[index];
            for (d, s) in dst
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(buf.as_chunks::<4>().0)
            {
                if s[3] == 0 {
                    continue;
                }
                blend_pixel_into(
                    group.blend_mode,
                    d,
                    s[0],
                    s[1],
                    s[2],
                    mul_u8(s[3], group.opacity),
                );
            }
        }
        self.scratch[depth] = buf;
        result
    }
}

/// Composite `layer`'s cel at `frame` (following a link) into `dst`.
fn composite_layer_cel(
    dst: &mut [u8],
    scene: &Scene<'_>,
    layer: &Layer,
    frame: FrameIndex,
    viewport: Rect,
) -> Result<(), RenderError> {
    let Scene { sprite, cels, .. } = *scene;
    let Some(cel) = resolve_cel(sprite, cels, layer, frame)? else {
        return Ok(());
    };
    let paint = Paint {
        decode: scene.decoders.for_layer(layer),
        opacity: mul_u8(layer.opacity, cel.opacity),
        blend_mode: layer.blend_mode,
    };
    match (&layer.kind, &cel.data) {
        (LayerKind::Image, CelData::Image(pixels)) => {
            if pixels.color_mode != sprite.color_mode {
                return Err(RenderError::CelColorModeMismatch {
                    layer: layer.id,
                    frame,
                    mode: pixels.color_mode,
                });
            }
            if !pixels.is_well_formed() {
                return Err(RenderError::MalformedCelBuffer {
                    layer: layer.id,
                    frame,
                });
            }
            composite_image_cel(dst, viewport, cel.position, pixels, paint);
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
                frame,
                paint,
            )?;
        }
        (_, CelData::Linked(_)) => {
            unreachable!("resolve_cel follows links one hop and refuses chains");
        }
        (LayerKind::Image, CelData::Tilemap { .. })
        | (LayerKind::Tilemap { .. }, CelData::Image(_)) => {
            return Err(RenderError::CelTypeMismatch {
                layer: layer.id,
                frame,
            });
        }
        (LayerKind::Group, _) => {
            // A cel stored on a group layer (a corrupt document) is ignored;
            // the group tree never asks for one.
        }
    }
    Ok(())
}

/// The cel `layer` shows at `frame`: its own, or for a linked cel the
/// target frame's cel on the same layer, whose data, position and cel
/// opacity a link shares (spec §3.2, Decision Log 2026-10-29). A missing
/// or itself-linked target is an error, so the result is never a link.
fn resolve_cel<'a>(
    sprite: &Sprite,
    cels: &'a CelMap,
    layer: &Layer,
    frame: FrameIndex,
) -> Result<Option<&'a Cel>, RenderError> {
    let Some(cel) = cels.get(layer.id, frame) else {
        return Ok(None);
    };
    let CelData::Linked(target) = cel.data else {
        return Ok(Some(cel));
    };
    let missing = RenderError::LinkedCelTargetMissing {
        layer: layer.id,
        frame,
        target,
    };
    if (target.0 as usize) >= sprite.frames.len() {
        return Err(missing);
    }
    let resolved = cels.get(layer.id, target).ok_or(missing)?;
    if let CelData::Linked(_) = resolved.data {
        return Err(RenderError::LinkedCelChained {
            layer: layer.id,
            frame,
            target,
        });
    }
    Ok(Some(resolved))
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
/// matching Aseprite. Group opacity and blend mode fold separately (see
/// [`composite_visible_layers`]). `LayerFilter::All` and
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
        Cel, CelData, ColorMode, Frame, FrameIndex, Layer, LayerId, Palette, PaletteEntry,
        PixelBuffer, Rgba, TilesetId,
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
    fn compose_indexed_color_mode_renders_palette_colour() {
        let mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        let sprite = Sprite::builder(2, 1)
            .color_mode(mode)
            .palette(Palette::from_entries(vec![
                PaletteEntry::new(Rgba::BLACK),
                PaletteEntry::new(Rgba::new(1, 2, 3, 255)),
            ]))
            .add_layer(Layer::image(LayerId::new(0), "art"))
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let mut buf = PixelBuffer::empty(2, 1, mode);
        buf.data = vec![0, 1];
        let mut cels = CelMap::new();
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), buf));
        let r = compose_owned(&sprite, &cels, &full_req(2, 1)).unwrap();
        assert_eq!(r.pixels, vec![0, 0, 0, 0, 1, 2, 3, 255]);
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
    fn layer_filter_only_draws_in_sprite_z_order_not_list_order() {
        // Layer 1 (green) sits above layer 0 (red); the list names it first.
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
            include_layers: LayerFilter::Only(vec![LayerId::new(1), LayerId::new(0)]),
            ..full_req(1, 1)
        };
        let r = compose_owned(&sprite, &cels, &req).unwrap();
        assert_eq!(r.pixels, vec![0, 255, 0, 255]);
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
        for px in r.pixels.as_chunks::<4>().0 {
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
    fn compose_linked_cel_renders_target_frame_cel() {
        let sprite = one_layer_sprite(1, 1, 2);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, [9, 8, 7, 255]),
        ));
        cels.insert(Cel {
            data: CelData::Linked(FrameIndex::new(0)),
            ..Cel::image(LayerId::new(0), FrameIndex::new(1), solid(1, 1, [0; 4]))
        });
        let req = ComposeRequest::full(FrameIndex::new(1), 1, 1);
        let r = compose_owned(&sprite, &cels, &req).unwrap();
        assert_eq!(r.pixels, vec![9, 8, 7, 255]);
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
            user_data: None,
            z_index: 0,
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

    #[test]
    fn rejects_slices_or_selection_marquee_overlay_alone() {
        let sprite = one_layer_sprite(1, 1, 1);
        let cels = CelMap::new();
        for overlays in [
            Overlays {
                slices: true,
                ..Overlays::default()
            },
            Overlays {
                selection_marquee: true,
                ..Overlays::default()
            },
        ] {
            let req = ComposeRequest {
                overlays,
                ..full_req(1, 1)
            };
            assert_eq!(
                compose_owned(&sprite, &cels, &req).unwrap_err(),
                RenderError::OverlaysUnsupported,
                "{overlays:?}"
            );
        }
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
