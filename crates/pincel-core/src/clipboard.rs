//! Pixel extraction for Copy / Cut / Copy Merged (spec §5.4 Paste,
//! Decision Log 2026-10-29). Pure data in, RGBA8 out — encoding the PNG
//! and talking to the system clipboard are the callers' business.

use thiserror::Error;

use crate::document::{CelData, CelMap, ColorMode, FrameIndex, LayerId, LayerKind, Sprite};
use crate::geometry::Rect;
use crate::render::{ComposeRequest, LayerFilter, RenderError, compose};

/// Background-layer bit of [`crate::document::Layer::extra_flags`]
/// (Aseprite layer flag `8`).
const BACKGROUND_FLAG: u16 = 0x8;

/// Where Copy takes its pixels from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopySource {
    /// One layer: an image layer's own cel pixels (raw, without layer or
    /// cel opacity); a group's or tilemap's rendered pixels.
    Layer(LayerId),
    /// The composite of every visible layer (Copy Merged).
    Merged,
}

/// The pixels Copy collected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiedPixels {
    /// Sprite-space region the pixels came from: the selection's bounding
    /// box clipped to the canvas, or the whole canvas without a selection.
    pub rect: Rect,
    /// Row-major, non-premultiplied RGBA8, `rect.width × rect.height`.
    /// Pixels outside a shaped selection are fully transparent.
    pub rgba: Vec<u8>,
}

/// Why Copy produced nothing.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CopyError {
    /// No layer has this id.
    #[error("unknown layer id {0}")]
    UnknownLayer(u32),
    /// The selection does not overlap the canvas.
    #[error("the selection lies outside the canvas")]
    EmptyRegion,
    /// Rendering a group, a tilemap or the composite failed.
    #[error(transparent)]
    Render(#[from] RenderError),
}

/// Collects what Copy puts on the clipboard: `source` at `frame`, cropped
/// to the active selection's bounding box (whole canvas without one), with
/// pixels outside a shaped selection cleared to transparent. Indexed and
/// grayscale image cels are expanded to RGBA through the palette.
pub fn copy_pixels(
    sprite: &Sprite,
    cels: &CelMap,
    frame: FrameIndex,
    source: CopySource,
) -> Result<CopiedPixels, CopyError> {
    let canvas = Rect::new(0, 0, sprite.width, sprite.height);
    let rect = match sprite.selection {
        Some(sel) if sprite.has_selection() => sel.intersect(canvas),
        _ => canvas,
    };
    if rect.is_empty() {
        return Err(CopyError::EmptyRegion);
    }
    let full = match source {
        CopySource::Merged => render(sprite, cels, frame, LayerFilter::Visible)?,
        CopySource::Layer(id) => {
            let layer = sprite.layer(id).ok_or(CopyError::UnknownLayer(id.0))?;
            if matches!(layer.kind, LayerKind::Image) {
                image_layer_rgba(sprite, cels, id, frame)
            } else {
                render(sprite, cels, frame, LayerFilter::Only(subtree(sprite, id)))?
            }
        }
    };
    let mask = sprite
        .selection_mask
        .as_ref()
        .filter(|_| sprite.has_selection());
    let canvas_w = sprite.width as usize;
    let mut rgba = Vec::with_capacity(rect.width as usize * rect.height as usize * 4);
    for y in rect.y..rect.y + rect.height as i32 {
        for x in rect.x..rect.x + rect.width as i32 {
            if mask.is_some_and(|m| !m.contains(x, y)) {
                rgba.extend_from_slice(&[0; 4]);
                continue;
            }
            let i = (y as usize * canvas_w + x as usize) * 4;
            rgba.extend_from_slice(&full[i..i + 4]);
        }
    }
    Ok(CopiedPixels { rect, rgba })
}

/// Full-canvas composite of the layers `filter` admits, no onion skin.
fn render(
    sprite: &Sprite,
    cels: &CelMap,
    frame: FrameIndex,
    filter: LayerFilter,
) -> Result<Vec<u8>, RenderError> {
    let mut request = ComposeRequest::full(frame, sprite.width, sprite.height);
    request.include_layers = filter;
    let mut out = Vec::new();
    compose(sprite, cels, &request, &mut out)?;
    Ok(out)
}

/// `root` plus every layer nested in it.
fn subtree(sprite: &Sprite, root: LayerId) -> Vec<LayerId> {
    sprite
        .layers
        .iter()
        .filter(|l| {
            let mut next = Some(l.id);
            // Bounded so a malformed parent cycle cannot hang.
            for _ in 0..=sprite.layers.len() {
                match next {
                    Some(id) if id == root => return true,
                    Some(id) => next = sprite.layer(id).and_then(|p| p.parent),
                    None => return false,
                }
            }
            false
        })
        .map(|l| l.id)
        .collect()
}

/// An image layer's cel at `frame` as canvas-sized RGBA8 (a linked cel is
/// followed one hop); transparent without a cel.
fn image_layer_rgba(sprite: &Sprite, cels: &CelMap, id: LayerId, frame: FrameIndex) -> Vec<u8> {
    let (w, h) = (sprite.width as i64, sprite.height as i64);
    let mut out = vec![0u8; (w * h * 4) as usize];
    let cel = match cels.get(id, frame) {
        Some(c) => match c.data {
            CelData::Linked(target) => cels.get(id, target),
            _ => Some(c),
        },
        None => None,
    };
    let Some(cel) = cel else { return out };
    let CelData::Image(buf) = &cel.data else {
        return out;
    };
    let background = sprite
        .layer(id)
        .is_some_and(|l| l.extra_flags & BACKGROUND_FLAG != 0);
    let bpp = buf.color_mode.bytes_per_pixel();
    if !buf.is_well_formed() {
        return out;
    }
    for ly in 0..buf.height as i64 {
        let cy = cel.position.1 as i64 + ly;
        if !(0..h).contains(&cy) {
            continue;
        }
        for lx in 0..buf.width as i64 {
            let cx = cel.position.0 as i64 + lx;
            if !(0..w).contains(&cx) {
                continue;
            }
            let si = ((ly * buf.width as i64 + lx) as usize) * bpp;
            let px = &buf.data[si..si + bpp];
            let rgba = match buf.color_mode {
                ColorMode::Rgba => [px[0], px[1], px[2], px[3]],
                ColorMode::Grayscale => [px[0], px[0], px[0], px[1]],
                ColorMode::Indexed { transparent_index } => {
                    if px[0] == transparent_index && !background {
                        [0; 4]
                    } else {
                        sprite
                            .palette
                            .colors
                            .get(usize::from(px[0]))
                            .map_or([0; 4], |e| [e.rgba.r, e.rgba.g, e.rgba.b, e.rgba.a])
                    }
                }
            };
            let di = ((cy * w + cx) as usize) * 4;
            out[di..di + 4].copy_from_slice(&rgba);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{
        Cel, Frame, Layer, Palette, PaletteEntry, PixelBuffer, Rgba, TileImage, TileRef, Tileset,
        TilesetId,
    };
    use crate::selection::SelectionMask;

    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    /// 4×2 RGBA sprite, layer 0 (bottom) filled red, layer 1 with one
    /// blue pixel at (1, 0).
    fn two_layers() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(4, 2)
            .add_layer(Layer::image(LayerId::new(0), "bottom"))
            .add_layer(Layer::image(LayerId::new(1), "top"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite");
        let mut cels = CelMap::new();
        let mut bottom = PixelBuffer::empty(4, 2, ColorMode::Rgba);
        for px in bottom.data.chunks_mut(4) {
            px.copy_from_slice(&RED);
        }
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), bottom));
        let mut top = PixelBuffer::empty(4, 2, ColorMode::Rgba);
        top.data[4..8].copy_from_slice(&BLUE);
        cels.insert(Cel::image(LayerId::new(1), FrameIndex::new(0), top));
        (sprite, cels)
    }

    fn px(c: &CopiedPixels, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * c.rect.width + x) * 4) as usize;
        [c.rgba[i], c.rgba[i + 1], c.rgba[i + 2], c.rgba[i + 3]]
    }

    #[test]
    fn copy_pixels_with_selection_crops_to_the_bounding_box() {
        let (mut sprite, cels) = two_layers();
        sprite.set_selection(Rect::new(1, 0, 2, 1));
        let c = copy_pixels(
            &sprite,
            &cels,
            FrameIndex::new(0),
            CopySource::Layer(LayerId::new(1)),
        )
        .expect("copy");
        assert_eq!(c.rect, Rect::new(1, 0, 2, 1));
        assert_eq!(c.rgba.len(), 2 * 4);
        assert_eq!(px(&c, 0, 0), BLUE);
        assert_eq!(px(&c, 1, 0), [0; 4]);
    }

    #[test]
    fn copy_pixels_shaped_selection_clears_unselected_pixels() {
        let (mut sprite, cels) = two_layers();
        let mut mask = SelectionMask::rect(4, 2, Rect::new(0, 0, 1, 1));
        mask.combine(
            &SelectionMask::rect(4, 2, Rect::new(2, 1, 1, 1)),
            crate::selection::SelectionMode::Add,
        );
        sprite.set_selection_mask(mask);
        let c = copy_pixels(
            &sprite,
            &cels,
            FrameIndex::new(0),
            CopySource::Layer(LayerId::new(0)),
        )
        .expect("copy");
        assert_eq!(c.rect, Rect::new(0, 0, 3, 2));
        assert_eq!(px(&c, 0, 0), RED);
        assert_eq!(px(&c, 1, 0), [0; 4], "outside the shape");
        assert_eq!(px(&c, 2, 1), RED);
        assert_eq!(px(&c, 0, 1), [0; 4], "outside the shape");
    }

    #[test]
    fn copy_pixels_without_selection_takes_the_whole_layer() {
        let (sprite, cels) = two_layers();
        let c = copy_pixels(
            &sprite,
            &cels,
            FrameIndex::new(0),
            CopySource::Layer(LayerId::new(0)),
        )
        .expect("copy");
        assert_eq!(c.rect, Rect::new(0, 0, 4, 2));
        assert!(c.rgba.chunks(4).all(|p| p == RED));
    }

    #[test]
    fn copy_pixels_merged_takes_the_visible_composite() {
        let (mut sprite, cels) = two_layers();
        sprite.set_selection(Rect::new(0, 0, 2, 1));
        let c = copy_pixels(&sprite, &cels, FrameIndex::new(0), CopySource::Merged).expect("copy");
        assert_eq!(c.rect, Rect::new(0, 0, 2, 1));
        assert_eq!(px(&c, 0, 0), RED);
        assert_eq!(px(&c, 1, 0), BLUE);
    }

    #[test]
    fn copy_pixels_ignores_layer_opacity_and_visibility() {
        let (mut sprite, cels) = two_layers();
        sprite.layers[1].opacity = 10;
        sprite.layers[1].visible = false;
        let c = copy_pixels(
            &sprite,
            &cels,
            FrameIndex::new(0),
            CopySource::Layer(LayerId::new(1)),
        )
        .expect("copy");
        assert_eq!(px(&c, 1, 0), BLUE);
    }

    #[test]
    fn copy_pixels_missing_cel_is_transparent() {
        let (mut sprite, cels) = two_layers();
        sprite.layers.push(Layer::image(LayerId::new(2), "empty"));
        sprite.set_selection(Rect::new(0, 0, 3, 2));
        let c = copy_pixels(
            &sprite,
            &cels,
            FrameIndex::new(0),
            CopySource::Layer(LayerId::new(2)),
        )
        .expect("copy");
        assert_eq!(c.rgba, vec![0; 3 * 2 * 4]);
    }

    #[test]
    fn copy_pixels_selection_partly_off_canvas_is_clipped() {
        let (mut sprite, cels) = two_layers();
        sprite.set_selection(Rect::new(-2, 1, 4, 5));
        let c = copy_pixels(
            &sprite,
            &cels,
            FrameIndex::new(0),
            CopySource::Layer(LayerId::new(0)),
        )
        .expect("copy");
        assert_eq!(c.rect, Rect::new(0, 1, 2, 1));
        sprite.set_selection(Rect::new(10, 10, 2, 2));
        assert_eq!(
            copy_pixels(&sprite, &cels, FrameIndex::new(0), CopySource::Merged),
            Err(CopyError::EmptyRegion)
        );
    }

    #[test]
    fn copy_pixels_unknown_layer_errors() {
        let (sprite, cels) = two_layers();
        assert_eq!(
            copy_pixels(
                &sprite,
                &cels,
                FrameIndex::new(0),
                CopySource::Layer(LayerId::new(9))
            ),
            Err(CopyError::UnknownLayer(9))
        );
    }

    #[test]
    fn copy_pixels_indexed_expands_through_the_palette() {
        let palette = Palette {
            colors: vec![
                PaletteEntry::new(Rgba::new(1, 2, 3, 255)),
                PaletteEntry::new(Rgba::new(10, 20, 30, 128)),
            ],
        };
        let sprite = Sprite::builder(4, 1)
            .color_mode(ColorMode::Indexed {
                transparent_index: 0,
            })
            .add_layer(Layer::image(LayerId::new(0), "l"))
            .add_frame(Frame::new(100))
            .palette(palette)
            .build()
            .expect("sprite");
        let mut buf = PixelBuffer::empty(
            4,
            1,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        );
        buf.data = vec![1, 0, 7, 1];
        let mut cels = CelMap::new();
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), buf));
        let c = copy_pixels(
            &sprite,
            &cels,
            FrameIndex::new(0),
            CopySource::Layer(LayerId::new(0)),
        )
        .expect("copy");
        assert_eq!(
            px(&c, 0, 0),
            [10, 20, 30, 128],
            "palette entry with its alpha"
        );
        assert_eq!(px(&c, 1, 0), [0; 4], "transparent index");
        assert_eq!(px(&c, 2, 0), [0; 4], "index past the palette");
        assert_eq!(px(&c, 3, 0), [10, 20, 30, 128]);
    }

    #[test]
    fn copy_pixels_group_takes_its_rendered_subtree() {
        let (mut sprite, mut cels) = two_layers();
        let group = LayerId::new(5);
        sprite.layers.push(Layer::group(group, "g"));
        sprite.layers[1].parent = Some(group);
        // A hidden child stays out of the group's render, as on screen.
        sprite.layers.push(Layer::image(LayerId::new(6), "hidden"));
        sprite.layers[3].parent = Some(group);
        sprite.layers[3].visible = false;
        let mut hidden = PixelBuffer::empty(4, 2, ColorMode::Rgba);
        hidden.data[0..4].copy_from_slice(&[0, 255, 0, 255]);
        cels.insert(Cel::image(LayerId::new(6), FrameIndex::new(0), hidden));
        let c = copy_pixels(&sprite, &cels, FrameIndex::new(0), CopySource::Layer(group))
            .expect("copy");
        assert_eq!(px(&c, 1, 0), BLUE, "the child's pixels");
        assert_eq!(px(&c, 2, 0), [0; 4], "not the layer outside the group");
    }

    #[test]
    fn copy_pixels_tilemap_takes_its_rendered_tiles() {
        let mut sprite = Sprite::builder(2, 1)
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite");
        let mut tileset = Tileset::new(TilesetId::new(0), "t", (1, 1));
        let mut tile = PixelBuffer::empty(1, 1, ColorMode::Rgba);
        tile.data.copy_from_slice(&BLUE);
        let empty = PixelBuffer::empty(1, 1, ColorMode::Rgba);
        tileset.tiles.push(TileImage { pixels: empty });
        tileset.tiles.push(TileImage { pixels: tile });
        sprite.tilesets.push(tileset);
        sprite
            .layers
            .push(Layer::tilemap(LayerId::new(0), "map", TilesetId::new(0)));
        let mut cels = CelMap::new();
        cels.insert(Cel {
            data: CelData::Tilemap {
                grid_w: 2,
                grid_h: 1,
                tiles: vec![TileRef::new(1), TileRef::new(0)],
            },
            ..Cel::image(
                LayerId::new(0),
                FrameIndex::new(0),
                PixelBuffer::empty(1, 1, ColorMode::Rgba),
            )
        });
        let c = copy_pixels(
            &sprite,
            &cels,
            FrameIndex::new(0),
            CopySource::Layer(LayerId::new(0)),
        )
        .expect("copy");
        assert_eq!(px(&c, 0, 0), BLUE);
        assert_eq!(px(&c, 1, 0), [0; 4], "tile id 0 is empty");
    }
}
