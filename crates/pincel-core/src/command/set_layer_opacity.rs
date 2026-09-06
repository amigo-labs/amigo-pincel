//! `SetLayerOpacity` command — set a layer's opacity (0–255).
//!
//! Layer opacity is folded into every cel of the layer by `compose()`, so
//! the change is `Canvas`-level. Consecutive edits to the same layer merge
//! into one undo entry (an opacity slider emits one command per tick).

use crate::document::{CelMap, LayerId, Sprite};

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// Set the named layer's `opacity`.
#[derive(Debug, Clone)]
pub struct SetLayerOpacity {
    layer: LayerId,
    opacity: u8,
    /// Prior value, captured on `apply` for `revert`.
    prev: Option<u8>,
}

impl SetLayerOpacity {
    pub fn new(layer: LayerId, opacity: u8) -> Self {
        Self {
            layer,
            opacity,
            prev: None,
        }
    }
}

impl Command for SetLayerOpacity {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let layer = doc
            .layer_mut(self.layer)
            .ok_or(CommandError::UnknownLayer(self.layer.0))?;
        if self.prev.is_none() {
            self.prev = Some(layer.opacity);
        }
        layer.opacity = self.opacity;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some(prev) = self.prev.take() else {
            return;
        };
        if let Some(layer) = doc.layer_mut(self.layer) {
            layer.opacity = prev;
        }
    }

    fn merge(&mut self, next: &Self) -> bool {
        if next.layer != self.layer {
            return false;
        }
        self.opacity = next.opacity;
        true
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::Canvas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Bus;
    use crate::document::Layer;

    fn doc() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(4, 4)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .build()
            .expect("sprite builds");
        (sprite, CelMap::new())
    }

    fn opacity(s: &Sprite) -> u8 {
        s.layer(LayerId::new(0)).unwrap().opacity
    }

    #[test]
    fn apply_sets_and_revert_restores_opacity() {
        let (mut s, mut c) = doc();
        assert_eq!(opacity(&s), 255);
        let mut cmd = SetLayerOpacity::new(LayerId::new(0), 90);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(opacity(&s), 90);
        cmd.revert(&mut s, &mut c);
        assert_eq!(opacity(&s), 255);
    }

    #[test]
    fn unknown_layer_is_rejected() {
        let (mut s, mut c) = doc();
        let mut cmd = SetLayerOpacity::new(LayerId::new(7), 1);
        assert_eq!(
            cmd.apply(&mut s, &mut c),
            Err(CommandError::UnknownLayer(7))
        );
    }

    #[test]
    fn consecutive_edits_merge_into_one_undo_entry_restoring_the_first_prior() {
        let (mut s, mut c) = doc();
        let mut bus = Bus::new();
        for o in [200u8, 150, 100] {
            bus.execute(
                SetLayerOpacity::new(LayerId::new(0), o).into(),
                &mut s,
                &mut c,
            )
            .expect("execute");
        }
        assert_eq!(bus.undo_depth(), 1);
        assert_eq!(opacity(&s), 100);
        assert!(bus.undo(&mut s, &mut c));
        assert_eq!(opacity(&s), 255);
    }

    #[test]
    fn edits_to_different_layers_do_not_merge() {
        let (mut s, _c) = doc();
        s.layers.push(Layer::image(LayerId::new(1), "fg"));
        let mut a = SetLayerOpacity::new(LayerId::new(0), 10);
        let b = SetLayerOpacity::new(LayerId::new(1), 20);
        assert!(!a.merge(&b));
    }
}
