//! `ReorderLayer` command — drag a layer to another sibling's slot.
//!
//! Built from [`MoveLayer`] steps, so it keeps that command's rules: a
//! layer moves with its whole subtree and never leaves its group (see the
//! 2026-05-31 Decision Log entry). The target may be any layer nested in
//! a sibling of the moved one; the move lands on that sibling's slot.
//! The whole drag is one undo entry.

use crate::document::{CelMap, LayerId, Sprite};

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;
use super::move_layer::{MoveDirection, MoveLayer};

/// Move `layer` (with its subtree) to the sibling slot of `target`.
#[derive(Debug, Clone)]
pub struct ReorderLayer {
    layer: LayerId,
    target: LayerId,
    /// The single-step moves `apply` ran, undone in reverse by `revert`.
    steps: Vec<MoveLayer>,
}

impl ReorderLayer {
    /// Move `layer` to where `target` (or the sibling of `layer` that
    /// contains it) sits now.
    pub fn new(layer: LayerId, target: LayerId) -> Self {
        Self {
            layer,
            target,
            steps: Vec::new(),
        }
    }
}

/// Sibling ids of `parent` in z-order (bottom first).
fn siblings(doc: &Sprite, parent: Option<LayerId>) -> Vec<LayerId> {
    doc.layers
        .iter()
        .filter(|l| l.parent == parent)
        .map(|l| l.id)
        .collect()
}

/// The ancestor-or-self of `id` whose parent is `parent`.
fn sibling_ancestor(doc: &Sprite, id: LayerId, parent: Option<LayerId>) -> Option<LayerId> {
    let mut cur = id;
    for _ in 0..=doc.layers.len() {
        let layer = doc.layer(cur)?;
        if layer.parent == parent {
            return Some(cur);
        }
        cur = layer.parent?;
    }
    None
}

impl Command for ReorderLayer {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let parent = doc
            .layer(self.layer)
            .ok_or(CommandError::UnknownLayer(self.layer.0))?
            .parent;
        if doc.layer(self.target).is_none() {
            return Err(CommandError::UnknownLayer(self.target.0));
        }
        let not_sibling = CommandError::NotASibling {
            layer: self.layer.0,
            target: self.target.0,
        };
        let target = match sibling_ancestor(doc, self.target, parent) {
            Some(t) => t,
            None => return Err(not_sibling),
        };
        let order = siblings(doc, parent);
        let from = order.iter().position(|&id| id == self.layer);
        let to = order.iter().position(|&id| id == target);
        let (Some(from), Some(to)) = (from, to) else {
            return Err(not_sibling);
        };
        let direction = if to > from {
            MoveDirection::Up
        } else {
            MoveDirection::Down
        };
        self.steps.clear();
        for _ in 0..from.abs_diff(to) {
            let mut step = MoveLayer::new(self.layer, direction);
            if let Err(e) = step.apply(doc, cels) {
                self.revert(doc, cels);
                return Err(e);
            }
            self.steps.push(step);
        }
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap) {
        while let Some(mut step) = self.steps.pop() {
            step.revert(doc, cels);
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        if self.steps.is_empty() {
            DirtyRegion::None
        } else {
            DirtyRegion::Canvas
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Layer;

    fn sprite_with(layers: &[(u32, Option<u32>)]) -> (Sprite, CelMap) {
        let mut b = Sprite::builder(8, 8);
        for &(id, parent) in layers {
            let mut l = Layer::image(LayerId::new(id), format!("l{id}"));
            l.parent = parent.map(LayerId::new);
            b = b.add_layer(l);
        }
        (b.build().expect("sprite builds"), CelMap::new())
    }

    fn order(s: &Sprite) -> Vec<u32> {
        s.layers.iter().map(|l| l.id.0).collect()
    }

    #[test]
    fn apply_moves_bottom_layer_to_top_and_revert_restores() {
        let (mut s, mut c) = sprite_with(&[(0, None), (1, None), (2, None), (3, None)]);
        let mut cmd = ReorderLayer::new(LayerId::new(0), LayerId::new(3));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(order(&s), vec![1, 2, 3, 0]);
        assert!(matches!(cmd.dirty_region(), DirtyRegion::Canvas));
        cmd.revert(&mut s, &mut c);
        assert_eq!(order(&s), vec![0, 1, 2, 3]);
    }

    #[test]
    fn apply_moves_down_past_a_group_as_a_block() {
        // 0, group 1 { 2, 3 }, 4  →  move 4 onto 1's slot.
        let (mut s, mut c) =
            sprite_with(&[(0, None), (1, None), (2, Some(1)), (3, Some(1)), (4, None)]);
        let mut cmd = ReorderLayer::new(LayerId::new(4), LayerId::new(3));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(order(&s), vec![0, 4, 1, 2, 3]);
    }

    #[test]
    fn apply_onto_itself_is_a_no_op() {
        let (mut s, mut c) = sprite_with(&[(0, None), (1, None)]);
        let mut cmd = ReorderLayer::new(LayerId::new(1), LayerId::new(1));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(order(&s), vec![0, 1]);
        assert!(cmd.dirty_region().is_none());
    }

    #[test]
    fn apply_into_another_group_errs() {
        let (mut s, mut c) = sprite_with(&[(0, None), (1, None), (2, Some(1))]);
        let err = ReorderLayer::new(LayerId::new(2), LayerId::new(0))
            .apply(&mut s, &mut c)
            .unwrap_err();
        assert!(matches!(
            err,
            CommandError::NotASibling {
                layer: 2,
                target: 0
            }
        ));
        assert_eq!(order(&s), vec![0, 1, 2]);
    }
}
