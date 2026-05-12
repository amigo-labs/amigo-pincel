//! `AddSlice` command — append a slice to the sprite's slice list.
//!
//! Slices have no rendering z-order between each other (they're overlay-
//! only and looked up by id), but the list's appearance order is still
//! meaningful: `pincel-core::codec::aseprite_write` serializes slices in
//! list order, and `aseprite_read` assigns sequential `SliceId`s by that
//! same order. `RemoveSlice` therefore re-inserts at the prior index on
//! revert, and this command appends to the tail so newly added slices
//! land at the end. Validation rejects duplicate ids, empty key vectors
//! (the aseprite codec refuses them on write), and per-key empty
//! bounding rects.

use crate::document::{CelMap, Slice, Sprite};

use super::Command;
use super::error::CommandError;

/// Append a [`Slice`] to the sprite's slice list. `revert` pops it back.
#[derive(Debug, Clone)]
pub struct AddSlice {
    /// `Some` until applied; consumed on `apply` and re-populated on `revert`.
    slice: Option<Slice>,
    /// `Some(index)` after a successful `apply`; consumed on `revert`.
    inserted_index: Option<usize>,
}

impl AddSlice {
    /// Append `slice` to the sprite's slice list.
    pub fn new(slice: Slice) -> Self {
        Self {
            slice: Some(slice),
            inserted_index: None,
        }
    }
}

impl Command for AddSlice {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let slice = self
            .slice
            .as_ref()
            .expect("AddSlice applied without a slice payload");

        if doc.slices.iter().any(|s| s.id == slice.id) {
            return Err(CommandError::DuplicateSliceId(slice.id.0));
        }
        if slice.keys.is_empty() {
            return Err(CommandError::EmptySliceKeys(slice.id.0));
        }
        if let Some(bad) = slice.keys.iter().find(|k| k.bounds.is_empty()) {
            return Err(CommandError::EmptySliceBounds {
                frame: bad.frame,
                bounds: bad.bounds,
            });
        }

        let slice = self.slice.take().expect("payload checked above");
        let index = doc.slices.len();
        doc.slices.push(slice);
        self.inserted_index = Some(index);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some(index) = self.inserted_index.take() else {
            return;
        };
        if index < doc.slices.len() {
            let slice = doc.slices.remove(index);
            self.slice = Some(slice);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{FrameIndex, Rgba, SliceId, SliceKey};
    use crate::geometry::Rect;

    fn empty_doc() -> (Sprite, CelMap) {
        (
            Sprite::builder(16, 16).build().expect("sprite builds"),
            CelMap::new(),
        )
    }

    fn slice(id: u32, name: &str) -> Slice {
        Slice {
            id: SliceId::new(id),
            name: name.into(),
            color: Rgba::WHITE,
            keys: vec![SliceKey {
                frame: FrameIndex::new(0),
                bounds: Rect::new(0, 0, 4, 4),
                center: None,
                pivot: None,
            }],
        }
    }

    #[test]
    fn apply_appends_slice() {
        let (mut sprite, mut cels) = empty_doc();
        let mut cmd = AddSlice::new(slice(1, "hitbox"));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(sprite.slices.len(), 1);
        assert_eq!(sprite.slices[0].id, SliceId::new(1));
        assert_eq!(sprite.slices[0].name, "hitbox");
    }

    #[test]
    fn revert_removes_inserted_slice() {
        let (mut sprite, mut cels) = empty_doc();
        let mut cmd = AddSlice::new(slice(7, "s"));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(sprite.slices.len(), 1);
        cmd.revert(&mut sprite, &mut cels);
        assert!(sprite.slices.is_empty());
    }

    #[test]
    fn duplicate_id_is_rejected() {
        let (mut sprite, mut cels) = empty_doc();
        AddSlice::new(slice(1, "a"))
            .apply(&mut sprite, &mut cels)
            .expect("apply 1");
        let mut dup = AddSlice::new(slice(1, "b"));
        assert_eq!(
            dup.apply(&mut sprite, &mut cels),
            Err(CommandError::DuplicateSliceId(1))
        );
        assert_eq!(sprite.slices.len(), 1);
    }

    #[test]
    fn empty_keys_is_rejected() {
        let (mut sprite, mut cels) = empty_doc();
        let s = Slice {
            id: SliceId::new(3),
            name: "empty".into(),
            color: Rgba::WHITE,
            keys: Vec::new(),
        };
        let mut cmd = AddSlice::new(s);
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::EmptySliceKeys(3))
        );
        assert!(sprite.slices.is_empty());
    }

    #[test]
    fn empty_bounds_rect_is_rejected() {
        let (mut sprite, mut cels) = empty_doc();
        let bad = Rect::new(2, 2, 0, 4);
        let s = Slice {
            id: SliceId::new(5),
            name: "bad".into(),
            color: Rgba::WHITE,
            keys: vec![SliceKey {
                frame: FrameIndex::new(0),
                bounds: bad,
                center: None,
                pivot: None,
            }],
        };
        let mut cmd = AddSlice::new(s);
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::EmptySliceBounds {
                frame: FrameIndex::new(0),
                bounds: bad,
            })
        );
        assert!(sprite.slices.is_empty());
    }

    #[test]
    fn apply_revert_apply_round_trip_restores_state() {
        let (mut sprite, mut cels) = empty_doc();
        let original = slice(2, "round");
        let mut cmd = AddSlice::new(original.clone());
        cmd.apply(&mut sprite, &mut cels).expect("apply 1");
        cmd.revert(&mut sprite, &mut cels);
        assert!(sprite.slices.is_empty());
        cmd.apply(&mut sprite, &mut cels).expect("apply 2");
        assert_eq!(sprite.slices, vec![original]);
    }
}
