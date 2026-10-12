//! `SetLayerEditable` command — lock or unlock a layer.
//!
//! Locking is metadata (spec §3.2 `editable`, Aseprite's layer flag `0x2`);
//! the drawing entry points refuse to write into a locked layer, see
//! [`crate::Sprite::layer_locked`]. Nothing repaints, so the command reports
//! [`DirtyRegion::None`].

use crate::document::{CelMap, LayerId, Sprite};

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// Set the named layer's `editable` flag.
#[derive(Debug, Clone)]
pub struct SetLayerEditable {
    layer: LayerId,
    editable: bool,
    /// Prior flag, captured on `apply` for `revert`.
    prev: Option<bool>,
}

impl SetLayerEditable {
    /// Lock (`editable = false`) or unlock (`true`) `layer`.
    pub fn new(layer: LayerId, editable: bool) -> Self {
        Self {
            layer,
            editable,
            prev: None,
        }
    }
}

impl Command for SetLayerEditable {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let layer = doc
            .layer_mut(self.layer)
            .ok_or(CommandError::UnknownLayer(self.layer.0))?;
        self.prev = Some(std::mem::replace(&mut layer.editable, self.editable));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some(prev) = self.prev.take() else {
            return;
        };
        if let Some(layer) = doc.layer_mut(self.layer) {
            layer.editable = prev;
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Layer, Sprite};

    fn doc() -> (Sprite, CelMap) {
        let mut child = Layer::image(LayerId::new(1), "child");
        child.parent = Some(LayerId::new(0));
        let sprite = Sprite::builder(8, 8)
            .add_layer(Layer::group(LayerId::new(0), "group"))
            .add_layer(child)
            .build()
            .expect("sprite builds");
        (sprite, CelMap::new())
    }

    #[test]
    fn apply_locks_and_revert_unlocks() {
        let (mut s, mut c) = doc();
        let mut cmd = SetLayerEditable::new(LayerId::new(1), false);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert!(s.layer_locked(LayerId::new(1)));
        cmd.revert(&mut s, &mut c);
        assert!(!s.layer_locked(LayerId::new(1)));
    }

    #[test]
    fn locking_a_group_locks_its_children() {
        let (mut s, mut c) = doc();
        SetLayerEditable::new(LayerId::new(0), false)
            .apply(&mut s, &mut c)
            .expect("apply");
        assert!(s.layer_locked(LayerId::new(1)));
        assert!(s.layer(LayerId::new(1)).expect("child").editable);
    }

    #[test]
    fn apply_unknown_layer_errs() {
        let (mut s, mut c) = doc();
        let err = SetLayerEditable::new(LayerId::new(9), false)
            .apply(&mut s, &mut c)
            .unwrap_err();
        assert!(matches!(err, CommandError::UnknownLayer(9)));
    }

    #[test]
    fn dirty_region_is_none() {
        assert!(
            SetLayerEditable::new(LayerId::new(0), true)
                .dirty_region()
                .is_none()
        );
    }
}
