//! `RemoveLayer` command — remove a layer (and, for a group, its whole
//! contiguous subtree) together with every cel those layers own, as one
//! undoable unit.
//!
//! `Sprite.layers` is a flat Vec in z-order with group nesting encoded by
//! [`crate::Layer::parent`] and the contiguity invariant (a group's
//! subtree is one unbroken run right after the header — see
//! [`super::MoveLayer`]). This command drains that run and drops the cels
//! keyed to the removed layer ids; `revert` splices the layers back at
//! their original z-position and restores the cels.

use std::collections::{BTreeMap, BTreeSet};

use crate::document::{Cel, CelMap, FrameIndex, Layer, LayerId, Sprite};

use super::Command;
use super::error::CommandError;

/// Remove `layer` and its descendant subtree plus the cels those layers
/// own. Fails with [`CommandError::UnknownLayer`] when the id is absent
/// (the document is left untouched).
#[derive(Debug, Clone)]
pub struct RemoveLayer {
    layer: LayerId,
    /// Captured on a successful `apply` for `revert`: the index the
    /// subtree started at, the removed layers (original order), and the
    /// removed cels.
    removed: Option<(usize, Vec<Layer>, Vec<Cel>)>,
}

impl RemoveLayer {
    pub fn new(layer: LayerId) -> Self {
        Self {
            layer,
            removed: None,
        }
    }
}

/// True when `id`'s parent chain reaches `ancestor`.
fn is_descendant(
    parents: &BTreeMap<LayerId, Option<LayerId>>,
    mut id: LayerId,
    ancestor: LayerId,
) -> bool {
    while let Some(Some(p)) = parents.get(&id).copied() {
        if p == ancestor {
            return true;
        }
        id = p;
    }
    false
}

impl Command for RemoveLayer {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let i = doc
            .layers
            .iter()
            .position(|l| l.id == self.layer)
            .ok_or(CommandError::UnknownLayer(self.layer.0))?;

        // Contiguous subtree [i..end): the layer plus any descendants.
        let parents: BTreeMap<LayerId, Option<LayerId>> =
            doc.layers.iter().map(|l| (l.id, l.parent)).collect();
        let mut end = i + 1;
        while end < doc.layers.len() && is_descendant(&parents, doc.layers[end].id, self.layer) {
            end += 1;
        }

        let removed_layers: Vec<Layer> = doc.layers.drain(i..end).collect();
        let removed_ids: BTreeSet<LayerId> = removed_layers.iter().map(|l| l.id).collect();

        // Drop every cel keyed to a removed layer, capturing them for revert.
        let keys: Vec<(LayerId, FrameIndex)> = cels
            .iter()
            .map(|(k, _)| *k)
            .filter(|(lid, _)| removed_ids.contains(lid))
            .collect();
        let mut removed_cels = Vec::with_capacity(keys.len());
        for (lid, fid) in keys {
            if let Some(cel) = cels.remove(lid, fid) {
                removed_cels.push(cel);
            }
        }

        self.removed = Some((i, removed_layers, removed_cels));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap) {
        let Some((start, layers, removed_cels)) = self.removed.take() else {
            return;
        };
        // Splice the subtree back at its original position, order preserved.
        let start = start.min(doc.layers.len());
        for (offset, layer) in layers.into_iter().enumerate() {
            doc.layers.insert(start + offset, layer);
        }
        for cel in removed_cels {
            cels.insert(cel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{ColorMode, Frame, PixelBuffer, Sprite};

    /// Build a sprite from `(id, parent)` pairs in z-order (all image
    /// layers; only `parent` matters for subtree grouping) with two frames.
    fn sprite_with(layers: &[(u32, Option<u32>)]) -> (Sprite, CelMap) {
        let mut b = Sprite::builder(4, 4).add_frame(Frame::new(100));
        for &(id, parent) in layers {
            let mut l = Layer::image(LayerId::new(id), format!("l{id}"));
            l.parent = parent.map(LayerId::new);
            b = b.add_layer(l);
        }
        (b.build().expect("sprite builds"), CelMap::new())
    }

    fn order(sprite: &Sprite) -> Vec<u32> {
        sprite.layers.iter().map(|l| l.id.0).collect()
    }

    fn img_cel(layer: u32, frame: u32) -> Cel {
        Cel::image(
            LayerId::new(layer),
            FrameIndex::new(frame),
            PixelBuffer::empty(4, 4, ColorMode::Rgba),
        )
    }

    #[test]
    fn remove_leaf_layer_and_restore_it() {
        let (mut s, mut c) = sprite_with(&[(0, None), (1, None), (2, None)]);
        let mut cmd = RemoveLayer::new(LayerId::new(1));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(order(&s), vec![0, 2]);
        cmd.revert(&mut s, &mut c);
        assert_eq!(order(&s), vec![0, 1, 2], "layer restored at original index");
    }

    #[test]
    fn remove_layer_drops_its_cels_across_frames_and_revert_restores() {
        let (mut s, mut c) = sprite_with(&[(0, None), (1, None)]);
        c.insert(img_cel(1, 0));
        c.insert(img_cel(1, 1));
        c.insert(img_cel(0, 0));
        assert_eq!(c.len(), 3);
        let mut cmd = RemoveLayer::new(LayerId::new(1));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert!(c.get(LayerId::new(1), FrameIndex::new(0)).is_none());
        assert!(c.get(LayerId::new(1), FrameIndex::new(1)).is_none());
        assert!(
            c.get(LayerId::new(0), FrameIndex::new(0)).is_some(),
            "other layers' cels untouched"
        );
        assert_eq!(c.len(), 1);
        cmd.revert(&mut s, &mut c);
        assert_eq!(c.len(), 3, "removed cels restored");
        assert!(c.get(LayerId::new(1), FrameIndex::new(1)).is_some());
    }

    #[test]
    fn removing_a_group_takes_its_whole_subtree() {
        // z-order: img0, group1 { child2, child3 }, img4
        let (mut s, mut c) =
            sprite_with(&[(0, None), (1, None), (2, Some(1)), (3, Some(1)), (4, None)]);
        c.insert(img_cel(2, 0));
        c.insert(img_cel(3, 1));
        let mut cmd = RemoveLayer::new(LayerId::new(1));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(order(&s), vec![0, 4], "group + children removed as a block");
        assert!(c.is_empty(), "children cels removed with the group");
        cmd.revert(&mut s, &mut c);
        assert_eq!(order(&s), vec![0, 1, 2, 3, 4]);
        assert_eq!(c.len(), 2, "children cels restored");
    }

    #[test]
    fn unknown_layer_is_rejected_without_touching_the_document() {
        let (mut s, mut c) = sprite_with(&[(0, None)]);
        let mut cmd = RemoveLayer::new(LayerId::new(9));
        assert_eq!(
            cmd.apply(&mut s, &mut c),
            Err(CommandError::UnknownLayer(9))
        );
        assert_eq!(order(&s), vec![0]);
    }
}
