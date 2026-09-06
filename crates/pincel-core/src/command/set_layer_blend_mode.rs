//! `SetLayerBlendMode` command — change how a layer composites over the
//! layers below it (spec §3.2).

use crate::document::{BlendMode, CelMap, LayerId, Sprite};

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// Set the named layer's `blend_mode`.
#[derive(Debug, Clone)]
pub struct SetLayerBlendMode {
    layer: LayerId,
    mode: BlendMode,
    /// Prior value, captured on `apply` for `revert`.
    prev: Option<BlendMode>,
}

impl SetLayerBlendMode {
    pub fn new(layer: LayerId, mode: BlendMode) -> Self {
        Self {
            layer,
            mode,
            prev: None,
        }
    }
}

impl Command for SetLayerBlendMode {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let layer = doc
            .layer_mut(self.layer)
            .ok_or(CommandError::UnknownLayer(self.layer.0))?;
        self.prev = Some(layer.blend_mode);
        layer.blend_mode = self.mode;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some(prev) = self.prev.take() else {
            return;
        };
        if let Some(layer) = doc.layer_mut(self.layer) {
            layer.blend_mode = prev;
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::Canvas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Layer;

    fn doc() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(4, 4)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .build()
            .expect("sprite builds");
        (sprite, CelMap::new())
    }

    #[test]
    fn apply_sets_and_revert_restores_blend_mode() {
        let (mut s, mut c) = doc();
        let mut cmd = SetLayerBlendMode::new(LayerId::new(0), BlendMode::Multiply);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(s.layers[0].blend_mode, BlendMode::Multiply);
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.layers[0].blend_mode, BlendMode::Normal);
    }

    #[test]
    fn unknown_layer_is_rejected() {
        let (mut s, mut c) = doc();
        let mut cmd = SetLayerBlendMode::new(LayerId::new(3), BlendMode::Screen);
        assert_eq!(
            cmd.apply(&mut s, &mut c),
            Err(CommandError::UnknownLayer(3))
        );
    }
}
