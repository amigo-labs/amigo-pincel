//! `ScaleImage` command — resample the whole sprite to a new size.
//!
//! Every image cel is normalised to canvas size and resampled with the
//! chosen filter (nearest for crisp pixel art, bilinear / bicubic for
//! smooth results); slice keys scale proportionally; the selection is
//! cleared. Tilemap layers are rejected (their grid cannot be resampled).

use crate::document::{CelMap, LayerKind, Slice, Sprite};
use crate::geometry::Rect;

use super::Command;
use super::error::CommandError;
use super::transform_support::{Interpolation, cel_to_canvas, raster_to_cel, rgba_buffer};

/// Scale the sprite to `width × height`.
#[derive(Debug, Clone)]
pub struct ScaleImage {
    width: u32,
    height: u32,
    filter: Interpolation,
    prior: Option<Prior>,
}

#[derive(Debug, Clone)]
struct Prior {
    width: u32,
    height: u32,
    cels: CelMap,
    slices: Vec<Slice>,
    selection: Option<Rect>,
    selection_mask: Option<crate::selection::SelectionMask>,
}

impl ScaleImage {
    pub fn new(width: u32, height: u32, filter: Interpolation) -> Self {
        Self {
            width,
            height,
            filter,
            prior: None,
        }
    }
}

impl Command for ScaleImage {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        if self.width == 0 || self.height == 0 {
            return Err(CommandError::EmptyRegion);
        }
        if let Some(l) = doc
            .layers
            .iter()
            .find(|l| matches!(l.kind, LayerKind::Tilemap { .. }))
        {
            return Err(CommandError::UnsupportedLayerKind(l.id.0));
        }
        let (w, h) = (doc.width, doc.height);
        let mut new_cels = CelMap::new();
        for (&(layer, frame), cel) in cels.iter() {
            let Some(buffer) = rgba_buffer(cel) else {
                return Err(CommandError::NotAnImageCel { layer, frame });
            };
            let raster =
                cel_to_canvas(cel, buffer, w, h).resample(self.width, self.height, self.filter);
            let mut c = raster_to_cel(layer, frame, raster);
            c.opacity = cel.opacity;
            new_cels.insert(c);
        }
        let sx = f64::from(self.width) / f64::from(w);
        let sy = f64::from(self.height) / f64::from(h);
        let scale_rect = |r: Rect| {
            let x0 = (f64::from(r.x) * sx).round();
            let y0 = (f64::from(r.y) * sy).round();
            let x1 = ((f64::from(r.x) + f64::from(r.width)) * sx).round();
            let y1 = ((f64::from(r.y) + f64::from(r.height)) * sy).round();
            Rect::new(
                x0 as i32,
                y0 as i32,
                ((x1 - x0).max(1.0)) as u32,
                ((y1 - y0).max(1.0)) as u32,
            )
        };
        let new_slices: Vec<Slice> = doc
            .slices
            .iter()
            .map(|s| {
                let mut s = s.clone();
                for k in &mut s.keys {
                    k.bounds = scale_rect(k.bounds);
                    k.center = k.center.map(scale_rect);
                    k.pivot = k.pivot.map(|p| {
                        (
                            (f64::from(p.0) * sx).round() as i32,
                            (f64::from(p.1) * sy).round() as i32,
                        )
                    });
                }
                s
            })
            .collect();
        self.prior = Some(Prior {
            width: w,
            height: h,
            cels: std::mem::replace(cels, new_cels),
            slices: std::mem::replace(&mut doc.slices, new_slices),
            selection: doc.selection.take(),
            selection_mask: doc.selection_mask.take(),
        });
        doc.width = self.width;
        doc.height = self.height;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap) {
        let Some(p) = self.prior.take() else {
            return;
        };
        doc.width = p.width;
        doc.height = p.height;
        *cels = p.cels;
        doc.slices = p.slices;
        doc.selection = p.selection;
        doc.selection_mask = p.selection_mask;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Bus;
    use crate::document::{
        Cel, CelData, ColorMode, Frame, FrameIndex, Layer, LayerId, PixelBuffer,
    };

    const L: LayerId = LayerId::new(0);
    const F: FrameIndex = FrameIndex::new(0);

    fn doc() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(2, 1)
            .add_layer(Layer::image(L, "a"))
            .add_frame(Frame::new(100))
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        let mut buf = PixelBuffer::empty(2, 1, ColorMode::Rgba);
        buf.data = vec![255, 0, 0, 255, 0, 0, 255, 255];
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
    fn nearest_double_then_half_is_identity() {
        let (mut s, mut c) = doc();
        let before = data(&c);
        ScaleImage::new(4, 2, Interpolation::Nearest)
            .apply(&mut s, &mut c)
            .unwrap();
        assert_eq!((s.width, s.height), (4, 2));
        assert_eq!(data(&c).len(), 32);
        ScaleImage::new(2, 1, Interpolation::Nearest)
            .apply(&mut s, &mut c)
            .unwrap();
        assert_eq!(data(&c), before);
    }

    #[test]
    fn undo_restores_size_and_pixels() {
        let (mut s, mut c) = doc();
        let before = data(&c);
        let mut bus = Bus::new();
        bus.execute(
            ScaleImage::new(6, 3, Interpolation::Bicubic).into(),
            &mut s,
            &mut c,
        )
        .unwrap();
        assert_eq!((s.width, s.height), (6, 3));
        assert!(bus.undo(&mut s, &mut c));
        assert_eq!((s.width, s.height), (2, 1));
        assert_eq!(data(&c), before);
    }

    #[test]
    fn zero_size_and_tilemaps_are_rejected() {
        let (mut s, mut c) = doc();
        assert_eq!(
            ScaleImage::new(0, 4, Interpolation::Nearest).apply(&mut s, &mut c),
            Err(CommandError::EmptyRegion)
        );
        s.layers.push(Layer::tilemap(
            LayerId::new(1),
            "t",
            crate::document::TilesetId::new(0),
        ));
        assert_eq!(
            ScaleImage::new(4, 4, Interpolation::Nearest).apply(&mut s, &mut c),
            Err(CommandError::UnsupportedLayerKind(1))
        );
    }
}
