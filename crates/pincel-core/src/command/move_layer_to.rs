//! `MoveLayerTo` command — drag a layer (with its subtree) anywhere in
//! the stack, into or out of groups.
//!
//! `Sprite.layers` keeps a group's subtree as one contiguous run right
//! after the group header (index `0` = bottom). This command cuts the
//! moved subtree out, gives its root a new parent and splices it back in
//! next to the target, so the run stays contiguous and every descendant
//! keeps its parent. See the 2026-10-12 Decision Log entry in
//! `docs/specs/pincel.md` (it supersedes the 2026-05-31 sibling-only rule).

use std::collections::BTreeMap;

use crate::document::{CelMap, Layer, LayerId, LayerKind, Sprite};

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;
use super::move_layer::{is_descendant, subtree_end};

/// Where a moved layer lands relative to its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerPlacement {
    /// Directly above the target's whole subtree, sharing its parent.
    Above,
    /// Directly below the target, sharing its parent.
    Below,
    /// Inside the target group, as its top-most child.
    Into,
}

/// What `apply` did, so `revert` can put the subtree back.
#[derive(Debug, Clone, Copy)]
struct Applied {
    /// Index the subtree was spliced in at.
    inserted_at: usize,
    /// Length of the subtree (root + descendants).
    len: usize,
    /// Index the subtree was cut from.
    removed_at: usize,
    /// The root's parent before the move.
    old_parent: Option<LayerId>,
}

/// Move `layer` and its subtree next to (or into) `target`. One undo
/// entry. Fails without touching the document when either id is unknown
/// ([`CommandError::UnknownLayer`]), when `target` is `layer` or inside
/// it ([`CommandError::LayerIntoOwnSubtree`]), or when `Into` names a
/// layer that is not a group ([`CommandError::NotAGroup`]).
#[derive(Debug, Clone)]
pub struct MoveLayerTo {
    layer: LayerId,
    target: LayerId,
    place: LayerPlacement,
    applied: Option<Applied>,
}

impl MoveLayerTo {
    pub fn new(layer: LayerId, target: LayerId, place: LayerPlacement) -> Self {
        Self {
            layer,
            target,
            place,
            applied: None,
        }
    }
}

fn parents(layers: &[Layer]) -> BTreeMap<LayerId, Option<LayerId>> {
    layers.iter().map(|l| (l.id, l.parent)).collect()
}

fn index_of(layers: &[Layer], id: LayerId) -> Option<usize> {
    layers.iter().position(|l| l.id == id)
}

impl Command for MoveLayerTo {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let layers = &mut doc.layers;
        let i = index_of(layers, self.layer).ok_or(CommandError::UnknownLayer(self.layer.0))?;
        let t = index_of(layers, self.target).ok_or(CommandError::UnknownLayer(self.target.0))?;
        let map = parents(layers);
        if self.target == self.layer || is_descendant(&map, self.target, self.layer) {
            return Err(CommandError::LayerIntoOwnSubtree(self.layer.0));
        }
        if self.place == LayerPlacement::Into && !matches!(layers[t].kind, LayerKind::Group) {
            return Err(CommandError::NotAGroup(self.target.0));
        }
        let end = subtree_end(layers, &map, i);
        let old_parent = layers[i].parent;
        let mut block: Vec<Layer> = layers.drain(i..end).collect();

        // Re-locate the target in the shortened stack.
        let t = index_of(layers, self.target).expect("target is outside the moved subtree");
        let map = parents(layers);
        let (inserted_at, new_parent) = match self.place {
            LayerPlacement::Below => (t, layers[t].parent),
            LayerPlacement::Above => (subtree_end(layers, &map, t), layers[t].parent),
            LayerPlacement::Into => (subtree_end(layers, &map, t), Some(self.target)),
        };
        block[0].parent = new_parent;
        let len = block.len();
        layers.splice(inserted_at..inserted_at, block);
        self.applied = Some(Applied {
            inserted_at,
            len,
            removed_at: i,
            old_parent,
        });
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some(a) = self.applied.take() else {
            return;
        };
        let layers = &mut doc.layers;
        if a.inserted_at + a.len > layers.len() {
            return;
        }
        let mut block: Vec<Layer> = layers.drain(a.inserted_at..a.inserted_at + a.len).collect();
        block[0].parent = a.old_parent;
        let at = a.removed_at.min(layers.len());
        layers.splice(at..at, block);
    }

    fn dirty_region(&self) -> DirtyRegion {
        if self.applied.is_some() {
            DirtyRegion::Canvas
        } else {
            DirtyRegion::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(id, parent, is_group)` bottom-first.
    fn sprite_with(layers: &[(u32, Option<u32>, bool)]) -> (Sprite, CelMap) {
        let mut b = Sprite::builder(8, 8);
        for &(id, parent, group) in layers {
            let mut l = if group {
                Layer::group(LayerId::new(id), format!("g{id}"))
            } else {
                Layer::image(LayerId::new(id), format!("l{id}"))
            };
            l.parent = parent.map(LayerId::new);
            b = b.add_layer(l);
        }
        (b.build().expect("sprite builds"), CelMap::new())
    }

    fn shape(s: &Sprite) -> Vec<(u32, Option<u32>)> {
        s.layers
            .iter()
            .map(|l| (l.id.0, l.parent.map(|p| p.0)))
            .collect()
    }

    fn id(n: u32) -> LayerId {
        LayerId::new(n)
    }

    #[test]
    fn move_into_group_makes_it_the_top_child_and_undo_restores() {
        // 1 (root), group 2 { 3 }, 4 (root)
        let (mut s, mut c) = sprite_with(&[
            (1, None, false),
            (2, None, true),
            (3, Some(2), false),
            (4, None, false),
        ]);
        let before = shape(&s);
        let mut cmd = MoveLayerTo::new(id(1), id(2), LayerPlacement::Into);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(
            shape(&s),
            vec![(2, None), (3, Some(2)), (1, Some(2)), (4, None)]
        );
        assert_eq!(cmd.dirty_region(), DirtyRegion::Canvas);
        cmd.revert(&mut s, &mut c);
        assert_eq!(shape(&s), before);
    }

    #[test]
    fn move_out_of_group_above_a_root_layer() {
        let (mut s, mut c) = sprite_with(&[
            (2, None, true),
            (3, Some(2), false),
            (5, Some(2), false),
            (4, None, false),
        ]);
        MoveLayerTo::new(id(3), id(4), LayerPlacement::Above)
            .apply(&mut s, &mut c)
            .expect("apply");
        assert_eq!(
            shape(&s),
            vec![(2, None), (5, Some(2)), (4, None), (3, None)]
        );
    }

    #[test]
    fn move_below_a_group_lands_under_its_header() {
        let (mut s, mut c) = sprite_with(&[(2, None, true), (3, Some(2), false), (4, None, false)]);
        MoveLayerTo::new(id(4), id(2), LayerPlacement::Below)
            .apply(&mut s, &mut c)
            .expect("apply");
        assert_eq!(shape(&s), vec![(4, None), (2, None), (3, Some(2))]);
    }

    #[test]
    fn a_group_moves_with_its_children_into_another_group() {
        // group 1 { 2 }, group 3 { 4 }
        let (mut s, mut c) = sprite_with(&[
            (1, None, true),
            (2, Some(1), false),
            (3, None, true),
            (4, Some(3), false),
        ]);
        let before = shape(&s);
        let mut cmd = MoveLayerTo::new(id(1), id(3), LayerPlacement::Into);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(
            shape(&s),
            vec![(3, None), (4, Some(3)), (1, Some(3)), (2, Some(1))]
        );
        cmd.revert(&mut s, &mut c);
        assert_eq!(shape(&s), before);
    }

    #[test]
    fn moving_into_own_subtree_or_a_non_group_errs_without_change() {
        let (mut s, mut c) = sprite_with(&[(1, None, true), (2, Some(1), false), (3, None, false)]);
        let before = shape(&s);
        assert!(matches!(
            MoveLayerTo::new(id(1), id(2), LayerPlacement::Above).apply(&mut s, &mut c),
            Err(CommandError::LayerIntoOwnSubtree(1))
        ));
        assert!(matches!(
            MoveLayerTo::new(id(1), id(1), LayerPlacement::Into).apply(&mut s, &mut c),
            Err(CommandError::LayerIntoOwnSubtree(1))
        ));
        assert!(matches!(
            MoveLayerTo::new(id(2), id(3), LayerPlacement::Into).apply(&mut s, &mut c),
            Err(CommandError::NotAGroup(3))
        ));
        assert!(matches!(
            MoveLayerTo::new(id(9), id(3), LayerPlacement::Above).apply(&mut s, &mut c),
            Err(CommandError::UnknownLayer(9))
        ));
        assert_eq!(shape(&s), before);
    }
}
