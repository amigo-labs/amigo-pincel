//! `DuplicateLayer` command — insert a copy of an image / tilemap layer
//! (with all its cels) directly above the source.
//!
//! The new layer takes the next free id (computed on first `apply` and
//! reused on redo so undo / redo stay deterministic) and the name
//! `"<source> copy"`. Group layers are rejected: copying a subtree needs
//! id remapping for every descendant, which is Phase 2.

use crate::document::{CelMap, FrameIndex, LayerId, LayerKind, Sprite};

use super::Command;
use super::error::CommandError;

/// Duplicate the layer `source` in place (above it).
#[derive(Debug, Clone)]
pub struct DuplicateLayer {
    source: LayerId,
    /// Chosen on the first `apply`, reused afterwards.
    new_id: Option<LayerId>,
    /// `Some` while applied: the inserted index and the frames whose cels
    /// were copied, for `revert`.
    inserted: Option<(usize, Vec<FrameIndex>)>,
}

impl DuplicateLayer {
    pub fn new(source: LayerId) -> Self {
        Self {
            source,
            new_id: None,
            inserted: None,
        }
    }

    /// Id of the copy, once `apply` has run.
    pub fn new_layer_id(&self) -> Option<LayerId> {
        self.new_id
    }
}

/// Smallest layer id not in use on `doc`.
pub(crate) fn next_layer_id(doc: &Sprite) -> Result<LayerId, CommandError> {
    match doc.layers.iter().map(|l| l.id.0).max() {
        None => Ok(LayerId::new(0)),
        Some(u32::MAX) => Err(CommandError::DuplicateLayerId(u32::MAX)),
        Some(m) => Ok(LayerId::new(m + 1)),
    }
}

impl Command for DuplicateLayer {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let index = doc
            .layers
            .iter()
            .position(|l| l.id == self.source)
            .ok_or(CommandError::UnknownLayer(self.source.0))?;
        if matches!(doc.layers[index].kind, LayerKind::Group) {
            return Err(CommandError::UnsupportedLayerKind(self.source.0));
        }
        let new_id = match self.new_id {
            Some(id) => id,
            None => next_layer_id(doc)?,
        };
        if doc.layers.iter().any(|l| l.id == new_id) {
            return Err(CommandError::DuplicateLayerId(new_id.0));
        }
        let mut copy = doc.layers[index].clone();
        copy.id = new_id;
        copy.name = format!("{} copy", copy.name);
        doc.layers.insert(index + 1, copy);

        let frames: Vec<FrameIndex> = cels
            .iter()
            .filter(|((lid, _), _)| *lid == self.source)
            .map(|((_, f), _)| *f)
            .collect();
        for &f in &frames {
            if let Some(src) = cels.get(self.source, f) {
                let mut cel = src.clone();
                cel.layer = new_id;
                cels.insert(cel);
            }
        }
        self.new_id = Some(new_id);
        self.inserted = Some((index + 1, frames));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap) {
        let (Some((index, frames)), Some(new_id)) = (self.inserted.take(), self.new_id) else {
            return;
        };
        if index < doc.layers.len() && doc.layers[index].id == new_id {
            doc.layers.remove(index);
        } else {
            doc.layers.retain(|l| l.id != new_id);
        }
        for f in frames {
            cels.remove(new_id, f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Bus;
    use crate::document::{Cel, ColorMode, Frame, Layer, PixelBuffer};

    const A: LayerId = LayerId::new(0);
    const B: LayerId = LayerId::new(1);

    fn doc() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::image(A, "a"))
            .add_layer(Layer::image(B, "b"))
            .add_frame(Frame::new(100))
            .add_frame(Frame::new(100))
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        let mut buf = PixelBuffer::empty(2, 2, ColorMode::Rgba);
        buf.data = vec![9; 16];
        cels.insert(Cel::image(A, FrameIndex::new(0), buf.clone()));
        cels.insert(Cel::image(A, FrameIndex::new(1), buf));
        (sprite, cels)
    }

    #[test]
    fn apply_inserts_copy_above_source_with_cels_and_revert_removes_it() {
        let (mut s, mut c) = doc();
        let mut cmd = DuplicateLayer::new(A);
        cmd.apply(&mut s, &mut c).unwrap();
        assert_eq!(cmd.new_layer_id(), Some(LayerId::new(2)));
        assert_eq!(
            s.layers.iter().map(|l| l.id.0).collect::<Vec<_>>(),
            vec![0, 2, 1]
        );
        assert_eq!(s.layers[1].name, "a copy");
        assert_eq!(c.len(), 4);
        assert_eq!(
            c.get(LayerId::new(2), FrameIndex::new(1)).unwrap().data,
            c.get(A, FrameIndex::new(1)).unwrap().data
        );
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.layers.len(), 2);
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn redo_reuses_the_same_id() {
        let (mut s, mut c) = doc();
        let mut bus = Bus::new();
        bus.execute(DuplicateLayer::new(B).into(), &mut s, &mut c)
            .unwrap();
        assert!(bus.undo(&mut s, &mut c));
        assert!(bus.redo(&mut s, &mut c).unwrap());
        assert_eq!(
            s.layers.iter().map(|l| l.id.0).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(s.layers[2].name, "b copy");
    }

    #[test]
    fn group_layer_is_rejected() {
        let (mut s, mut c) = doc();
        s.layers.push(Layer::group(LayerId::new(5), "g"));
        let mut cmd = DuplicateLayer::new(LayerId::new(5));
        assert_eq!(
            cmd.apply(&mut s, &mut c),
            Err(CommandError::UnsupportedLayerKind(5))
        );
    }

    #[test]
    fn unknown_layer_is_rejected() {
        let (mut s, mut c) = doc();
        let mut cmd = DuplicateLayer::new(LayerId::new(9));
        assert_eq!(
            cmd.apply(&mut s, &mut c),
            Err(CommandError::UnknownLayer(9))
        );
    }
}
