//! The layer tree inside the flat layer list (docs/specs/image-layer-groups.md).
//!
//! `Document::layers` stays flat and bottom-first. A group's subtree is one
//! contiguous run directly after the group entry, and every child names its
//! group in `parent` — the same shape as Pixel mode's `Sprite.layers`
//! (docs/specs/pincel.md §15, 2026-10-12 and 2026-10-29).

use super::{Document, Layer, MAX_LAYERS};
use crate::error::DocumentError;
use std::collections::HashSet;
use uuid::Uuid;

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

/// Exclusive end of the subtree rooted at `layers[i]`: `i + 1` for a raster
/// layer, the first index past the group's descendants for a group. Relies
/// on the contiguity invariant and treats parents outside the slice as
/// roots, so it works on any run of whole subtrees.
pub(crate) fn subtree_end(layers: &[Layer], i: usize) -> usize {
    if !layers[i].is_group() {
        return i + 1;
    }
    let mut groups = vec![layers[i].id];
    let mut j = i + 1;
    while let Some(layer) = layers.get(j) {
        match layer.parent {
            Some(p) if groups.contains(&p) => {
                if layer.is_group() {
                    groups.push(layer.id);
                }
                j += 1;
            }
            _ => break,
        }
    }
    j
}

/// Whether `layers` is a well-formed tree: unique ids, every parent is a
/// group listed earlier, and each group's descendants form one contiguous
/// run directly after it.
pub(crate) fn well_formed(layers: &[Layer]) -> bool {
    let mut ids = HashSet::with_capacity(layers.len());
    // The chain of open groups from the root to the latest group.
    let mut open: Vec<Uuid> = Vec::new();
    for layer in layers {
        if !ids.insert(layer.id) {
            return false;
        }
        match layer.parent {
            None => open.clear(),
            Some(p) => {
                while open.last().is_some_and(|&top| top != p) {
                    open.pop();
                }
                if open.is_empty() {
                    return false;
                }
            }
        }
        if layer.is_group() {
            open.push(layer.id);
        }
    }
    true
}

/// What [`Document::move_subtree`] did, so the move can be reverted.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SubtreeMove {
    /// Index the subtree was spliced in at.
    pub inserted_at: usize,
    /// Length of the subtree (root + descendants).
    pub len: usize,
    /// Index the subtree was cut from.
    pub removed_at: usize,
    /// The root's parent before the move.
    pub old_parent: Option<Uuid>,
}

impl Document {
    /// Exclusive end of the subtree rooted at `index` (`index + 1` for a
    /// raster layer). Returns `index + 1` for an out-of-range index.
    pub fn subtree_end(&self, index: usize) -> usize {
        if index >= self.layers.len() {
            return index + 1;
        }
        subtree_end(&self.layers, index)
    }

    /// Index of the group the layer at `index` sits in; `None` at the root
    /// or for an out-of-range index.
    pub fn parent_index(&self, index: usize) -> Option<usize> {
        let parent = self.layers.get(index)?.parent?;
        self.layers[..index].iter().rposition(|l| l.id == parent)
    }

    /// Number of groups the layer at `index` is nested in (0 at the root).
    pub fn depth(&self, index: usize) -> usize {
        let mut depth = 0;
        let mut at = index;
        while let Some(p) = self.parent_index(at) {
            depth += 1;
            at = p;
        }
        depth
    }

    /// Whether any group the layer at `index` sits in (at any level) is
    /// locked; the layer's own flag is not considered.
    pub fn locked_by_parent(&self, index: usize) -> bool {
        self.ancestors(index).any(|i| self.layers[i].locked)
    }

    /// Whether the layer at `index` is locked itself or through a group it
    /// sits in. `false` for an out-of-range index.
    pub fn effectively_locked(&self, index: usize) -> bool {
        self.layers.get(index).is_some_and(|l| l.locked) || self.locked_by_parent(index)
    }

    /// Indices of the groups the layer at `index` sits in, innermost first.
    fn ancestors(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(self.parent_index(index), move |&i| self.parent_index(i))
    }

    /// Whether the layer list is a well-formed tree (see
    /// docs/specs/image-layer-groups.md AC-06): every child's parent exists
    /// and is a group, no layer is its own ancestor, and each group's
    /// descendants are listed directly after it.
    pub fn is_well_formed(&self) -> bool {
        well_formed(&self.layers)
    }

    /// Number of raster (non-group) layers.
    pub fn raster_count(&self) -> usize {
        self.layers.iter().filter(|l| !l.is_group()).count()
    }

    /// Inserts a run of whole subtrees at `index` and activates its first
    /// layer. Refused as a whole at the layer limit and when the result
    /// would not be a well-formed tree.
    pub(crate) fn insert_subtree(
        &mut self,
        index: usize,
        block: Vec<Layer>,
    ) -> Result<(), DocumentError> {
        if self.layers.len() + block.len() > MAX_LAYERS {
            return Err(DocumentError::LayerLimit { max: MAX_LAYERS });
        }
        if index > self.layers.len() {
            return Err(DocumentError::LayerIndexOutOfBounds {
                index,
                len: self.layers.len(),
            });
        }
        let len = block.len();
        self.layers.splice(index..index, block);
        if !self.is_well_formed() {
            self.layers.drain(index..index + len);
            return Err(DocumentError::LayerTreeBroken);
        }
        self.active_layer = index;
        Ok(())
    }

    /// Removes the layer at `index` with its whole subtree and returns the
    /// removed run. Refused when no raster layer would remain (AC-20).
    pub(crate) fn remove_subtree(&mut self, index: usize) -> Result<Vec<Layer>, DocumentError> {
        let len = self.layers.len();
        if index >= len {
            return Err(DocumentError::LayerIndexOutOfBounds { index, len });
        }
        let end = subtree_end(&self.layers, index);
        let removed_rasters = self.layers[index..end]
            .iter()
            .filter(|l| !l.is_group())
            .count();
        if removed_rasters == self.raster_count() {
            return Err(DocumentError::LastLayer);
        }
        let removed: Vec<Layer> = self.layers.drain(index..end).collect();
        if self.active_layer >= end {
            self.active_layer -= removed.len();
        } else if self.active_layer >= index {
            self.active_layer = index.min(self.layers.len() - 1);
        }
        Ok(removed)
    }

    /// Moves the layer at `index` with its subtree next to (or into) the
    /// layer at `target` and activates it. Refused into its own subtree
    /// (`LayerIntoOwnSubtree`) and `Into` a raster layer (`NotAGroup`).
    pub(crate) fn move_subtree(
        &mut self,
        index: usize,
        target: usize,
        place: LayerPlacement,
    ) -> Result<SubtreeMove, DocumentError> {
        let len = self.layers.len();
        if index >= len || target >= len {
            return Err(DocumentError::LayerIndexOutOfBounds {
                index: index.max(target),
                len,
            });
        }
        let end = subtree_end(&self.layers, index);
        if (index..end).contains(&target) {
            return Err(DocumentError::LayerIntoOwnSubtree);
        }
        if place == LayerPlacement::Into && !self.layers[target].is_group() {
            return Err(DocumentError::NotAGroup { index: target });
        }
        let target_id = self.layers[target].id;
        let old_parent = self.layers[index].parent;
        let mut block: Vec<Layer> = self.layers.drain(index..end).collect();
        let t = self
            .layers
            .iter()
            .position(|l| l.id == target_id)
            .expect("the target lies outside the moved subtree");
        let (inserted_at, new_parent) = match place {
            LayerPlacement::Below => (t, self.layers[t].parent),
            LayerPlacement::Above => (subtree_end(&self.layers, t), self.layers[t].parent),
            LayerPlacement::Into => (subtree_end(&self.layers, t), Some(target_id)),
        };
        block[0].parent = new_parent;
        let moved = block.len();
        self.layers.splice(inserted_at..inserted_at, block);
        self.active_layer = inserted_at;
        Ok(SubtreeMove {
            inserted_at,
            len: moved,
            removed_at: index,
            old_parent,
        })
    }

    /// Reverts a [`move_subtree`](Document::move_subtree).
    pub(crate) fn unmove_subtree(&mut self, m: SubtreeMove) -> Result<(), DocumentError> {
        if m.inserted_at + m.len > self.layers.len() {
            return Err(DocumentError::LayerTreeBroken);
        }
        let mut block: Vec<Layer> = self
            .layers
            .drain(m.inserted_at..m.inserted_at + m.len)
            .collect();
        block[0].parent = m.old_parent;
        let at = m.removed_at.min(self.layers.len());
        self.layers.splice(at..at, block);
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A 4×4 document built from `(name, parent name, is_group)` bottom-first.
    pub(crate) fn tree_doc(spec: &[(&str, Option<&str>, bool)]) -> Document {
        let mut doc = Document::new(4, 4).unwrap();
        let mut layers: Vec<Layer> = Vec::new();
        for &(name, parent, group) in spec {
            let mut l = if group {
                Layer::new_group(name)
            } else {
                Layer::new_transparent(name, 4, 4)
            };
            l.parent = parent.map(|p| layers.iter().find(|x| x.name == p).unwrap().id);
            layers.push(l);
        }
        doc.layers = layers;
        doc
    }

    pub(crate) fn names(doc: &Document) -> Vec<&str> {
        doc.layers().iter().map(|l| l.name.as_str()).collect()
    }

    pub(crate) fn parent_name(doc: &Document, name: &str) -> Option<String> {
        let i = doc.layers().iter().position(|l| l.name == name).unwrap();
        doc.parent_index(i).map(|p| doc.layers()[p].name.clone())
    }

    #[test]
    fn subtree_end_spans_nested_descendants() {
        let doc = tree_doc(&[
            ("bg", None, false),
            ("G", None, true),
            ("a", Some("G"), false),
            ("H", Some("G"), true),
            ("b", Some("H"), false),
            ("top", None, false),
        ]);
        assert_eq!(doc.subtree_end(0), 1);
        assert_eq!(doc.subtree_end(1), 5);
        assert_eq!(doc.subtree_end(3), 5);
        assert_eq!(doc.depth(4), 2);
        assert_eq!(doc.parent_index(4), Some(3));
        assert!(doc.is_well_formed());
    }

    #[test]
    fn is_well_formed_rejects_split_runs_and_raster_parents() {
        let split = tree_doc(&[
            ("G", None, true),
            ("a", Some("G"), false),
            ("x", None, false),
            ("b", Some("G"), false),
        ]);
        assert!(!split.is_well_formed());
        let raster_parent = tree_doc(&[("a", None, false), ("b", Some("a"), false)]);
        assert!(!raster_parent.is_well_formed());
    }

    #[test]
    fn move_subtree_into_own_descendant_is_refused_and_tree_unchanged() {
        let mut doc = tree_doc(&[
            ("bg", None, false),
            ("G", None, true),
            ("H", Some("G"), true),
            ("a", Some("H"), false),
        ]);
        let before = doc.layers.clone();
        for target in [1, 2, 3] {
            for place in [
                LayerPlacement::Into,
                LayerPlacement::Above,
                LayerPlacement::Below,
            ] {
                let err = doc.move_subtree(1, target, place).unwrap_err();
                assert_eq!(err, DocumentError::LayerIntoOwnSubtree);
            }
        }
        assert_eq!(doc.layers, before);
    }

    #[test]
    fn move_subtree_into_raster_layer_is_refused() {
        let mut doc = tree_doc(&[("bg", None, false), ("a", None, false)]);
        assert_eq!(
            doc.move_subtree(1, 0, LayerPlacement::Into).unwrap_err(),
            DocumentError::NotAGroup { index: 0 }
        );
    }

    #[test]
    fn remove_subtree_refuses_removing_the_last_raster_layer() {
        let mut doc = tree_doc(&[("G", None, true), ("a", Some("G"), false)]);
        let before = doc.layers.clone();
        assert_eq!(doc.remove_subtree(0), Err(DocumentError::LastLayer));
        assert_eq!(doc.remove_subtree(1), Err(DocumentError::LastLayer));
        assert_eq!(doc.layers, before);
    }

    #[test]
    fn remove_subtree_removes_the_group_with_its_descendants() {
        let mut doc = tree_doc(&[
            ("bg", None, false),
            ("G", None, true),
            ("a", Some("G"), false),
            ("top", None, false),
        ]);
        let removed = doc.remove_subtree(1).unwrap();
        assert_eq!(removed.len(), 2);
        assert_eq!(names(&doc), ["bg", "top"]);
        assert!(doc.is_well_formed());
    }

    #[test]
    fn insert_subtree_beyond_the_layer_limit_is_refused_as_a_whole() {
        let mut doc = Document::new(1, 1).unwrap();
        for _ in 1..MAX_LAYERS - 1 {
            doc.add_layer("l").unwrap();
        }
        let g = Layer::new_group("G");
        let child = Layer::new_transparent("c", 1, 1).with_parent(Some(g.id));
        let len = doc.layer_count();
        assert_eq!(
            doc.insert_subtree(len, vec![g, child]).unwrap_err(),
            DocumentError::LayerLimit { max: MAX_LAYERS }
        );
        assert_eq!(doc.layer_count(), len);
    }
}
