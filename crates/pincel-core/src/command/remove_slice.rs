//! `RemoveSlice` command — drop a slice from the sprite's slice list.
//!
//! `apply` records the removed slice together with its previous index so
//! `revert` can re-insert it at the same position, preserving the
//! appearance order that the codec serializes by (see
//! `pincel-core::codec::aseprite_write`).

use crate::document::{CelMap, Slice, Sprite, SliceId};

use super::Command;
use super::error::CommandError;

/// Remove the slice identified by [`SliceId`] from the sprite. `revert`
/// re-inserts it at the same position.
#[derive(Debug, Clone)]
pub struct RemoveSlice {
    id: SliceId,
    /// `Some((index, slice))` after a successful `apply`; consumed on `revert`.
    removed: Option<(usize, Slice)>,
}

impl RemoveSlice {
    pub fn new(id: SliceId) -> Self {
        Self { id, removed: None }
    }
}

impl Command for RemoveSlice {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let index = doc
            .slices
            .iter()
            .position(|s| s.id == self.id)
            .ok_or(CommandError::UnknownSlice(self.id.0))?;
        let slice = doc.slices.remove(index);
        self.removed = Some((index, slice));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some((index, slice)) = self.removed.take() else {
            return;
        };
        let target = index.min(doc.slices.len());
        doc.slices.insert(target, slice);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{FrameIndex, Rgba, SliceKey};
    use crate::geometry::Rect;

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

    fn doc_with_slices(slices: Vec<Slice>) -> (Sprite, CelMap) {
        let mut builder = Sprite::builder(16, 16);
        for s in slices {
            builder = builder.add_slice(s);
        }
        (builder.build().expect("sprite builds"), CelMap::new())
    }

    #[test]
    fn apply_removes_matching_slice() {
        let (mut sprite, mut cels) =
            doc_with_slices(vec![slice(1, "a"), slice(2, "b"), slice(3, "c")]);
        let mut cmd = RemoveSlice::new(SliceId::new(2));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(sprite.slices.len(), 2);
        assert_eq!(sprite.slices[0].id, SliceId::new(1));
        assert_eq!(sprite.slices[1].id, SliceId::new(3));
    }

    #[test]
    fn revert_reinserts_at_same_index() {
        let (mut sprite, mut cels) =
            doc_with_slices(vec![slice(1, "a"), slice(2, "b"), slice(3, "c")]);
        let mut cmd = RemoveSlice::new(SliceId::new(2));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(sprite.slices.len(), 3);
        assert_eq!(
            sprite.slices.iter().map(|s| s.id.0).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn unknown_id_is_rejected() {
        let (mut sprite, mut cels) = doc_with_slices(vec![slice(1, "a")]);
        let mut cmd = RemoveSlice::new(SliceId::new(99));
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::UnknownSlice(99))
        );
        assert_eq!(sprite.slices.len(), 1);
    }

    #[test]
    fn revert_before_apply_is_a_noop() {
        let (mut sprite, mut cels) = doc_with_slices(vec![slice(1, "a")]);
        let mut cmd = RemoveSlice::new(SliceId::new(1));
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(sprite.slices.len(), 1);
    }

    #[test]
    fn apply_revert_apply_round_trip_restores_state() {
        let (mut sprite, mut cels) = doc_with_slices(vec![slice(1, "a"), slice(2, "b")]);
        let mut cmd = RemoveSlice::new(SliceId::new(1));
        cmd.apply(&mut sprite, &mut cels).expect("apply 1");
        assert_eq!(sprite.slices.len(), 1);
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(
            sprite.slices.iter().map(|s| s.id.0).collect::<Vec<_>>(),
            vec![1, 2]
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply 2");
        assert_eq!(sprite.slices.len(), 1);
        assert_eq!(sprite.slices[0].id, SliceId::new(2));
    }
}
