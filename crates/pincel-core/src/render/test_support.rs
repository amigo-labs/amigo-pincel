//! Shared fixtures for the render module's unit tests.

use crate::document::{
    Cel, CelData, CelMap, ColorMode, Frame, FrameIndex, Layer, LayerId, PixelBuffer, Sprite,
    TileImage, TileRef, Tileset, TilesetId,
};

use super::compose::compose;
use super::error::RenderError;
use super::request::ComposeRequest;

/// Test helper that owns the output buffer alongside the metadata so
/// existing `r.pixels` / `r.width` / `r.height` assertions keep working
/// after the M12.2 move of pixel ownership to a caller-provided `out`.
#[derive(Debug)]
pub(super) struct OwnedFrame {
    pub(super) pixels: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) fn compose_owned(
    sprite: &Sprite,
    cels: &CelMap,
    request: &ComposeRequest,
) -> Result<OwnedFrame, RenderError> {
    let mut pixels = Vec::new();
    let r = compose(sprite, cels, request, &mut pixels)?;
    Ok(OwnedFrame {
        pixels,
        width: r.width,
        height: r.height,
    })
}

pub(super) fn solid(w: u32, h: u32, rgba: [u8; 4]) -> PixelBuffer {
    let mut buf = PixelBuffer::empty(w, h, ColorMode::Rgba);
    for px in buf.data.chunks_exact_mut(4) {
        px.copy_from_slice(&rgba);
    }
    buf
}

pub(super) fn one_layer_sprite(w: u32, h: u32, frames: u32) -> Sprite {
    let mut b = Sprite::builder(w, h).add_layer(Layer::image(LayerId::new(0), "bg"));
    for _ in 0..frames {
        b = b.add_frame(Frame::default());
    }
    b.build().expect("test sprite")
}

pub(super) fn full_req(w: u32, h: u32) -> ComposeRequest {
    ComposeRequest::full(FrameIndex::new(0), w, h)
}

/// Two-tile tileset: tile 0 is the Aseprite empty tile (transparent),
/// tile 1 is a solid colored tile.
pub(super) fn two_tile_tileset(id: u32, tile_size: u32, color: [u8; 4]) -> Tileset {
    let mut ts = Tileset::new(TilesetId::new(id), "tiles", (tile_size, tile_size));
    ts.tiles.push(TileImage {
        pixels: PixelBuffer::empty(tile_size, tile_size, ColorMode::Rgba),
    });
    ts.tiles.push(TileImage {
        pixels: solid(tile_size, tile_size, color),
    });
    ts
}

pub(super) fn tilemap_cel(
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
