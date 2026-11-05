//! The `SetPixels` command — the basis of every drawing tool (spec §7.3).

use super::tiles::LayerTiles;
use super::{Command, Damage};
use crate::document::{Document, ImageBuffer};
use crate::error::DocumentError;
use crate::geometry::Rect;
use std::any::Any;

/// Overwrites pixels on a layer. All drawing tools emit this command.
///
/// Multiple `SetPixels` produced during one pointer drag merge into a single
/// undo step via [`Command::merge_with`] (spec §7.3, §9.2 Pencil). Merging is
/// gated on a shared [`stroke_id`](SetPixels::with_stroke): only edits tagged
/// with the same stroke coalesce, so separate strokes remain separate undo
/// steps. A command with no stroke id never merges.
///
/// Undo keeps only the tiles the edit changed (`super::tiles`,
/// docs/specs/performance-image-undo-memory.md): the new pixels are written
/// on the first apply, after which apply and revert swap the saved tiles
/// with the layer's.
pub struct SetPixels {
    /// Edits not written yet: layer, region and its new pixels.
    pending: Vec<(usize, Rect, ImageBuffer)>,
    /// Per changed layer, the tiles that are not in the document right now.
    saved: Vec<(usize, LayerTiles)>,
    /// Every region written, for [`Command::damage`].
    regions: Vec<Rect>,
    label: String,
    stroke_id: Option<u64>,
}

impl SetPixels {
    /// Creates a command writing `after` into `region` of layer `layer_index`.
    pub fn new(layer_index: usize, region: Rect, after: ImageBuffer) -> Self {
        Self {
            pending: vec![(layer_index, region, after)],
            saved: Vec::new(),
            regions: vec![region],
            label: "Edit Pixels".to_string(),
            stroke_id: None,
        }
    }

    /// Sets the history label (e.g. "Pencil Stroke").
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Tags this command with a stroke id so consecutive edits from the same
    /// pointer drag merge into one undo step (spec §7.3).
    pub fn with_stroke(mut self, stroke_id: u64) -> Self {
        self.stroke_id = Some(stroke_id);
        self
    }

    /// Number of sub-edits (one per merged stroke segment).
    pub fn edit_count(&self) -> usize {
        self.regions.len()
    }

    /// The saved tiles of `layer`, created on first use.
    fn tiles_of(&mut self, layer: usize) -> &mut LayerTiles {
        let at = match self.saved.iter().position(|(l, _)| *l == layer) {
            Some(at) => at,
            None => {
                self.saved.push((layer, LayerTiles::default()));
                self.saved.len() - 1
            }
        };
        &mut self.saved[at].1
    }

    /// Swaps every saved tile with the document's.
    fn swap(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let len = doc.layers.len();
        for (layer, tiles) in &mut self.saved {
            let target = doc
                .layers
                .get_mut(*layer)
                .ok_or(DocumentError::LayerIndexOutOfBounds { index: *layer, len })?;
            tiles.swap(&mut target.pixels);
        }
        Ok(())
    }
}

impl Command for SetPixels {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        if self.pending.is_empty() {
            // Redo: the saved tiles hold the pixels after the edit.
            return self.swap(doc);
        }
        let len = doc.layers.len();
        for (layer, region, after) in &self.pending {
            let target = doc
                .layers
                .get(*layer)
                .ok_or(DocumentError::LayerIndexOutOfBounds { index: *layer, len })?;
            let (w, h) = (target.pixels.width(), target.pixels.height());
            if after.width() != region.w
                || after.height() != region.h
                || region.x < 0
                || region.y < 0
                || region.right() > w as i32
                || region.bottom() > h as i32
            {
                return Err(DocumentError::RegionOutOfBounds);
            }
        }
        for (layer, region, after) in std::mem::take(&mut self.pending) {
            let mut tiles = std::mem::take(self.tiles_of(layer));
            tiles.write(&mut doc.layers[layer].pixels, region, &after);
            *self.tiles_of(layer) = tiles;
        }
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        if !self.pending.is_empty() {
            return Err(DocumentError::RegionOutOfBounds);
        }
        self.swap(doc)
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn merge_with(&mut self, newer: &dyn Command) -> bool {
        // Coalesce consecutive pixel edits from the *same* pointer drag into one
        // step. `newer` is already applied, so its saved tiles hold the pixels
        // before it: kept wherever this command has not saved the tile yet.
        // Only merge when both carry the same stroke id; otherwise distinct
        // strokes would collapse into a single undo step.
        match newer.as_any().downcast_ref::<SetPixels>() {
            Some(other)
                if self.stroke_id.is_some()
                    && self.stroke_id == other.stroke_id
                    && self.pending.is_empty()
                    && other.pending.is_empty() =>
            {
                for (layer, tiles) in &other.saved {
                    self.tiles_of(*layer).absorb(tiles);
                }
                self.regions.extend_from_slice(&other.regions);
                true
            }
            _ => false,
        }
    }

    fn damage(&self) -> Damage {
        let mut damage = Damage::None;
        for region in &self.regions {
            damage.add(Damage::rect(*region));
        }
        damage
    }

    fn undo_bytes(&self) -> usize {
        let saved: usize = self.saved.iter().map(|(_, t)| t.bytes()).sum();
        let pending: usize = self
            .pending
            .iter()
            .map(|(_, _, after)| after.data().len())
            .sum();
        saved + pending
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    fn solid_patch(w: u32, h: u32, c: Color) -> ImageBuffer {
        let mut b = ImageBuffer::new_transparent(w, h);
        for y in 0..h {
            for x in 0..w {
                b.set_pixel(x, y, c);
            }
        }
        b
    }

    #[test]
    fn apply_revert_round_trip_restores_exact_pixels() {
        let mut doc = Document::new(8, 8).unwrap();
        let mut cmd = SetPixels::new(0, Rect::new(2, 2, 3, 3), solid_patch(3, 3, Color::WHITE));
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(2, 2), Some(Color::WHITE));
        cmd.revert(&mut doc).unwrap();
        assert_eq!(
            doc.layers[0].pixels.get_pixel(2, 2),
            Some(Color::TRANSPARENT)
        );
    }

    #[test]
    fn merge_with_combines_two_segments_of_same_stroke() {
        let mut doc = Document::new(8, 8).unwrap();
        let mut a = SetPixels::new(0, Rect::new(0, 0, 2, 2), solid_patch(2, 2, Color::WHITE))
            .with_stroke(7);
        let mut b = SetPixels::new(0, Rect::new(4, 4, 2, 2), solid_patch(2, 2, Color::WHITE))
            .with_stroke(7);
        a.apply(&mut doc).unwrap();
        b.apply(&mut doc).unwrap();

        assert!(a.merge_with(&b));
        assert_eq!(a.edit_count(), 2);

        // Reverting the merged command restores BOTH regions.
        a.revert(&mut doc).unwrap();
        assert_eq!(
            doc.layers[0].pixels.get_pixel(0, 0),
            Some(Color::TRANSPARENT)
        );
        assert_eq!(
            doc.layers[0].pixels.get_pixel(4, 4),
            Some(Color::TRANSPARENT)
        );
    }

    #[test]
    fn merge_with_rejects_different_strokes() {
        let mut a = SetPixels::new(0, Rect::new(0, 0, 2, 2), solid_patch(2, 2, Color::WHITE))
            .with_stroke(1);
        let b = SetPixels::new(0, Rect::new(4, 4, 2, 2), solid_patch(2, 2, Color::WHITE))
            .with_stroke(2);
        assert!(!a.merge_with(&b));
        assert_eq!(a.edit_count(), 1);
    }

    #[test]
    fn merge_with_rejects_untagged_commands() {
        // Without a stroke id, two pixel edits stay independent undo steps.
        let mut a = SetPixels::new(0, Rect::new(0, 0, 2, 2), solid_patch(2, 2, Color::WHITE));
        let b = SetPixels::new(0, Rect::new(4, 4, 2, 2), solid_patch(2, 2, Color::WHITE));
        assert!(!a.merge_with(&b));
    }

    /// Undo bytes of `w`×`h` pixels rounded out to the undo tile grid.
    fn tile_rounded(r: Rect, layer_w: u32, layer_h: u32) -> usize {
        let t = super::super::UNDO_TILE as i32;
        let x0 = r.x.div_euclid(t) * t;
        let y0 = r.y.div_euclid(t) * t;
        let x1 = ((r.right() + t - 1).div_euclid(t) * t).min(layer_w as i32);
        let y1 = ((r.bottom() + t - 1).div_euclid(t) * t).min(layer_h as i32);
        ((x1 - x0) * (y1 - y0) * 4) as usize
    }

    #[test]
    fn undo_bytes_stay_within_the_tile_rounded_rect() {
        for r in [
            Rect::new(126, 126, 4, 4),
            Rect::new(0, 0, 300, 200),
            Rect::new(10, 140, 250, 3),
            Rect::new(299, 199, 1, 1),
        ] {
            let mut doc = Document::new(300, 200).unwrap();
            let mut cmd = SetPixels::new(0, r, solid_patch(r.w, r.h, Color::BLACK));
            cmd.apply(&mut doc).unwrap();
            assert!(cmd.undo_bytes() > 0, "{r:?} changed pixels");
            assert!(
                cmd.undo_bytes() <= tile_rounded(r, 300, 200),
                "{r:?}: {} > {}",
                cmd.undo_bytes(),
                tile_rounded(r, 300, 200)
            );
            // Reverted, it holds the pixels after the edit: same size.
            let held = cmd.undo_bytes();
            cmd.revert(&mut doc).unwrap();
            assert_eq!(cmd.undo_bytes(), held);
        }
    }

    #[test]
    fn merged_stroke_stores_each_tile_once() {
        let mut doc = Document::new(256, 256).unwrap();
        let original = doc.layers[0].pixels.clone();
        let mut stroke = SetPixels::new(
            0,
            Rect::new(10, 10, 20, 20),
            solid_patch(20, 20, Color::BLACK),
        )
        .with_stroke(3);
        stroke.apply(&mut doc).unwrap();
        // Forty overlapping segments inside the same tile.
        for i in 0..40 {
            let r = Rect::new(10 + i / 2, 12, 20, 20);
            let mut seg =
                SetPixels::new(0, r, solid_patch(20, 20, Color::rgba(i as u8, 0, 0, 255)))
                    .with_stroke(3);
            seg.apply(&mut doc).unwrap();
            assert!(stroke.merge_with(&seg));
        }
        let tile = (super::super::UNDO_TILE * super::super::UNDO_TILE * 4) as usize;
        assert_eq!(stroke.undo_bytes(), tile, "one tile, not one per segment");
        let after = doc.layers[0].pixels.clone();
        stroke.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels, original);
        stroke.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels, after);
    }

    #[test]
    fn edit_that_changes_nothing_stores_no_tiles() {
        let mut doc = Document::new(64, 64).unwrap();
        let mut cmd = SetPixels::new(
            0,
            Rect::new(0, 0, 64, 64),
            solid_patch(64, 64, Color::TRANSPARENT),
        );
        cmd.apply(&mut doc).unwrap();
        assert_eq!(cmd.undo_bytes(), 0);
    }

    #[test]
    fn region_outside_the_layer_is_refused_unchanged() {
        let mut doc = Document::new(8, 8).unwrap();
        let mut cmd = SetPixels::new(0, Rect::new(6, 6, 4, 4), solid_patch(4, 4, Color::WHITE));
        assert!(cmd.apply(&mut doc).is_err());
        assert_eq!(
            doc.layers[0].pixels.get_pixel(7, 7),
            Some(Color::TRANSPARENT)
        );
    }
}
