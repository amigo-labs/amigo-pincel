//! `SetLayerVisible` command — toggle a layer's visibility flag.
//!
//! Visibility is a per-layer boolean read by `compose()` (hidden layers
//! don't contribute to the composite), so flipping it is a `Canvas`-level
//! change. The command captures the prior value on `apply` so `revert`
//! restores it exactly.

use crate::document::{CelMap, LayerId, Sprite};

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// Set the named layer's `visible` flag to `visible`.
#[derive(Debug, Clone)]
pub struct SetLayerVisible {
    layer: LayerId,
    visible: bool,
    /// Prior `visible` value, captured on `apply` for `revert`.
    prev: Option<bool>,
}

impl SetLayerVisible {
    pub fn new(layer: LayerId, visible: bool) -> Self {
        Self {
            layer,
            visible,
            prev: None,
        }
    }
}

impl Command for SetLayerVisible {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let layer = doc
            .layer_mut(self.layer)
            .ok_or(CommandError::UnknownLayer(self.layer.0))?;
        self.prev = Some(layer.visible);
        layer.visible = self.visible;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some(prev) = self.prev.take() else {
            return;
        };
        if let Some(layer) = doc.layer_mut(self.layer) {
            layer.visible = prev;
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        // Showing / hiding a layer changes the whole composite.
        DirtyRegion::Canvas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Layer, Sprite};

    fn doc_one_layer() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(8, 8)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .build()
            .expect("sprite builds");
        (sprite, CelMap::new())
    }

    #[test]
    fn apply_sets_and_revert_restores_visibility() {
        let (mut s, mut c) = doc_one_layer();
        assert!(s.layer(LayerId::new(0)).unwrap().visible);
        let mut cmd = SetLayerVisible::new(LayerId::new(0), false);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert!(!s.layer(LayerId::new(0)).unwrap().visible);
        cmd.revert(&mut s, &mut c);
        assert!(s.layer(LayerId::new(0)).unwrap().visible);
    }

    #[test]
    fn unknown_layer_is_rejected() {
        let (mut s, mut c) = doc_one_layer();
        let mut cmd = SetLayerVisible::new(LayerId::new(9), false);
        assert_eq!(
            cmd.apply(&mut s, &mut c),
            Err(CommandError::UnknownLayer(9))
        );
    }

    #[test]
    fn dirty_region_is_canvas() {
        assert!(matches!(
            SetLayerVisible::new(LayerId::new(0), false).dirty_region(),
            DirtyRegion::Canvas
        ));
    }
}
