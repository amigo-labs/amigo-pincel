//! `MoveLayer` command — reorder a layer up or down among its siblings.
//!
//! `Sprite.layers` is a flat Vec in z-order (index `0` = bottom). Group
//! nesting is encoded by [`crate::Layer::parent`] plus the invariant that
//! a group's subtree is contiguous immediately after the group header
//! (enforced by `codec::aseprite_write::validate_layer_order`). This
//! command swaps a layer's whole subtree with the adjacent sibling's
//! subtree, so moving a group carries its children and a layer never
//! crosses into or out of a group. Cross-group / arbitrary drag moves are
//! deferred to a later slice (see `STATUS.md` Layers plan).

use std::collections::BTreeMap;

use crate::document::{CelMap, Layer, LayerId, Sprite};

use super::Command;
use super::error::CommandError;

/// Direction to move a layer through the z-order stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    /// Toward the top of the stack (higher index).
    Up,
    /// Toward the bottom of the stack (lower index).
    Down,
}

/// Reorder `layer` one sibling position in `direction`, moving its whole
/// subtree as a unit. Fails with [`CommandError::LayerAtEdge`] when there
/// is no sibling in that direction, and [`CommandError::UnknownLayer`]
/// when the id is absent — neither touches the document.
#[derive(Debug, Clone)]
pub struct MoveLayer {
    layer: LayerId,
    direction: MoveDirection,
    /// `Some((start, amount, end))` after a successful apply: `apply`
    /// rotated `layers[start..end]` left by `amount`, so `revert` rotates
    /// the same range right by `amount`.
    applied: Option<(usize, usize, usize)>,
}

impl MoveLayer {
    pub fn new(layer: LayerId, direction: MoveDirection) -> Self {
        Self {
            layer,
            direction,
            applied: None,
        }
    }
}

fn index_of(layers: &[Layer], id: LayerId) -> Option<usize> {
    layers.iter().position(|l| l.id == id)
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

/// Exclusive end of the subtree rooted at `layers[i]`: the first index
/// after `i` whose layer is not a descendant of `layers[i]`. Relies on
/// the contiguity invariant (a subtree is one unbroken run).
fn subtree_end(layers: &[Layer], parents: &BTreeMap<LayerId, Option<LayerId>>, i: usize) -> usize {
    let root = layers[i].id;
    let mut j = i + 1;
    while j < layers.len() && is_descendant(parents, layers[j].id, root) {
        j += 1;
    }
    j
}

impl Command for MoveLayer {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let layers = &mut doc.layers;
        let i = index_of(layers, self.layer).ok_or(CommandError::UnknownLayer(self.layer.0))?;
        let parents: BTreeMap<LayerId, Option<LayerId>> =
            layers.iter().map(|l| (l.id, l.parent)).collect();
        let parent = layers[i].parent;
        let end_i = subtree_end(layers, &parents, i);

        let (start, amount, end) = match self.direction {
            MoveDirection::Up => {
                // The sibling above begins right after our subtree, and is
                // a sibling only if it shares our parent (otherwise we are
                // the last child of our group → at the edge).
                if end_i >= layers.len() || layers[end_i].parent != parent {
                    return Err(CommandError::LayerAtEdge(self.layer.0));
                }
                let sib_end = subtree_end(layers, &parents, end_i);
                let our_len = end_i - i;
                layers[i..sib_end].rotate_left(our_len);
                (i, our_len, sib_end)
            }
            MoveDirection::Down => {
                if i == 0 {
                    return Err(CommandError::LayerAtEdge(self.layer.0));
                }
                // Walk up-and-left from i-1 to the previous sibling's
                // subtree root (the node sharing our parent). Hitting our
                // own group header means we are the first child → edge.
                let mut k = i - 1;
                let prev = loop {
                    if Some(layers[k].id) == parent {
                        return Err(CommandError::LayerAtEdge(self.layer.0));
                    }
                    if layers[k].parent == parent {
                        break k;
                    }
                    match layers[k].parent {
                        Some(pid) => {
                            k = index_of(layers, pid)
                                .expect("parent present in layer vec (invariant)");
                        }
                        None => return Err(CommandError::LayerAtEdge(self.layer.0)),
                    }
                };
                let prev_len = i - prev;
                layers[prev..end_i].rotate_left(prev_len);
                (prev, prev_len, end_i)
            }
        };
        self.applied = Some((start, amount, end));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some((start, amount, end)) = self.applied.take() else {
            return;
        };
        if end <= doc.layers.len() && start < end {
            doc.layers[start..end].rotate_right(amount);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Layer, LayerId, Sprite};

    /// Build a sprite whose layers are constructed from `(id, parent)`
    /// pairs in z-order. All are image layers unless a later layer names
    /// them as a parent (then they act as groups — kind doesn't matter
    /// for ordering, only `parent` does).
    fn sprite_with(layers: &[(u32, Option<u32>)]) -> (Sprite, CelMap) {
        let mut b = Sprite::builder(8, 8);
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

    #[test]
    fn move_up_swaps_with_next_sibling() {
        let (mut s, mut c) = sprite_with(&[(0, None), (1, None), (2, None)]);
        let mut cmd = MoveLayer::new(LayerId::new(1), MoveDirection::Up);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(order(&s), vec![0, 2, 1]);
        cmd.revert(&mut s, &mut c);
        assert_eq!(order(&s), vec![0, 1, 2]);
    }

    #[test]
    fn move_down_swaps_with_previous_sibling() {
        let (mut s, mut c) = sprite_with(&[(0, None), (1, None), (2, None)]);
        let mut cmd = MoveLayer::new(LayerId::new(2), MoveDirection::Down);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(order(&s), vec![0, 2, 1]);
        cmd.revert(&mut s, &mut c);
        assert_eq!(order(&s), vec![0, 1, 2]);
    }

    #[test]
    fn top_layer_cannot_move_up() {
        let (mut s, mut c) = sprite_with(&[(0, None), (1, None)]);
        let mut cmd = MoveLayer::new(LayerId::new(1), MoveDirection::Up);
        assert_eq!(cmd.apply(&mut s, &mut c), Err(CommandError::LayerAtEdge(1)));
        assert_eq!(order(&s), vec![0, 1], "document untouched on edge");
    }

    #[test]
    fn bottom_layer_cannot_move_down() {
        let (mut s, mut c) = sprite_with(&[(0, None), (1, None)]);
        let mut cmd = MoveLayer::new(LayerId::new(0), MoveDirection::Down);
        assert_eq!(cmd.apply(&mut s, &mut c), Err(CommandError::LayerAtEdge(0)));
    }

    #[test]
    fn unknown_layer_is_rejected() {
        let (mut s, mut c) = sprite_with(&[(0, None)]);
        let mut cmd = MoveLayer::new(LayerId::new(9), MoveDirection::Up);
        assert_eq!(
            cmd.apply(&mut s, &mut c),
            Err(CommandError::UnknownLayer(9))
        );
    }

    #[test]
    fn moving_a_group_carries_its_children() {
        // z-order: img0, group1 { child2, child3 }, img4
        let (mut s, mut c) =
            sprite_with(&[(0, None), (1, None), (2, Some(1)), (3, Some(1)), (4, None)]);
        // Move the group (id 1) up past sibling img4: the whole subtree
        // [1,2,3] jumps over [4].
        let mut cmd = MoveLayer::new(LayerId::new(1), MoveDirection::Up);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(order(&s), vec![0, 4, 1, 2, 3]);
        cmd.revert(&mut s, &mut c);
        assert_eq!(order(&s), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn a_leaf_jumps_over_a_sibling_group_as_a_block() {
        // z-order: group0 { child1 }, img2
        let (mut s, mut c) = sprite_with(&[(0, None), (1, Some(0)), (2, None)]);
        // img2 moves down past the whole group [0,1].
        let mut cmd = MoveLayer::new(LayerId::new(2), MoveDirection::Down);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(order(&s), vec![2, 0, 1]);
        cmd.revert(&mut s, &mut c);
        assert_eq!(order(&s), vec![0, 1, 2]);
    }

    #[test]
    fn child_moves_within_its_group_only() {
        // group0 { child1, child2 }
        let (mut s, mut c) = sprite_with(&[(0, None), (1, Some(0)), (2, Some(0))]);
        // child1 up → swaps with sibling child2 inside the group.
        let mut cmd = MoveLayer::new(LayerId::new(1), MoveDirection::Up);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(order(&s), vec![0, 2, 1]);
    }

    #[test]
    fn first_child_cannot_move_down_out_of_group() {
        // img0, group1 { child2 }
        let (mut s, mut c) = sprite_with(&[(0, None), (1, None), (2, Some(1))]);
        // child2 is the first (and only) child → no sibling below within
        // the group; it must not escape into img0's level.
        let mut cmd = MoveLayer::new(LayerId::new(2), MoveDirection::Down);
        assert_eq!(cmd.apply(&mut s, &mut c), Err(CommandError::LayerAtEdge(2)));
        assert_eq!(order(&s), vec![0, 1, 2]);
    }

    #[test]
    fn last_child_cannot_move_up_out_of_group() {
        // group0 { child1 }, img2
        let (mut s, mut c) = sprite_with(&[(0, None), (1, Some(0)), (2, None)]);
        let mut cmd = MoveLayer::new(LayerId::new(1), MoveDirection::Up);
        assert_eq!(cmd.apply(&mut s, &mut c), Err(CommandError::LayerAtEdge(1)));
    }
}
