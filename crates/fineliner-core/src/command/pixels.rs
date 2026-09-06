//! The `SetPixels` command — the basis of every drawing tool (spec §7.3).

use super::Command;
use crate::document::{Document, ImageBuffer};
use crate::error::DocumentError;
use crate::geometry::Rect;
use std::any::Any;

/// A single rectangular pixel edit on one layer.
///
/// `before` is captured lazily on first apply, so a stroke can be constructed
/// before the target pixels are known (spec §7.3).
#[derive(Debug, Clone)]
pub struct PixelEdit {
    /// Target layer index.
    pub layer_index: usize,
    /// Region of the layer being overwritten.
    pub region: Rect,
    /// Pixels as they were before the edit (captured on first apply).
    pub before: Option<ImageBuffer>,
    /// Pixels to write into `region`.
    pub after: ImageBuffer,
}

impl PixelEdit {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let layer =
            doc.layers
                .get_mut(self.layer_index)
                .ok_or(DocumentError::LayerIndexOutOfBounds {
                    index: self.layer_index,
                    len: 0,
                })?;
        if self.before.is_none() {
            self.before = Some(layer.pixels.copy_region(self.region)?);
        }
        layer.pixels.paste_region(self.region, &self.after)
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let before = self
            .before
            .as_ref()
            .ok_or(DocumentError::RegionOutOfBounds)?;
        let layer =
            doc.layers
                .get_mut(self.layer_index)
                .ok_or(DocumentError::LayerIndexOutOfBounds {
                    index: self.layer_index,
                    len: 0,
                })?;
        layer.pixels.paste_region(self.region, before)
    }
}

/// Overwrites pixels on a layer. All drawing tools emit this command.
///
/// Multiple `SetPixels` produced during one pointer drag merge into a single
/// undo step via [`Command::merge_with`] (spec §7.3, §9.2 Pencil). Merging is
/// gated on a shared [`stroke_id`](SetPixels::with_stroke): only edits tagged
/// with the same stroke coalesce, so separate strokes remain separate undo
/// steps. A command with no stroke id never merges.
pub struct SetPixels {
    edits: Vec<PixelEdit>,
    label: String,
    stroke_id: Option<u64>,
}

impl SetPixels {
    /// Creates a command writing `after` into `region` of layer `layer_index`.
    pub fn new(layer_index: usize, region: Rect, after: ImageBuffer) -> Self {
        Self {
            edits: vec![PixelEdit {
                layer_index,
                region,
                before: None,
                after,
            }],
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
        self.edits.len()
    }
}

impl Command for SetPixels {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        for edit in &mut self.edits {
            edit.apply(doc)?;
        }
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        // Revert in reverse so chained edits restore the original state.
        for edit in self.edits.iter_mut().rev() {
            edit.revert(doc)?;
        }
        Ok(())
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn merge_with(&mut self, newer: &dyn Command) -> bool {
        // Coalesce consecutive pixel edits from the *same* pointer drag into one
        // step. `newer` is already applied, so its `before` buffers are
        // populated. Only merge when both carry the same stroke id; otherwise
        // distinct strokes would collapse into a single undo step.
        match newer.as_any().downcast_ref::<SetPixels>() {
            Some(other) if self.stroke_id.is_some() && self.stroke_id == other.stroke_id => {
                self.edits.extend(other.edits.iter().cloned());
                true
            }
            _ => false,
        }
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
}
