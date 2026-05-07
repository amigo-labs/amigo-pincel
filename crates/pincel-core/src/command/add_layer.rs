//! `AddLayer` command — insert a layer into the sprite's layer stack.
//!
//! No cels are added or removed; cels associated with this layer must be
//! managed by other commands (M3+).

use crate::document::{CelMap, Layer, Sprite};

use super::Command;
use super::error::CommandError;

/// Insert a layer at a chosen z-order position. `None` means "on top".
#[derive(Debug, Clone)]
pub struct AddLayer {
    layer: Option<Layer>,
    insert_at: Option<usize>,
    /// `Some(index)` after a successful `apply`; consumed on `revert`.
    inserted_index: Option<usize>,
}

impl AddLayer {
    /// Add `layer` on top of the existing stack.
    pub fn on_top(layer: Layer) -> Self {
        Self {
            layer: Some(layer),
            insert_at: None,
            inserted_index: None,
        }
    }

    /// Add `layer` at the given z-order index (`0` is the bottom).
    pub fn at(layer: Layer, index: usize) -> Self {
        Self {
            layer: Some(layer),
            insert_at: Some(index),
            inserted_index: None,
        }
    }
}

impl Command for AddLayer {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let layer = self
            .layer
            .as_ref()
            .expect("AddLayer applied without a layer payload");

        if doc.layers.iter().any(|l| l.id == layer.id) {
            return Err(CommandError::DuplicateLayerId(layer.id.0));
        }

        let target = self.insert_at.unwrap_or(doc.layers.len());
        let target = target.min(doc.layers.len());
        let layer = self.layer.take().expect("layer payload checked above");
        doc.layers.insert(target, layer);
        self.inserted_index = Some(target);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some(index) = self.inserted_index.take() else {
            return;
        };
        if index < doc.layers.len() {
            let layer = doc.layers.remove(index);
            self.layer = Some(layer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{LayerId, Sprite};

    fn empty_doc() -> (Sprite, CelMap) {
        (
            Sprite::builder(8, 8).build().expect("sprite builds"),
            CelMap::new(),
        )
    }

    #[test]
    fn apply_appends_layer_when_no_index_given() {
        let (mut sprite, mut cels) = empty_doc();
        let mut cmd = AddLayer::on_top(Layer::image(LayerId::new(1), "bg"));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(sprite.layers.len(), 1);
        assert_eq!(sprite.layers[0].id, LayerId::new(1));
    }

    #[test]
    fn apply_inserts_layer_at_specified_index() {
        let (mut sprite, mut cels) = empty_doc();
        AddLayer::on_top(Layer::image(LayerId::new(0), "bg"))
            .apply(&mut sprite, &mut cels)
            .expect("apply 0");
        AddLayer::on_top(Layer::image(LayerId::new(1), "fg"))
            .apply(&mut sprite, &mut cels)
            .expect("apply 1");

        let mut cmd = AddLayer::at(Layer::image(LayerId::new(2), "mid"), 1);
        cmd.apply(&mut sprite, &mut cels).expect("apply mid");
        assert_eq!(
            sprite.layers.iter().map(|l| l.id).collect::<Vec<_>>(),
            vec![LayerId::new(0), LayerId::new(2), LayerId::new(1)],
        );
    }

    #[test]
    fn revert_removes_inserted_layer() {
        let (mut sprite, mut cels) = empty_doc();
        let mut cmd = AddLayer::on_top(Layer::image(LayerId::new(7), "ink"));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(sprite.layers.len(), 1);
        cmd.revert(&mut sprite, &mut cels);
        assert!(sprite.layers.is_empty());
    }

    #[test]
    fn duplicate_id_is_rejected() {
        let (mut sprite, mut cels) = empty_doc();
        AddLayer::on_top(Layer::image(LayerId::new(1), "a"))
            .apply(&mut sprite, &mut cels)
            .expect("apply 1");
        let mut dup = AddLayer::on_top(Layer::image(LayerId::new(1), "b"));
        assert_eq!(
            dup.apply(&mut sprite, &mut cels),
            Err(CommandError::DuplicateLayerId(1))
        );
        assert_eq!(sprite.layers.len(), 1);
    }
}
