//! `AddTilemapLayer` command — insert a tilemap layer and seed its
//! per-frame tilemap cels as one undoable unit.
//!
//! Plain [`AddLayer`](super::AddLayer) does not manage cels, so seeding
//! tilemap cels by hand after running it leaves orphan entries in
//! [`CelMap`] if the layer is later undone (the layer leaves, the cels
//! stay). This command bundles both sides so revert removes the cels
//! along with the layer.

use crate::document::{Cel, CelMap, Layer, Sprite};

use super::Command;
use super::error::CommandError;

/// Insert a tilemap layer on top of the stack and seed the supplied
/// per-frame cels under one undo step.
#[derive(Debug, Clone)]
pub struct AddTilemapLayer {
    layer: Option<Layer>,
    cels: Vec<Cel>,
    /// Index where the layer was inserted; `Some` after `apply`,
    /// consumed on `revert`.
    inserted_index: Option<usize>,
    /// Cels that previously occupied any of the seeded `(layer, frame)`
    /// slots, captured on `apply` so `revert` can restore them. Empty
    /// when the slots were unoccupied (the common case for a freshly
    /// added tilemap layer).
    displaced: Vec<Cel>,
}

impl AddTilemapLayer {
    /// Build the command. `cels` are inserted in order after the layer
    /// lands; any of them whose `(layer, frame)` collides with an
    /// existing entry will displace that entry, which `revert` then
    /// restores.
    pub fn new(layer: Layer, cels: Vec<Cel>) -> Self {
        Self {
            layer: Some(layer),
            cels,
            inserted_index: None,
            displaced: Vec::new(),
        }
    }
}

impl Command for AddTilemapLayer {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let layer = self
            .layer
            .as_ref()
            .expect("AddTilemapLayer applied without a layer payload");
        if doc.layers.iter().any(|l| l.id == layer.id) {
            return Err(CommandError::DuplicateLayerId(layer.id.0));
        }
        let layer = self.layer.take().expect("layer payload checked above");
        let target = doc.layers.len();
        doc.layers.insert(target, layer);
        self.inserted_index = Some(target);
        // Insert seeded cels, capturing any displaced entries so revert
        // restores the pre-command state exactly.
        self.displaced.clear();
        for cel in &self.cels {
            if let Some(prev) = cels.insert(cel.clone()) {
                self.displaced.push(prev);
            }
        }
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap) {
        let Some(index) = self.inserted_index.take() else {
            return;
        };
        // Remove seeded cels in reverse order. Then restore displaced
        // entries (also reverse, so first-displaced lands first into a
        // now-empty slot before later ones).
        for cel in self.cels.iter().rev() {
            cels.remove(cel.layer, cel.frame);
        }
        for cel in self.displaced.drain(..).rev() {
            cels.insert(cel);
        }
        if index < doc.layers.len() {
            let layer = doc.layers.remove(index);
            self.layer = Some(layer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{
        CelData, ColorMode, FrameIndex, LayerId, PixelBuffer, TileImage, TileRef, Tileset,
        TilesetId,
    };

    fn fixture() -> (Sprite, CelMap, Layer, Vec<Cel>) {
        let mut sprite = Sprite::builder(4, 4)
            .add_frame(crate::document::Frame::new(100))
            .build()
            .expect("sprite builds");
        let ts = Tileset::new(TilesetId::new(0), "t", (2, 2));
        sprite.tilesets.push(ts);
        let layer = Layer::tilemap(LayerId::new(7), "tiles", TilesetId::new(0));
        let cel = Cel {
            layer: LayerId::new(7),
            frame: FrameIndex::new(0),
            position: (0, 0),
            opacity: 255,
            data: CelData::Tilemap {
                grid_w: 2,
                grid_h: 2,
                tiles: vec![TileRef::EMPTY; 4],
            },
        };
        (sprite, CelMap::new(), layer, vec![cel])
    }

    #[test]
    fn apply_inserts_layer_and_cels_together() {
        let (mut sprite, mut cels, layer, seeded) = fixture();
        let mut cmd = AddTilemapLayer::new(layer, seeded);
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(sprite.layers.len(), 1);
        assert_eq!(sprite.layers[0].id, LayerId::new(7));
        assert!(cels.get(LayerId::new(7), FrameIndex::new(0)).is_some());
    }

    #[test]
    fn revert_removes_layer_and_seeded_cels() {
        let (mut sprite, mut cels, layer, seeded) = fixture();
        let mut cmd = AddTilemapLayer::new(layer, seeded);
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        cmd.revert(&mut sprite, &mut cels);
        assert!(sprite.layers.is_empty());
        assert!(
            cels.get(LayerId::new(7), FrameIndex::new(0)).is_none(),
            "revert must drop seeded cels alongside the layer"
        );
    }

    #[test]
    fn revert_restores_displaced_cel() {
        let (mut sprite, mut cels, layer, seeded) = fixture();
        // Stash a pre-existing cel at the same (layer, frame) slot so
        // we can verify the revert path restores it.
        let preexisting = Cel {
            layer: LayerId::new(7),
            frame: FrameIndex::new(0),
            position: (0, 0),
            opacity: 200,
            data: CelData::Image(PixelBuffer::empty(4, 4, ColorMode::Rgba)),
        };
        cels.insert(preexisting.clone());
        // Pad the tileset with one stored tile so identity comparisons
        // exercise the actual round-trip rather than the trivial empty
        // case.
        sprite.tilesets[0].tiles.push(TileImage {
            pixels: PixelBuffer::empty(2, 2, ColorMode::Rgba),
        });
        let mut cmd = AddTilemapLayer::new(layer, seeded);
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        cmd.revert(&mut sprite, &mut cels);
        let restored = cels
            .get(LayerId::new(7), FrameIndex::new(0))
            .expect("displaced cel restored");
        assert_eq!(restored.opacity, 200);
        assert!(matches!(restored.data, CelData::Image(_)));
    }

    #[test]
    fn duplicate_layer_id_is_rejected_without_touching_cels() {
        let (mut sprite, mut cels, layer, seeded) = fixture();
        sprite
            .layers
            .push(Layer::tilemap(LayerId::new(7), "existing", TilesetId::new(0)));
        let mut cmd = AddTilemapLayer::new(layer, seeded);
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::DuplicateLayerId(7))
        );
        assert!(
            cels.get(LayerId::new(7), FrameIndex::new(0)).is_none(),
            "no cels should be seeded when the layer insert fails"
        );
    }
}
