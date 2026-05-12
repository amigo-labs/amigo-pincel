//! `SetSliceKey` command — upsert a per-frame [`SliceKey`] on a slice.
//!
//! If the targeted slice already carries a key for the given frame, the
//! existing key is replaced and stashed for revert. Otherwise the new
//! key is inserted at the sorted-by-frame position required by
//! [`Slice::keys`]'s ascending-frame invariant, and revert removes it.

use crate::document::{CelMap, SliceId, SliceKey, Sprite};

use super::Command;
use super::error::CommandError;

/// Slot taken by the prior key, used by `revert`.
#[derive(Debug, Clone)]
enum PriorSlot {
    /// The new key replaced an existing one at this index.
    Replaced { index: usize, prior: SliceKey },
    /// The new key was inserted at this index; nothing was there before.
    Inserted { index: usize },
}

/// Set (insert or replace) a [`SliceKey`] on the slice identified by
/// [`SliceId`]. `revert` restores the prior key, or removes the inserted
/// one when no key existed for the frame.
#[derive(Debug, Clone)]
pub struct SetSliceKey {
    slice: SliceId,
    new_key: SliceKey,
    /// `Some` after a successful `apply`; consumed on `revert`.
    prior: Option<PriorSlot>,
}

impl SetSliceKey {
    pub fn new(slice: SliceId, key: SliceKey) -> Self {
        Self {
            slice,
            new_key: key,
            prior: None,
        }
    }
}

impl Command for SetSliceKey {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        if self.new_key.bounds.is_empty() {
            return Err(CommandError::EmptySliceBounds {
                frame: self.new_key.frame,
                bounds: self.new_key.bounds,
            });
        }
        let slice = doc
            .slices
            .iter_mut()
            .find(|s| s.id == self.slice)
            .ok_or(CommandError::UnknownSlice(self.slice.0))?;
        // Keys are sorted by `frame` ascending. `partition_point` lands
        // either on an existing key with the same frame (replace) or on
        // the slot where the new key belongs (insert).
        let index = slice
            .keys
            .partition_point(|k| k.frame < self.new_key.frame);
        if slice
            .keys
            .get(index)
            .is_some_and(|k| k.frame == self.new_key.frame)
        {
            let prior = std::mem::replace(&mut slice.keys[index], self.new_key);
            self.prior = Some(PriorSlot::Replaced { index, prior });
        } else {
            slice.keys.insert(index, self.new_key);
            self.prior = Some(PriorSlot::Inserted { index });
        }
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some(prior) = self.prior.take() else {
            return;
        };
        let Some(slice) = doc.slices.iter_mut().find(|s| s.id == self.slice) else {
            return;
        };
        match prior {
            PriorSlot::Replaced { index, prior } => {
                if index < slice.keys.len() {
                    slice.keys[index] = prior;
                }
            }
            PriorSlot::Inserted { index } => {
                if index < slice.keys.len() {
                    slice.keys.remove(index);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{FrameIndex, Rgba, Slice};
    use crate::geometry::Rect;

    fn slice_with_keys(id: u32, keys: Vec<SliceKey>) -> Slice {
        Slice {
            id: SliceId::new(id),
            name: "s".into(),
            color: Rgba::WHITE,
            keys,
        }
    }

    fn key(frame: u32, bounds: Rect) -> SliceKey {
        SliceKey {
            frame: FrameIndex::new(frame),
            bounds,
            center: None,
            pivot: None,
        }
    }

    fn doc_with(slice: Slice) -> (Sprite, CelMap) {
        let sprite = Sprite::builder(32, 32)
            .add_slice(slice)
            .build()
            .expect("sprite builds");
        (sprite, CelMap::new())
    }

    #[test]
    fn apply_replaces_existing_key_for_same_frame() {
        let (mut sprite, mut cels) =
            doc_with(slice_with_keys(1, vec![key(0, Rect::new(0, 0, 4, 4))]));
        let new = key(0, Rect::new(2, 3, 6, 7));
        let mut cmd = SetSliceKey::new(SliceId::new(1), new);
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        let keys = &sprite.slices[0].keys;
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].bounds, Rect::new(2, 3, 6, 7));
    }

    #[test]
    fn apply_inserts_new_key_at_sorted_position() {
        let (mut sprite, mut cels) = doc_with(slice_with_keys(
            1,
            vec![
                key(0, Rect::new(0, 0, 4, 4)),
                key(5, Rect::new(1, 1, 4, 4)),
            ],
        ));
        let mut cmd = SetSliceKey::new(SliceId::new(1), key(2, Rect::new(7, 7, 3, 3)));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        let keys = &sprite.slices[0].keys;
        assert_eq!(keys.len(), 3);
        assert_eq!(
            keys.iter().map(|k| k.frame.0).collect::<Vec<_>>(),
            vec![0, 2, 5],
            "keys must stay sorted by frame ascending"
        );
        assert_eq!(keys[1].bounds, Rect::new(7, 7, 3, 3));
    }

    #[test]
    fn revert_after_replace_restores_prior_key() {
        let original = key(0, Rect::new(0, 0, 4, 4));
        let (mut sprite, mut cels) = doc_with(slice_with_keys(1, vec![original]));
        let mut cmd = SetSliceKey::new(SliceId::new(1), key(0, Rect::new(9, 9, 1, 1)));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        cmd.revert(&mut sprite, &mut cels);
        let keys = &sprite.slices[0].keys;
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].bounds, Rect::new(0, 0, 4, 4));
    }

    #[test]
    fn revert_after_insert_removes_inserted_key() {
        let (mut sprite, mut cels) = doc_with(slice_with_keys(
            1,
            vec![
                key(0, Rect::new(0, 0, 4, 4)),
                key(5, Rect::new(1, 1, 4, 4)),
            ],
        ));
        let mut cmd = SetSliceKey::new(SliceId::new(1), key(2, Rect::new(7, 7, 3, 3)));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        cmd.revert(&mut sprite, &mut cels);
        let keys = &sprite.slices[0].keys;
        assert_eq!(
            keys.iter().map(|k| k.frame.0).collect::<Vec<_>>(),
            vec![0, 5]
        );
    }

    #[test]
    fn unknown_slice_is_rejected() {
        let (mut sprite, mut cels) =
            doc_with(slice_with_keys(1, vec![key(0, Rect::new(0, 0, 4, 4))]));
        let mut cmd = SetSliceKey::new(SliceId::new(99), key(0, Rect::new(0, 0, 4, 4)));
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::UnknownSlice(99))
        );
        assert_eq!(sprite.slices[0].keys.len(), 1);
    }

    #[test]
    fn empty_bounds_rect_is_rejected() {
        let (mut sprite, mut cels) =
            doc_with(slice_with_keys(1, vec![key(0, Rect::new(0, 0, 4, 4))]));
        let bad = Rect::new(2, 2, 0, 4);
        let mut cmd = SetSliceKey::new(SliceId::new(1), key(0, bad));
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::EmptySliceBounds {
                frame: FrameIndex::new(0),
                bounds: bad,
            })
        );
        assert_eq!(sprite.slices[0].keys[0].bounds, Rect::new(0, 0, 4, 4));
    }

    #[test]
    fn apply_revert_apply_round_trip_restores_state() {
        let (mut sprite, mut cels) =
            doc_with(slice_with_keys(1, vec![key(0, Rect::new(0, 0, 4, 4))]));
        let new = key(0, Rect::new(2, 3, 6, 7));
        let mut cmd = SetSliceKey::new(SliceId::new(1), new);
        cmd.apply(&mut sprite, &mut cels).expect("apply 1");
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(sprite.slices[0].keys[0].bounds, Rect::new(0, 0, 4, 4));
        cmd.apply(&mut sprite, &mut cels).expect("apply 2");
        assert_eq!(sprite.slices[0].keys[0].bounds, Rect::new(2, 3, 6, 7));
    }
}
