//! `FlattenImage` command — replace the whole layer stack with one image
//! layer holding the visible composite of every frame.
//!
//! Each frame is rendered through `compose()` (visible layers, tilemaps
//! included), so the flattened cel matches what the user saw. Hidden
//! layers are dropped, as in Aseprite. `revert` restores the previous
//! layer list and cel map wholesale.

use crate::document::{Cel, CelMap, ColorMode, FrameIndex, Layer, LayerId, PixelBuffer, Sprite};
use crate::render::{ComposeRequest, compose};

use super::Command;
use super::duplicate_layer::next_layer_id;
use super::error::CommandError;

/// Flatten every visible layer into a single image layer.
#[derive(Debug, Clone)]
pub struct FlattenImage {
    /// Chosen on the first `apply`, reused on redo.
    new_id: Option<LayerId>,
    prior: Option<(Vec<Layer>, CelMap)>,
}

impl Default for FlattenImage {
    fn default() -> Self {
        Self::new()
    }
}

impl FlattenImage {
    pub fn new() -> Self {
        Self {
            new_id: None,
            prior: None,
        }
    }

    /// Id of the flattened layer, once `apply` has run.
    pub fn flattened_layer_id(&self) -> Option<LayerId> {
        self.new_id
    }
}

impl Command for FlattenImage {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let new_id = match self.new_id {
            Some(id) => id,
            None => next_layer_id(doc)?,
        };
        let mut new_cels = CelMap::new();
        let mut out = Vec::new();
        for f in 0..doc.frames.len() {
            let frame = FrameIndex::new(f as u32);
            let request = ComposeRequest::full(frame, doc.width, doc.height);
            compose(doc, cels, &request, &mut out)
                .map_err(|e| CommandError::Render(e.to_string()))?;
            let mut buf = PixelBuffer::empty(doc.width, doc.height, ColorMode::Rgba);
            buf.data = std::mem::take(&mut out);
            new_cels.insert(Cel::image(new_id, frame, buf));
        }
        let layers = std::mem::replace(&mut doc.layers, vec![Layer::image(new_id, "Flattened")]);
        let old_cels = std::mem::replace(cels, new_cels);
        self.new_id = Some(new_id);
        self.prior = Some((layers, old_cels));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap) {
        let Some((layers, old_cels)) = self.prior.take() else {
            return;
        };
        doc.layers = layers;
        *cels = old_cels;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Bus;
    use crate::document::{CelData, Frame};

    fn solid(w: u32, h: u32, px: [u8; 4]) -> PixelBuffer {
        let mut b = PixelBuffer::empty(w, h, ColorMode::Rgba);
        b.data = px.repeat((w * h) as usize);
        b
    }

    fn doc() -> (Sprite, CelMap) {
        let mut hidden = Layer::image(LayerId::new(2), "hidden");
        hidden.visible = false;
        let sprite = Sprite::builder(2, 1)
            .add_layer(Layer::image(LayerId::new(0), "a"))
            .add_layer(Layer::image(LayerId::new(1), "b"))
            .add_layer(hidden)
            .add_frame(Frame::new(100))
            .add_frame(Frame::new(100))
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(2, 1, [255, 0, 0, 255]),
        ));
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            solid(1, 1, [0, 255, 0, 255]),
        ));
        cels.insert(Cel::image(
            LayerId::new(2),
            FrameIndex::new(0),
            solid(2, 1, [0, 0, 255, 255]),
        ));
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(1),
            solid(2, 1, [7, 7, 7, 255]),
        ));
        (sprite, cels)
    }

    fn data(cels: &CelMap, layer: LayerId, f: u32) -> Vec<u8> {
        match &cels.get(layer, FrameIndex::new(f)).unwrap().data {
            CelData::Image(b) => b.data.clone(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn apply_leaves_one_layer_with_the_visible_composite_per_frame() {
        let (mut s, mut c) = doc();
        let mut cmd = FlattenImage::new();
        cmd.apply(&mut s, &mut c).unwrap();
        let id = cmd.flattened_layer_id().unwrap();
        assert_eq!(id, LayerId::new(3));
        assert_eq!(s.layers.len(), 1);
        assert_eq!(s.layers[0].name, "Flattened");
        assert_eq!(c.len(), 2);
        assert_eq!(data(&c, id, 0), vec![0, 255, 0, 255, 255, 0, 0, 255]);
        assert_eq!(data(&c, id, 1), vec![7, 7, 7, 255, 7, 7, 7, 255]);
    }

    #[test]
    fn undo_restores_layers_and_cels_exactly_and_redo_reflattens() {
        let (mut s, mut c) = doc();
        let (s0, c0) = (s.clone(), c.clone());
        let mut bus = Bus::new();
        bus.execute(FlattenImage::new().into(), &mut s, &mut c)
            .unwrap();
        assert!(bus.undo(&mut s, &mut c));
        assert_eq!(s, s0);
        assert_eq!(c, c0);
        assert!(bus.redo(&mut s, &mut c).unwrap());
        assert_eq!(s.layers.len(), 1);
        assert_eq!(s.layers[0].id, LayerId::new(3));
    }
}
