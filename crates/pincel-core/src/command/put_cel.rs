//! `PutCel` command — insert a whole cel at its `(layer, frame)` key,
//! replacing any cel already there, undoably.
//!
//! The paint paths of the wasm layer seed an empty cel outside the undo
//! bus (`ensure_paint_cel`); hosts that need an exact undo — the agent
//! tools in `pincel-agent`, where one undo must restore the document
//! byte for byte — insert the cel with this command instead, usually in a
//! [`super::CommandBatch`] with the paint command that needs it.

use crate::document::{Cel, CelMap, Sprite};

use super::Command;
use super::error::CommandError;

/// Put `cel` into the cel map; `revert` restores whatever was there.
#[derive(Debug, Clone)]
pub struct PutCel {
    /// Before `apply`: the incoming cel. After `apply`: `None`.
    cel: Option<Cel>,
    /// After `apply`: the key written and the cel it displaced.
    applied: Option<Applied>,
}

#[derive(Debug, Clone)]
struct Applied {
    key: (crate::document::LayerId, crate::document::FrameIndex),
    prior: Option<Cel>,
}

impl PutCel {
    /// Insert `cel` at `(cel.layer, cel.frame)`.
    pub fn new(cel: Cel) -> Self {
        Self {
            cel: Some(cel),
            applied: None,
        }
    }
}

impl Command for PutCel {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let Some(cel) = self.cel.take() else {
            return Ok(());
        };
        if doc.layer(cel.layer).is_none() {
            let layer = cel.layer;
            self.cel = Some(cel);
            return Err(CommandError::UnknownLayer(layer.0));
        }
        let key = (cel.layer, cel.frame);
        let prior = cels.insert(cel);
        self.applied = Some(Applied { key, prior });
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        let Some(Applied { key, prior }) = self.applied.take() else {
            return;
        };
        self.cel = match prior {
            Some(prior) => cels.insert(prior),
            None => cels.remove(key.0, key.1),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{ColorMode, FrameIndex, Layer, LayerId, PixelBuffer};

    fn doc() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::image(LayerId::new(0), "L"))
            .build()
            .expect("sprite builds");
        (sprite, CelMap::new())
    }

    fn cel(fill: u8) -> Cel {
        let mut buf = PixelBuffer::empty(2, 2, ColorMode::Rgba);
        buf.data.fill(fill);
        Cel::image(LayerId::new(0), FrameIndex::new(0), buf)
    }

    #[test]
    fn apply_inserts_and_revert_removes_new_cel() {
        let (mut sprite, mut cels) = doc();
        let mut cmd = PutCel::new(cel(7));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(cels.get(LayerId::new(0), FrameIndex::new(0)), Some(&cel(7)));
        cmd.revert(&mut sprite, &mut cels);
        assert!(cels.is_empty());
        cmd.apply(&mut sprite, &mut cels).expect("redo");
        assert_eq!(cels.len(), 1);
    }

    #[test]
    fn apply_replaces_and_revert_restores_prior_cel() {
        let (mut sprite, mut cels) = doc();
        cels.insert(cel(1));
        let mut cmd = PutCel::new(cel(2));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(cels.get(LayerId::new(0), FrameIndex::new(0)), Some(&cel(2)));
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(cels.get(LayerId::new(0), FrameIndex::new(0)), Some(&cel(1)));
    }

    #[test]
    fn apply_unknown_layer_returns_err() {
        let (mut sprite, mut cels) = doc();
        let mut c = cel(0);
        c.layer = LayerId::new(5);
        let mut cmd = PutCel::new(c);
        assert!(cmd.apply(&mut sprite, &mut cels).is_err());
        assert!(cels.is_empty());
    }
}
