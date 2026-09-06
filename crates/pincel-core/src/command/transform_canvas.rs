//! `TransformCanvas` command — flip or rotate the whole sprite: every
//! image cel on every frame, the slice keys, and (for quarter turns) the
//! canvas dimensions.
//!
//! Tilemap cels cannot be reoriented without rewriting their tile grid,
//! so a document with a tilemap layer is rejected. The selection is
//! cleared. `revert` restores the prior cel map, slices and dimensions
//! wholesale.

use crate::document::{CelMap, LayerKind, Slice, Sprite};
use crate::geometry::Rect;

use super::Command;
use super::error::CommandError;
use super::transform_support::{
    Orientation, cel_to_canvas, orient_point, orient_rect, raster_to_cel, rgba_buffer,
};

/// Reorient the entire sprite.
#[derive(Debug, Clone)]
pub struct TransformCanvas {
    op: Orientation,
    prior: Option<Prior>,
}

#[derive(Debug, Clone)]
struct Prior {
    width: u32,
    height: u32,
    cels: CelMap,
    slices: Vec<Slice>,
    selection: Option<Rect>,
}

impl TransformCanvas {
    pub fn new(op: Orientation) -> Self {
        Self { op, prior: None }
    }
}

impl Command for TransformCanvas {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
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
            let raster = cel_to_canvas(cel, buffer, w, h).oriented(self.op);
            let mut out = raster_to_cel(layer, frame, raster);
            out.opacity = cel.opacity;
            new_cels.insert(out);
        }
        let new_slices: Vec<Slice> = doc
            .slices
            .iter()
            .map(|s| {
                let mut s = s.clone();
                for k in &mut s.keys {
                    k.bounds = orient_rect(k.bounds, self.op, w, h);
                    k.center = k.center.map(|c| orient_rect(c, self.op, w, h));
                    k.pivot = k.pivot.map(|p| orient_point(p, self.op, w, h));
                }
                s
            })
            .collect();

        let prior = Prior {
            width: w,
            height: h,
            cels: std::mem::replace(cels, new_cels),
            slices: std::mem::replace(&mut doc.slices, new_slices),
            selection: doc.selection.take(),
        };
        if self.op.swaps_axes() {
            doc.width = h;
            doc.height = w;
        }
        self.prior = Some(prior);
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Bus;
    use crate::document::{
        Cel, CelData, ColorMode, Frame, FrameIndex, Layer, LayerId, PixelBuffer, SliceId, SliceKey,
    };

    const L: LayerId = LayerId::new(0);
    const F: FrameIndex = FrameIndex::new(0);

    fn doc() -> (Sprite, CelMap) {
        let slice = Slice {
            id: SliceId::new(0),
            name: "s".into(),
            color: crate::document::Rgba::BLACK,
            keys: vec![SliceKey {
                frame: F,
                bounds: Rect::new(0, 0, 1, 1),
                center: None,
                pivot: Some((0, 0)),
            }],
        };
        let sprite = Sprite::builder(3, 2)
            .add_layer(Layer::image(L, "a"))
            .add_frame(Frame::new(100))
            .add_slice(slice)
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        let mut buf = PixelBuffer::empty(3, 2, ColorMode::Rgba);
        // Row 0: 1 2 3, row 1: 4 5 6 (grey levels, opaque).
        buf.data = (1u8..=6).flat_map(|v| [v, v, v, 255]).collect();
        cels.insert(Cel::image(L, F, buf));
        (sprite, cels)
    }

    fn greys(cels: &CelMap) -> Vec<u8> {
        match &cels.get(L, F).unwrap().data {
            CelData::Image(b) => b.data.iter().step_by(4).copied().collect(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn rotate_cw_swaps_canvas_dims_and_maps_pixels_and_slices() {
        let (mut s, mut c) = doc();
        s.set_selection(Rect::new(0, 0, 1, 1));
        let mut bus = Bus::new();
        bus.execute(
            TransformCanvas::new(Orientation::Rotate90Cw).into(),
            &mut s,
            &mut c,
        )
        .unwrap();
        assert_eq!((s.width, s.height), (2, 3));
        // CW: new row 0 = old column 0 bottom-up → 4 1; row 1 → 5 2; row 2 → 6 3.
        assert_eq!(greys(&c), vec![4, 1, 5, 2, 6, 3]);
        assert_eq!(s.slices[0].keys[0].bounds, Rect::new(1, 0, 1, 1));
        assert_eq!(s.slices[0].keys[0].pivot, Some((1, 0)));
        assert!(!s.has_selection());
        let buf = c.get(L, F).unwrap();
        assert_eq!(buf.position, (0, 0));
        assert!(bus.undo(&mut s, &mut c));
        assert_eq!((s.width, s.height), (3, 2));
        assert_eq!(greys(&c), vec![1, 2, 3, 4, 5, 6]);
        assert!(s.has_selection());
    }

    #[test]
    fn flip_vertical_keeps_dims() {
        let (mut s, mut c) = doc();
        TransformCanvas::new(Orientation::FlipVertical)
            .apply(&mut s, &mut c)
            .unwrap();
        assert_eq!((s.width, s.height), (3, 2));
        assert_eq!(greys(&c), vec![4, 5, 6, 1, 2, 3]);
    }

    #[test]
    fn tilemap_layer_is_rejected() {
        let (mut s, mut c) = doc();
        s.layers.push(Layer::tilemap(
            LayerId::new(1),
            "t",
            crate::document::TilesetId::new(0),
        ));
        assert_eq!(
            TransformCanvas::new(Orientation::Rotate180).apply(&mut s, &mut c),
            Err(CommandError::UnsupportedLayerKind(1))
        );
    }
}
