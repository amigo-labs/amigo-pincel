//! Layer-structure commands: `AddLayer`, `AddGroup`, `RemoveLayer`,
//! `MoveLayer`, `MoveLayerTo`, `DuplicateLayer` (spec §7.3).
//!
//! Groups keep their subtree as one contiguous run after the group entry
//! (docs/specs/image-layer-groups.md); removing, moving or duplicating a
//! group acts on that whole run in one undo step.

use super::HistoryKind;
use super::snapshot::layers_bytes;
use super::{Command, Damage};
use crate::document::tree::SubtreeMove;
use crate::document::{Document, Layer, LayerPlacement};
use crate::error::DocumentError;
use std::any::Any;
use std::collections::HashMap;
use uuid::Uuid;

/// Inserts a new transparent layer above the active layer, at its level:
/// directly above a raster layer, or above a group's whole subtree.
pub struct AddLayer {
    active: usize,
    /// Where the layer went on first apply; redo reuses it.
    index: Option<usize>,
    /// The layer revert took out of the document; redo re-inserts it exactly.
    layer: Option<Layer>,
    prev_active: usize,
}

impl AddLayer {
    /// Adds a layer directly above layer `active` (above its subtree when it
    /// is a group), sharing its parent.
    pub fn above(active: usize) -> Self {
        Self {
            active,
            index: None,
            layer: None,
            prev_active: active,
        }
    }
}

impl Command for AddLayer {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.prev_active = doc.active_layer_index();
        let parent = doc.layers().get(self.active).and_then(|l| l.parent);
        let index = *self
            .index
            .get_or_insert_with(|| doc.subtree_end(self.active));
        let layer = match self.layer.take() {
            Some(l) => l,
            None => Layer::new_transparent(
                format!("Layer {}", doc.layers.len() + 1),
                doc.canvas.width(),
                doc.canvas.height(),
            )
            .with_parent(parent),
        };
        // The layer moves into the document; revert takes it back, so it is
        // held only while undone (performance-image-undo-memory).
        doc.insert_layer(index, layer)
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let index = self.index.ok_or(DocumentError::RegionOutOfBounds)?;
        self.layer = Some(doc.remove_layer(index)?);
        doc.set_active_layer(self.prev_active.min(doc.layers.len() - 1))
    }

    fn undo_bytes(&self) -> usize {
        self.layer.as_ref().map_or(0, |l| l.pixels.data().len())
    }

    fn kind(&self) -> HistoryKind {
        HistoryKind::AddLayer
    }

    /// The inserted layer is fully transparent (redo restores the layer
    /// captured on first apply, before anything was painted on it), and
    /// compose skips transparent pixels, so the composite is unchanged.
    fn damage(&self) -> Damage {
        Damage::None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Adds an empty group named `Group N` on top of the root stack and makes it
/// the active layer (docs/specs/image-layer-groups.md AC-01).
pub struct AddGroup {
    /// The inserted group, captured on first apply so redo restores it.
    group: Option<Layer>,
    index: usize,
    prev_active: usize,
}

impl AddGroup {
    /// Adds the group on top of the root stack.
    pub fn on_top() -> Self {
        Self {
            group: None,
            index: 0,
            prev_active: 0,
        }
    }
}

impl Command for AddGroup {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.prev_active = doc.active_layer_index();
        let group = match self.group.take() {
            Some(g) => g,
            None => {
                let n = doc.layers().iter().filter(|l| l.is_group()).count() + 1;
                Layer::new_group(format!("Group {n}"))
            }
        };
        self.index = doc.layer_count();
        let restored = group.clone();
        doc.insert_layer(self.index, group)?;
        self.group = Some(restored);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        doc.remove_layer(self.index)?;
        doc.set_active_layer(self.prev_active.min(doc.layers.len() - 1))
    }

    fn kind(&self) -> HistoryKind {
        HistoryKind::AddGroup
    }

    /// An empty group contributes nothing to the composite.
    fn damage(&self) -> Damage {
        Damage::None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Removes the layer at `index` — a group with its whole subtree —
/// remembering it for undo. Refused when no raster layer would remain.
pub struct RemoveLayer {
    index: usize,
    removed: Option<Vec<Layer>>,
    prev_active: usize,
}

impl RemoveLayer {
    /// Removes the layer at `index`.
    pub fn at(index: usize) -> Self {
        Self {
            index,
            removed: None,
            prev_active: 0,
        }
    }
}

impl Command for RemoveLayer {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.prev_active = doc.active_layer_index();
        self.removed = Some(doc.remove_subtree(self.index)?);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let block = self
            .removed
            .take()
            .ok_or(DocumentError::RegionOutOfBounds)?;
        doc.insert_subtree(self.index, block)?;
        doc.set_active_layer(self.prev_active)
    }

    fn kind(&self) -> HistoryKind {
        HistoryKind::DeleteLayer
    }

    fn undo_bytes(&self) -> usize {
        self.removed.as_deref().map_or(0, layers_bytes)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Reorders a layer from one index to another. Refused when the move would
/// split a group's run or leave a child outside its group.
pub struct MoveLayer {
    from: usize,
    to: usize,
    prev_active: usize,
}

impl MoveLayer {
    /// Moves the layer at `from` to `to`.
    pub fn new(from: usize, to: usize) -> Self {
        Self {
            from,
            to,
            prev_active: 0,
        }
    }
}

impl Command for MoveLayer {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.prev_active = doc.active_layer_index();
        doc.move_layer(self.from, self.to)
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        doc.move_layer(self.to, self.from)?;
        // move_layer activates the moved layer; restore the prior selection.
        doc.set_active_layer(self.prev_active)
    }

    fn kind(&self) -> HistoryKind {
        HistoryKind::MoveLayer
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Moves the layer at `index` with its subtree next to (or into) the layer
/// at `target` and makes it active: one undo step (docs/specs/pincel.md §15,
/// 2026-10-12). Refused into its own subtree and `Into` a raster layer.
pub struct MoveLayerTo {
    index: usize,
    target: usize,
    place: LayerPlacement,
    applied: Option<SubtreeMove>,
    prev_active: usize,
}

impl MoveLayerTo {
    /// Moves the layer at `index` relative to the layer at `target`.
    pub fn new(index: usize, target: usize, place: LayerPlacement) -> Self {
        Self {
            index,
            target,
            place,
            applied: None,
            prev_active: 0,
        }
    }
}

impl Command for MoveLayerTo {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.prev_active = doc.active_layer_index();
        self.applied = Some(doc.move_subtree(self.index, self.target, self.place)?);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let m = self
            .applied
            .take()
            .ok_or(DocumentError::RegionOutOfBounds)?;
        doc.unmove_subtree(m)?;
        doc.set_active_layer(self.prev_active)
    }

    fn kind(&self) -> HistoryKind {
        HistoryKind::MoveLayer
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Duplicates the layer at `index` — a group with its whole subtree —
/// inserting the copy directly above it, at its level.
///
/// The copy is independent (fresh ids) and its root is named
/// `"<name> copy"`, preserving pixels, opacity, blend mode, visibility and
/// lock. Refused as a whole when it would exceed the layer limit.
pub struct DuplicateLayer {
    index: usize,
    /// The copy revert took out of the document; redo re-inserts it exactly.
    copy: Option<Vec<Layer>>,
    inserted_at: usize,
    prev_active: usize,
}

impl DuplicateLayer {
    /// Duplicates the layer at `index`.
    pub fn at(index: usize) -> Self {
        Self {
            index,
            copy: None,
            inserted_at: 0,
            prev_active: 0,
        }
    }
}

/// Clones a run of whole subtrees with fresh ids, re-pointing the parents
/// inside the run at the copies.
pub(crate) fn clone_subtree(block: &[Layer]) -> Vec<Layer> {
    let mut ids: HashMap<Uuid, Uuid> = HashMap::new();
    block
        .iter()
        .map(|l| {
            let mut c = l.clone();
            c.id = Uuid::new_v4();
            ids.insert(l.id, c.id);
            c.parent = l.parent.map(|p| ids.get(&p).copied().unwrap_or(p));
            c
        })
        .collect()
}

impl Command for DuplicateLayer {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.prev_active = doc.active_layer_index();
        if self.index >= doc.layer_count() {
            return Err(DocumentError::LayerIndexOutOfBounds {
                index: self.index,
                len: doc.layer_count(),
            });
        }
        let end = doc.subtree_end(self.index);
        let copy = match self.copy.take() {
            Some(c) => c,
            None => {
                let mut copy = clone_subtree(&doc.layers()[self.index..end]);
                copy[0].name = format!("{} copy", doc.layers()[self.index].name);
                copy
            }
        };
        // The copy moves into the document; revert takes it back, so it is
        // held only while undone (performance-image-undo-memory).
        self.inserted_at = end;
        doc.insert_subtree(end, copy)
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        self.copy = Some(doc.remove_subtree(self.inserted_at)?);
        doc.set_active_layer(self.prev_active)
    }

    fn undo_bytes(&self) -> usize {
        self.copy.as_deref().map_or(0, layers_bytes)
    }

    fn kind(&self) -> HistoryKind {
        HistoryKind::DuplicateLayer
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_layer_round_trip() {
        let mut doc = Document::new(8, 8).unwrap();
        let mut cmd = AddLayer::above(0);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 2);
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 1);
    }

    #[test]
    fn add_layer_below_top_inserts_transparent_layer_above_active_and_undoes() {
        use crate::command::CommandBus;

        // Three opaque layers; the middle one is active, not the top.
        let mut doc = Document::new(4, 4).unwrap();
        doc.add_layer("Middle").unwrap();
        doc.add_layer("Top").unwrap();
        for layer in &mut doc.layers {
            layer.pixels.data_mut().fill(255);
        }
        doc.set_active_layer(1).unwrap();
        let ids: Vec<_> = doc.layers.iter().map(|l| l.id).collect();

        let mut bus = CommandBus::new(doc);
        bus.apply(Box::new(AddLayer::above(1))).unwrap();
        let doc = &bus.document;
        assert_eq!(doc.layers.len(), 4);
        assert_eq!(doc.active_layer_index(), 2, "the new layer is active");
        assert_eq!(
            doc.layers[1].id, ids[1],
            "the old active layer stays below it"
        );
        assert_eq!(doc.layers[3].id, ids[2], "the old top stays on top");
        assert!(
            doc.layers[2]
                .pixels
                .data()
                .iter()
                .skip(3)
                .step_by(4)
                .all(|&a| a == 0),
            "the new layer is fully transparent"
        );

        assert!(bus.undo().unwrap());
        let doc = &bus.document;
        assert_eq!(doc.layers.iter().map(|l| l.id).collect::<Vec<_>>(), ids);
        assert_eq!(doc.active_layer_index(), 1);
    }

    #[test]
    fn remove_layer_round_trip_preserves_layer_identity() {
        let mut doc = Document::new(8, 8).unwrap();
        doc.add_layer("Layer 2").unwrap();
        let target_id = doc.layers[1].id;
        let mut cmd = RemoveLayer::at(1);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 1);
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 2);
        assert_eq!(doc.layers[1].id, target_id);
    }

    #[test]
    fn move_layer_round_trip() {
        let mut doc = Document::new(8, 8).unwrap();
        doc.add_layer("Layer 2").unwrap();
        let bottom_id = doc.layers[0].id;
        let mut cmd = MoveLayer::new(0, 1);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[1].id, bottom_id);
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].id, bottom_id);
    }

    #[test]
    fn duplicate_layer_inserts_independent_copy_above() {
        let mut doc = Document::new(8, 8).unwrap();
        doc.layers[0].name = "Base".to_string();
        let original_id = doc.layers[0].id;
        let mut cmd = DuplicateLayer::at(0);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 2);
        // Copy sits directly above the original and is the active layer.
        assert_eq!(doc.active_layer_index(), 1);
        assert_eq!(doc.layers[1].name, "Base copy");
        assert_ne!(doc.layers[1].id, original_id);
        assert_eq!(doc.layers[0].id, original_id);
    }

    #[test]
    fn duplicate_layer_round_trip_restores_active() {
        let mut doc = Document::new(8, 8).unwrap();
        doc.add_layer("Layer 2").unwrap();
        doc.set_active_layer(0).unwrap();
        let mut cmd = DuplicateLayer::at(1);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 3);
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers.len(), 2);
        assert_eq!(doc.active_layer_index(), 0);
    }

    use crate::command::CommandBus;
    use crate::document::LayerPlacement;
    use crate::document::tree::tests::{names, parent_name, tree_doc};

    fn index(doc: &Document, name: &str) -> usize {
        doc.layers().iter().position(|l| l.name == name).unwrap()
    }

    #[test]
    fn add_group_adds_empty_group_on_top_active_and_undo_removes_it() {
        let mut bus = CommandBus::new(tree_doc(&[("bg", None, false), ("a", None, false)]));
        bus.document.set_active_layer(0).unwrap();
        bus.apply(Box::new(AddGroup::on_top())).unwrap();
        let doc = &bus.document;
        assert_eq!(names(doc), ["bg", "a", "Group 1"]);
        assert!(doc.layers()[2].is_group());
        assert_eq!(doc.active_layer_index(), 2);
        assert!(bus.undo().unwrap());
        assert_eq!(names(&bus.document), ["bg", "a"]);
        assert_eq!(bus.document.active_layer_index(), 0);
    }

    #[test]
    fn move_layer_to_into_group_and_back_out_are_one_undo_step_each() {
        let mut bus = CommandBus::new(tree_doc(&[
            ("bg", None, false),
            ("G", None, true),
            ("a", None, false),
        ]));
        let into = MoveLayerTo::new(2, 1, LayerPlacement::Into);
        bus.apply(Box::new(into)).unwrap();
        assert_eq!(names(&bus.document), ["bg", "G", "a"]);
        assert_eq!(parent_name(&bus.document, "a").as_deref(), Some("G"));

        let out = MoveLayerTo::new(2, 0, LayerPlacement::Below);
        bus.apply(Box::new(out)).unwrap();
        assert_eq!(names(&bus.document), ["a", "bg", "G"]);
        assert_eq!(parent_name(&bus.document, "a"), None);

        assert!(bus.undo().unwrap());
        assert_eq!(parent_name(&bus.document, "a").as_deref(), Some("G"));
        assert!(bus.undo().unwrap());
        assert_eq!(names(&bus.document), ["bg", "G", "a"]);
        assert_eq!(parent_name(&bus.document, "a"), None);
    }

    #[test]
    fn move_layer_to_into_own_subtree_is_refused_and_tree_unchanged() {
        let doc = tree_doc(&[
            ("bg", None, false),
            ("G", None, true),
            ("H", Some("G"), true),
            ("a", Some("H"), false),
        ]);
        let before = doc.layers().to_vec();
        let mut bus = CommandBus::new(doc);
        for (target, place) in [(2, LayerPlacement::Into), (3, LayerPlacement::Above)] {
            let err = bus
                .apply(Box::new(MoveLayerTo::new(1, target, place)))
                .unwrap_err();
            assert_eq!(err, DocumentError::LayerIntoOwnSubtree);
        }
        assert_eq!(bus.document.layers(), before.as_slice());
        assert!(!bus.history.can_undo());
    }

    #[test]
    fn move_layer_to_carries_the_group_subtree_in_one_undo_step() {
        let mut bus = CommandBus::new(tree_doc(&[
            ("bg", None, false),
            ("G", None, true),
            ("a", Some("G"), false),
            ("b", Some("G"), false),
            ("top", None, false),
        ]));
        bus.apply(Box::new(MoveLayerTo::new(1, 4, LayerPlacement::Above)))
            .unwrap();
        assert_eq!(names(&bus.document), ["bg", "top", "G", "a", "b"]);
        assert!(bus.document.is_well_formed());
        assert!(bus.undo().unwrap());
        assert_eq!(names(&bus.document), ["bg", "G", "a", "b", "top"]);
    }

    #[test]
    fn remove_layer_on_group_removes_subtree_and_one_undo_restores_it() {
        let doc = tree_doc(&[
            ("bg", None, false),
            ("G", None, true),
            ("a", Some("G"), false),
            ("top", None, false),
        ]);
        let before = doc.layers().to_vec();
        let mut bus = CommandBus::new(doc);
        bus.apply(Box::new(RemoveLayer::at(1))).unwrap();
        assert_eq!(names(&bus.document), ["bg", "top"]);
        assert!(bus.undo().unwrap());
        assert_eq!(bus.document.layers(), before.as_slice());
    }

    #[test]
    fn remove_layer_leaving_no_raster_layer_is_refused() {
        let doc = tree_doc(&[
            ("G", None, true),
            ("a", Some("G"), false),
            ("E", None, true),
        ]);
        let before = doc.layers().to_vec();
        let mut bus = CommandBus::new(doc);
        for i in [0, 1] {
            let err = bus.apply(Box::new(RemoveLayer::at(i))).unwrap_err();
            assert_eq!(err, DocumentError::LastLayer);
        }
        assert_eq!(bus.document.layers(), before.as_slice());
        // An empty group can go.
        bus.apply(Box::new(RemoveLayer::at(2))).unwrap();
        assert_eq!(names(&bus.document), ["G", "a"]);
    }

    #[test]
    fn duplicate_layer_on_group_copies_an_independent_subtree() {
        let mut doc = tree_doc(&[
            ("bg", None, false),
            ("G", None, true),
            ("a", Some("G"), false),
            ("top", None, false),
        ]);
        doc.layers[2].pixels.data_mut().fill(9);
        let mut bus = CommandBus::new(doc);
        bus.apply(Box::new(DuplicateLayer::at(1))).unwrap();
        let doc = &bus.document;
        assert_eq!(names(doc), ["bg", "G", "a", "G copy", "a", "top"]);
        assert!(doc.is_well_formed());
        assert_eq!(
            doc.parent_index(4),
            Some(3),
            "the copied child is in the copy"
        );
        assert_eq!(doc.active_layer_index(), 3);
        assert_ne!(doc.layers()[3].id, doc.layers()[1].id);
        assert_ne!(doc.layers()[4].id, doc.layers()[2].id);

        // Editing the copy leaves the original alone.
        bus.document.layers[4].pixels.data_mut().fill(1);
        assert!(
            bus.document.layers()[2]
                .pixels
                .data()
                .iter()
                .all(|&b| b == 9)
        );

        assert!(bus.undo().unwrap());
        assert_eq!(names(&bus.document), ["bg", "G", "a", "top"]);
    }

    #[test]
    fn duplicate_group_beyond_the_layer_limit_is_refused_as_a_whole() {
        let mut doc = Document::new(1, 1).unwrap();
        for _ in 1..crate::document::MAX_LAYERS - 3 {
            doc.add_layer("l").unwrap();
        }
        let g = Layer::new_group("G");
        let gid = g.id;
        let len = doc.layer_count();
        doc.insert_layer(len, g).unwrap();
        for n in 0..2 {
            let c = Layer::new_transparent(format!("c{n}"), 1, 1).with_parent(Some(gid));
            doc.insert_layer(doc.layer_count(), c).unwrap();
        }
        assert_eq!(doc.layer_count(), crate::document::MAX_LAYERS);
        let mut bus = CommandBus::new(doc);
        let before = bus.document.layer_count();
        let err = bus.apply(Box::new(DuplicateLayer::at(len))).unwrap_err();
        assert_eq!(err, DocumentError::LayerLimit { max: 999 });
        assert_eq!(bus.document.layer_count(), before);
    }

    #[test]
    fn add_layer_above_a_child_stays_in_its_group() {
        let mut doc = tree_doc(&[
            ("bg", None, false),
            ("G", None, true),
            ("a", Some("G"), false),
        ]);
        doc.set_active_layer(2).unwrap();
        let mut bus = CommandBus::new(doc);
        bus.apply(Box::new(AddLayer::above(2))).unwrap();
        assert_eq!(bus.document.parent_index(3), Some(1));
        assert!(bus.document.is_well_formed());
    }

    #[test]
    fn add_layer_with_a_group_active_goes_above_the_group_at_its_level() {
        let mut bus = CommandBus::new(tree_doc(&[
            ("bg", None, false),
            ("G", None, true),
            ("a", Some("G"), false),
        ]));
        bus.apply(Box::new(AddLayer::above(1))).unwrap();
        assert_eq!(bus.document.active_layer_index(), 3);
        assert_eq!(bus.document.parent_index(3), None);
        assert!(bus.document.is_well_formed());
        assert!(bus.undo().unwrap());
        assert_eq!(names(&bus.document), ["bg", "G", "a"]);
    }

    #[test]
    fn move_layer_that_would_split_a_group_is_refused() {
        let mut bus = CommandBus::new(tree_doc(&[
            ("bg", None, false),
            ("G", None, true),
            ("a", Some("G"), false),
            ("b", Some("G"), false),
        ]));
        let before = bus.document.layers().to_vec();
        let err = bus.apply(Box::new(MoveLayer::new(0, 2))).unwrap_err();
        assert_eq!(err, DocumentError::LayerTreeBroken);
        assert_eq!(bus.document.layers(), before.as_slice());
        // Reordering siblings inside the group is fine.
        bus.apply(Box::new(MoveLayer::new(2, 3))).unwrap();
        assert_eq!(names(&bus.document), ["bg", "G", "b", "a"]);
        let _ = index(&bus.document, "a");
    }

    #[test]
    fn move_layer_revert_restores_previous_active_layer() {
        let mut doc = Document::new(8, 8).unwrap();
        doc.add_layer("Layer 2").unwrap(); // 3 layers below, indices 0..=2
        doc.add_layer("Layer 3").unwrap();
        doc.set_active_layer(0).unwrap();

        let mut cmd = MoveLayer::new(2, 0);
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        // The active layer selected before the move is restored.
        assert_eq!(doc.active_layer_index(), 0);
    }
}
