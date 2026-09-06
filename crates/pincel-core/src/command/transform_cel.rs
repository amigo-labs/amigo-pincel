//! `TransformCel` command — flip or rotate the pixels of one image cel,
//! optionally restricted to a sprite-space rect (the selection).
//!
//! The cel is normalised to a canvas-sized buffer at `(0, 0)` first, so
//! trimmed cels from opened files behave like tool-created ones. The
//! region (the whole canvas when no rect is given) is reoriented about
//! its own centre; a quarter turn of a non-square region keeps what fits
//! inside it. `revert` restores the exact prior cel.

use crate::document::{CelMap, FrameIndex, LayerId, Sprite};
use crate::geometry::Rect;

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;
use super::transform_support::{Orientation, cel_to_canvas, raster_to_cel, rgba_buffer};

/// Reorient the pixels of the cel at `(layer, frame)`.
#[derive(Debug, Clone)]
pub struct TransformCel {
    layer: LayerId,
    frame: FrameIndex,
    op: Orientation,
    region: Option<Rect>,
    prior: Option<crate::document::Cel>,
}

impl TransformCel {
    pub fn new(layer: LayerId, frame: FrameIndex, op: Orientation) -> Self {
        Self {
            layer,
            frame,
            op,
            region: None,
            prior: None,
        }
    }

    /// Restrict the transform to `rect` (sprite space), leaving pixels
    /// outside it untouched.
    pub fn within(mut self, rect: Rect) -> Self {
        self.region = Some(rect);
        self
    }
}

impl Command for TransformCel {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let cel = cels
            .get(self.layer, self.frame)
            .ok_or(CommandError::MissingCel {
                layer: self.layer,
                frame: self.frame,
            })?;
        let buffer = rgba_buffer(cel).ok_or(CommandError::NotAnImageCel {
            layer: self.layer,
            frame: self.frame,
        })?;
        let mut raster = cel_to_canvas(cel, buffer, doc.width, doc.height);
        let canvas = Rect::new(0, 0, doc.width, doc.height);
        let region = self.region.map_or(canvas, |r| r.intersect(canvas));
        if region.is_empty() {
            return Err(CommandError::EmptyRegion);
        }
        raster.orient_region(region, self.op);
        let prior = cels
            .remove(self.layer, self.frame)
            .expect("cel presence checked above");
        cels.insert(raster_to_cel(self.layer, self.frame, raster));
        self.prior = Some(prior);
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        let Some(prior) = self.prior.take() else {
            return;
        };
        cels.insert(prior);
    }

    fn dirty_region(&self) -> DirtyRegion {
        if self.prior.is_none() {
            return DirtyRegion::None;
        }
        match self.region {
            Some(r) => DirtyRegion::layer_rect(self.layer, self.frame, r),
            None => DirtyRegion::Canvas,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Bus;
    use crate::document::{Cel, CelData, ColorMode, Frame, Layer, PixelBuffer};

    const L: LayerId = LayerId::new(0);
    const F: FrameIndex = FrameIndex::new(0);

    fn doc(w: u32, h: u32, px: &[u8]) -> (Sprite, CelMap) {
        let sprite = Sprite::builder(w, h)
            .add_layer(Layer::image(L, "a"))
            .add_frame(Frame::new(100))
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        let mut buf = PixelBuffer::empty(w, h, ColorMode::Rgba);
        buf.data = px.to_vec();
        cels.insert(Cel::image(L, F, buf));
        (sprite, cels)
    }

    fn data(cels: &CelMap) -> Vec<u8> {
        match &cels.get(L, F).unwrap().data {
            CelData::Image(b) => b.data.clone(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn flip_horizontal_mirrors_and_undo_restores() {
        let (mut s, mut c) = doc(2, 1, &[1, 1, 1, 255, 2, 2, 2, 255]);
        let before = data(&c);
        let mut bus = Bus::new();
        bus.execute(
            TransformCel::new(L, F, Orientation::FlipHorizontal).into(),
            &mut s,
            &mut c,
        )
        .unwrap();
        assert_eq!(data(&c), vec![2, 2, 2, 255, 1, 1, 1, 255]);
        assert!(bus.undo(&mut s, &mut c));
        assert_eq!(data(&c), before);
    }

    #[test]
    fn within_rect_leaves_outside_pixels_alone() {
        let (mut s, mut c) = doc(3, 1, &[1, 1, 1, 255, 2, 2, 2, 255, 3, 3, 3, 255]);
        let mut cmd =
            TransformCel::new(L, F, Orientation::FlipHorizontal).within(Rect::new(1, 0, 2, 1));
        cmd.apply(&mut s, &mut c).unwrap();
        assert_eq!(data(&c), vec![1, 1, 1, 255, 3, 3, 3, 255, 2, 2, 2, 255]);
        assert_eq!(
            cmd.dirty_region(),
            DirtyRegion::layer_rect(L, F, Rect::new(1, 0, 2, 1))
        );
    }

    #[test]
    fn trimmed_cel_is_normalised_to_canvas_size() {
        let (mut s, mut c) = doc(4, 4, &[0; 64]);
        let mut small = PixelBuffer::empty(1, 1, ColorMode::Rgba);
        small.data = vec![9, 9, 9, 255];
        let mut cel = Cel::image(L, F, small);
        cel.position = (3, 0);
        c.insert(cel);
        let mut cmd = TransformCel::new(L, F, Orientation::FlipHorizontal);
        cmd.apply(&mut s, &mut c).unwrap();
        let out = c.get(L, F).unwrap();
        assert_eq!(out.position, (0, 0));
        let d = data(&c);
        assert_eq!(d.len(), 64);
        assert_eq!(&d[0..4], &[9, 9, 9, 255], "pixel moved to the left edge");
        cmd.revert(&mut s, &mut c);
        assert_eq!(c.get(L, F).unwrap().position, (3, 0));
    }

    #[test]
    fn missing_cel_and_empty_region_error() {
        let (mut s, mut c) = doc(2, 2, &[0; 16]);
        assert!(matches!(
            TransformCel::new(LayerId::new(4), F, Orientation::Rotate180).apply(&mut s, &mut c),
            Err(CommandError::MissingCel { .. })
        ));
        assert_eq!(
            TransformCel::new(L, F, Orientation::Rotate180)
                .within(Rect::new(10, 10, 2, 2))
                .apply(&mut s, &mut c),
            Err(CommandError::EmptyRegion)
        );
    }
}
