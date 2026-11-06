//! Tile-based undo storage for pixel edits
//! (docs/specs/performance-image-undo-memory.md).
//!
//! A [`LayerTiles`] keeps, for one layer, the [`TILE`]×[`TILE`] tiles a
//! command changed — the pixels that are *not* currently in the document.
//! Applying and reverting are then the same operation: swap the stored
//! tiles with the layer's. The undo memory of an edit grows with the tiles
//! it touched, never with the document.

use std::collections::BTreeMap;

use crate::document::ImageBuffer;
use crate::geometry::Rect;

/// Edge length of an undo tile in pixels; the grid starts at the layer's
/// top-left corner. Tiles on the right / bottom edge are clipped to the
/// layer.
pub const TILE: u32 = 64;

/// Saved tiles of one layer, keyed by tile column and row.
#[derive(Debug, Clone, Default)]
pub(crate) struct LayerTiles {
    tiles: BTreeMap<(u32, u32), Vec<u8>>,
}

/// The rect of tile `(tx, ty)` in a `w`×`h` buffer, clipped to it.
fn tile_rect(tx: u32, ty: u32, w: u32, h: u32) -> Rect {
    let x = tx * TILE;
    let y = ty * TILE;
    Rect::new(x as i32, y as i32, TILE.min(w - x), TILE.min(h - y))
}

/// The tile columns and rows `region` overlaps in a `w`×`h` buffer.
fn tiles_over(region: Rect, w: u32, h: u32) -> impl Iterator<Item = (u32, u32)> {
    let clipped = region.intersect(Rect::new(0, 0, w, h));
    let (tx0, ty0, tx1, ty1) = match clipped {
        Some(r) => (
            r.x as u32 / TILE,
            r.y as u32 / TILE,
            (r.right() as u32).div_ceil(TILE),
            (r.bottom() as u32).div_ceil(TILE),
        ),
        None => (0, 0, 0, 0),
    };
    (ty0..ty1).flat_map(move |ty| (tx0..tx1).map(move |tx| (tx, ty)))
}

/// Byte range of row `row` of `rect` in a buffer `w` pixels wide.
fn row_range(rect: Rect, row: u32, w: u32) -> std::ops::Range<usize> {
    let start = ((rect.y as u32 + row) as usize * w as usize + rect.x as usize) * 4;
    start..start + rect.w as usize * 4
}

impl LayerTiles {
    /// Saves the tiles of `layer` that writing `after` into `region` would
    /// change, unless already saved, and writes it. A tile whose pixels
    /// `after` leaves as they are is not saved. `region` must lie inside the
    /// layer and match `after`'s size.
    pub(crate) fn write(&mut self, layer: &mut ImageBuffer, region: Rect, after: &ImageBuffer) {
        let (w, h) = (layer.width(), layer.height());
        for key in tiles_over(region, w, h) {
            if self.tiles.contains_key(&key) {
                continue;
            }
            let tile = tile_rect(key.0, key.1, w, h);
            let Some(part) = tile.intersect(region) else {
                continue;
            };
            let changed = (0..part.h).any(|row| {
                let dst = row_range(part, row, w);
                let src_rect = Rect::new(part.x - region.x, part.y - region.y, part.w, part.h);
                let src = row_range(src_rect, row, region.w);
                layer.data()[dst] != after.data()[src]
            });
            if changed {
                self.tiles.insert(key, copy_rect(layer, tile));
            }
        }
        for row in 0..region.h {
            let dst = row_range(region, row, w);
            let src = row_range(Rect::new(0, 0, region.w, region.h), row, region.w);
            layer.data_mut()[dst].copy_from_slice(&after.data()[src]);
        }
    }

    /// Saves every tile of `before` where it differs from `after` (two
    /// buffers of the same size): the undo data of replacing a whole layer.
    pub(crate) fn diff(before: &ImageBuffer, after: &ImageBuffer) -> Self {
        let (w, h) = (before.width(), before.height());
        let mut out = Self::default();
        for key in tiles_over(Rect::new(0, 0, w, h), w, h) {
            let tile = tile_rect(key.0, key.1, w, h);
            let changed = (0..tile.h).any(|row| {
                let r = row_range(tile, row, w);
                before.data()[r.clone()] != after.data()[r]
            });
            if changed {
                out.tiles.insert(key, copy_rect(before, tile));
            }
        }
        out
    }

    /// Exchanges the saved tiles with the layer's pixels: after an apply
    /// this restores the state before it, and the other way round.
    pub(crate) fn swap(&mut self, layer: &mut ImageBuffer) {
        let (w, h) = (layer.width(), layer.height());
        for (&(tx, ty), saved) in &mut self.tiles {
            let tile = tile_rect(tx, ty, w, h);
            let len = tile.w as usize * 4;
            for row in 0..tile.h {
                let at = row as usize * len;
                layer.data_mut()[row_range(tile, row, w)].swap_with_slice(&mut saved[at..at + len]);
            }
        }
    }

    /// Adds the tiles of `newer` this store does not hold yet. For a newer
    /// edit applied right after this one, the result undoes both.
    pub(crate) fn absorb(&mut self, newer: &LayerTiles) {
        for (key, tile) in &newer.tiles {
            self.tiles.entry(*key).or_insert_with(|| tile.clone());
        }
    }

    /// Bytes of pixel data held.
    pub(crate) fn bytes(&self) -> usize {
        self.tiles.values().map(Vec::len).sum()
    }

    /// Number of saved tiles.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.tiles.len()
    }
}

/// The pixels of `rect` (inside `buf`) as tightly packed rows.
fn copy_rect(buf: &ImageBuffer, rect: Rect) -> Vec<u8> {
    let mut out = Vec::with_capacity(rect.w as usize * rect.h as usize * 4);
    for row in 0..rect.h {
        out.extend_from_slice(&buf.data()[row_range(rect, row, buf.width())]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    fn solid(w: u32, h: u32, c: Color) -> ImageBuffer {
        let mut b = ImageBuffer::new_transparent(w, h);
        for y in 0..h {
            for x in 0..w {
                b.set_pixel(x, y, c);
            }
        }
        b
    }

    #[test]
    fn write_at_tile_corner_saves_four_tiles() {
        let mut layer = ImageBuffer::new_transparent(300, 300);
        let mut t = LayerTiles::default();
        t.write(
            &mut layer,
            Rect::new(126, 126, 4, 4),
            &solid(4, 4, Color::BLACK),
        );
        assert_eq!(t.len(), 4);
        assert_eq!(t.bytes(), 4 * (TILE * TILE * 4) as usize);
    }

    #[test]
    fn write_clips_edge_tiles_to_the_layer() {
        let mut layer = ImageBuffer::new_transparent(130, 10);
        let mut t = LayerTiles::default();
        t.write(
            &mut layer,
            Rect::new(129, 0, 1, 1),
            &solid(1, 1, Color::BLACK),
        );
        assert_eq!(t.bytes(), 2 * 10 * 4, "one 2×10 edge tile");
    }

    #[test]
    fn write_without_change_saves_nothing() {
        let mut layer = solid(200, 200, Color::WHITE);
        let mut t = LayerTiles::default();
        t.write(
            &mut layer,
            Rect::new(10, 10, 150, 150),
            &solid(150, 150, Color::WHITE),
        );
        assert_eq!(t.bytes(), 0);
    }

    #[test]
    fn swap_toggles_between_before_and_after() {
        let mut layer = solid(200, 140, Color::WHITE);
        let original = layer.clone();
        let mut t = LayerTiles::default();
        t.write(
            &mut layer,
            Rect::new(120, 100, 30, 30),
            &solid(30, 30, Color::BLACK),
        );
        let after = layer.clone();
        assert_ne!(after, original);
        t.swap(&mut layer);
        assert_eq!(layer, original, "undo");
        t.swap(&mut layer);
        assert_eq!(layer, after, "redo");
    }

    #[test]
    fn absorb_keeps_the_older_tile() {
        let mut layer = solid(64, 64, Color::WHITE);
        let original = layer.clone();
        let mut a = LayerTiles::default();
        a.write(
            &mut layer,
            Rect::new(0, 0, 4, 4),
            &solid(4, 4, Color::BLACK),
        );
        let mut b = LayerTiles::default();
        b.write(
            &mut layer,
            Rect::new(2, 2, 4, 4),
            &solid(4, 4, Color::rgba(255, 0, 0, 255)),
        );
        a.absorb(&b);
        a.swap(&mut layer);
        assert_eq!(layer, original);
    }

    #[test]
    fn diff_saves_only_changed_tiles() {
        let before = solid(300, 300, Color::WHITE);
        let mut after = before.clone();
        after.set_pixel(299, 299, Color::BLACK);
        let mut t = LayerTiles::diff(&before, &after);
        assert_eq!(t.len(), 1);
        let mut layer = after.clone();
        t.swap(&mut layer);
        assert_eq!(layer, before);
    }
}
