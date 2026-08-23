//! The tilemap-cel composition path. See `docs/specs/pincel.md` §4 and §8.

use crate::document::{BlendMode, ColorMode, FrameIndex, LayerId, PixelBuffer, TileRef, Tileset};
use crate::geometry::Rect;

use super::blend::{blend_pixel_into, mul_u8};
use super::error::RenderError;

/// Composite a tilemap cel into the viewport buffer. Iterates the grid in
/// row-major order, looks each tile up in `tileset`, and rasterizes the tile
/// (honoring `flip_x`, `flip_y`, and `rotate_90`) at its sprite-coord
/// position. Tile id `0` is the Aseprite empty / transparent tile and is
/// skipped without consulting the tileset.
// Private helper with a single call site in `compose()`; the arguments are
// the already-unpacked pieces of that caller's loop state, so bundling them
// into a one-off struct would only add indirection.
#[allow(clippy::too_many_arguments)]
pub(super) fn composite_tilemap_cel(
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
    combined_opacity: u8,
    blend_mode: BlendMode,
) -> Result<(), RenderError> {
    let (tile_w, tile_h) = tileset.tile_size;
    if tile_w == 0 || tile_h == 0 {
        return Ok(());
    }
    // `grid_w * grid_h` overflows `usize` on a 32-bit target (wasm32 is
    // the one we ship), so multiply checked and treat an overflowing grid
    // as a corrupt cel rather than indexing a wrongly-sized buffer.
    let Some(expected_len) = (grid_w as usize).checked_mul(grid_h as usize) else {
        return Err(RenderError::MalformedCelBuffer {
            layer: layer_id,
            frame,
        });
    };
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
                combined_opacity,
                blend_mode,
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
    combined_opacity: u8,
    blend_mode: BlendMode,
) {
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
            blend_pixel_into(
                blend_mode,
                &mut dst[d..d + 4],
                src.data[s],
                src.data[s + 1],
                src.data[s + 2],
                sa,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{
        Cel, CelData, CelMap, Frame, Layer, LayerId, Sprite, TileImage, TilesetId,
    };

    use super::super::test_support::{
        compose_owned, full_req, solid, tilemap_cel, two_tile_tileset,
    };

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
        let r = compose_owned(&sprite, &cels, &full_req(4, 4)).unwrap();
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
        let r = compose_owned(&sprite, &cels, &full_req(2, 2)).unwrap();
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
        let r = compose_owned(&sprite, &cels, &full_req(2, 2)).unwrap();
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
        let r = compose_owned(&sprite, &cels, &full_req(2, 2)).unwrap();
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
            compose_owned(&sprite, &cels, &full_req(2, 2)).unwrap_err(),
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
            compose_owned(&sprite, &cels, &full_req(2, 2)).unwrap_err(),
            RenderError::TileIdOutOfRange {
                layer: LayerId::new(0),
                frame: FrameIndex::new(0),
                tile_id: 42,
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
            compose_owned(&sprite, &cels, &full_req(4, 4)).unwrap_err(),
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
        let r = compose_owned(&sprite, &cels, &full_req(2, 2)).unwrap();
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
            compose_owned(&sprite, &cels, &full_req(4, 4)).unwrap_err(),
            RenderError::MalformedCelBuffer {
                layer: LayerId::new(0),
                frame: FrameIndex::new(0),
            }
        );
    }

    #[test]
    fn tilemap_grid_whose_tile_count_overflows_usize_errors() {
        // 0x10000 * 0x10000 == 2^32, which wraps to 0 in a 32-bit `usize`
        // (wasm32). Without the checked multiply the empty tile vector would
        // then look well-formed and the grid walk would index out of bounds.
        let tileset = two_tile_tileset(0, 2, [255, 0, 0, 255]);
        let mut dst = vec![0u8; 4 * 4 * 4];
        let err = composite_tilemap_cel(
            &mut dst,
            Rect::new(0, 0, 4, 4),
            (0, 0),
            0x10000,
            0x10000,
            &[],
            &tileset,
            LayerId::new(0),
            FrameIndex::new(0),
            ColorMode::Rgba,
            255,
            BlendMode::Normal,
        )
        .expect_err("an overflowing grid is a corrupt cel");
        assert_eq!(
            err,
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
        let r = compose_owned(&sprite, &cels, &full_req(4, 4)).unwrap();
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
}
