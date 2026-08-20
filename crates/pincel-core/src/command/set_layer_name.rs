//! `SetLayerName` command — rename a layer.
//!
//! The name is metadata only (it does not affect `compose()`), so the
//! command reports [`DirtyRegion::None`] — no repaint is needed. The
//! prior name is captured on `apply` so `revert` restores it exactly.

use crate::document::{CelMap, LayerId, Sprite};

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// Rename the named layer to `name`.
#[derive(Debug, Clone)]
pub struct SetLayerName {
    layer: LayerId,
    name: String,
    /// Prior name, captured on `apply` for `revert`.
    prev: Option<String>,
}

impl SetLayerName {
    pub fn new(layer: LayerId, name: impl Into<String>) -> Self {
        Self {
            layer,
            name: name.into(),
            prev: None,
        }
    }
}

impl Command for SetLayerName {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let layer = doc
            .layer_mut(self.layer)
            .ok_or(CommandError::UnknownLayer(self.layer.0))?;
        // Swap the new name in and keep the old one for revert.
        self.prev = Some(std::mem::replace(&mut layer.name, self.name.clone()));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some(prev) = self.prev.take() else {
            return;
        };
        if let Some(layer) = doc.layer_mut(self.layer) {
            layer.name = prev;
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        // A rename changes no pixels.
        DirtyRegion::None
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
    fn apply_renames_and_revert_restores() {
        let (mut s, mut c) = doc_one_layer();
        let mut cmd = SetLayerName::new(LayerId::new(0), "ground");
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(s.layer(LayerId::new(0)).unwrap().name, "ground");
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.layer(LayerId::new(0)).unwrap().name, "bg");
    }

    #[test]
    fn unknown_layer_is_rejected() {
        let (mut s, mut c) = doc_one_layer();
        let mut cmd = SetLayerName::new(LayerId::new(9), "x");
        assert_eq!(
            cmd.apply(&mut s, &mut c),
            Err(CommandError::UnknownLayer(9))
        );
    }

    #[test]
    fn dirty_region_is_none() {
        assert!(matches!(
            SetLayerName::new(LayerId::new(0), "x").dirty_region(),
            DirtyRegion::None
        ));
    }
}
