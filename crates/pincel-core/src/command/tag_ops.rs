//! Animation-tag commands (spec §3.6): add, remove, re-range, rename and
//! change the playback direction of a [`Tag`].
//!
//! Tags are addressed by their index in `Sprite::tags`. They are playback
//! metadata — none of these commands changes the composite, so all report
//! [`DirtyRegion::None`]. Consecutive edits to the same tag through the
//! same command type merge into one undo entry (a range drag or a name
//! field emits one command per tick).

use crate::document::{CelMap, FrameIndex, Sprite, Tag, TagDirection};

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

fn check_range(doc: &Sprite, from: FrameIndex, to: FrameIndex) -> Result<(), CommandError> {
    if from.0 > to.0 || to.0 as usize >= doc.frames.len() {
        return Err(CommandError::InvalidTagRange {
            from: from.0,
            to: to.0,
            frames: doc.frames.len(),
        });
    }
    Ok(())
}

fn tag_mut(doc: &mut Sprite, index: usize) -> Result<&mut Tag, CommandError> {
    doc.tags
        .get_mut(index)
        .ok_or(CommandError::UnknownTag(index))
}

/// Append a tag to `Sprite::tags`.
#[derive(Debug, Clone)]
pub struct AddTag {
    tag: Tag,
    /// Index the tag was appended at, set by `apply`.
    inserted: Option<usize>,
}

impl AddTag {
    pub fn new(tag: Tag) -> Self {
        Self {
            tag,
            inserted: None,
        }
    }
}

impl Command for AddTag {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        if self.tag.name.is_empty() {
            return Err(CommandError::EmptyTagName);
        }
        check_range(doc, self.tag.from, self.tag.to)?;
        doc.tags.push(self.tag.clone());
        self.inserted = Some(doc.tags.len() - 1);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        if let Some(index) = self.inserted.take()
            && index < doc.tags.len()
        {
            doc.tags.remove(index);
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::None
    }
}

/// Remove the tag at `index`.
#[derive(Debug, Clone)]
pub struct RemoveTag {
    index: usize,
    removed: Option<Tag>,
}

impl RemoveTag {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            removed: None,
        }
    }
}

impl Command for RemoveTag {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        if self.index >= doc.tags.len() {
            return Err(CommandError::UnknownTag(self.index));
        }
        self.removed = Some(doc.tags.remove(self.index));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        if let Some(tag) = self.removed.take() {
            let at = self.index.min(doc.tags.len());
            doc.tags.insert(at, tag);
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::None
    }
}

/// Set the frame range of the tag at `index`.
#[derive(Debug, Clone)]
pub struct SetTagRange {
    index: usize,
    from: FrameIndex,
    to: FrameIndex,
    prev: Option<(FrameIndex, FrameIndex)>,
}

impl SetTagRange {
    pub fn new(index: usize, from: FrameIndex, to: FrameIndex) -> Self {
        Self {
            index,
            from,
            to,
            prev: None,
        }
    }
}

impl Command for SetTagRange {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        check_range(doc, self.from, self.to)?;
        let tag = tag_mut(doc, self.index)?;
        self.prev = Some((tag.from, tag.to));
        tag.from = self.from;
        tag.to = self.to;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        if let Some((from, to)) = self.prev.take()
            && let Ok(tag) = tag_mut(doc, self.index)
        {
            tag.from = from;
            tag.to = to;
        }
    }

    fn merge(&mut self, next: &Self) -> bool {
        if self.index != next.index {
            return false;
        }
        self.from = next.from;
        self.to = next.to;
        true
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::None
    }
}

/// Rename the tag at `index`.
///
/// The spec asks the editor to warn on names that are not identifier-like
/// (the animation state machine keys on them); that is a UI concern, so
/// the core only rejects the empty name.
#[derive(Debug, Clone)]
pub struct SetTagName {
    index: usize,
    name: String,
    prev: Option<String>,
}

impl SetTagName {
    pub fn new(index: usize, name: impl Into<String>) -> Self {
        Self {
            index,
            name: name.into(),
            prev: None,
        }
    }
}

impl Command for SetTagName {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        if self.name.is_empty() {
            return Err(CommandError::EmptyTagName);
        }
        let tag = tag_mut(doc, self.index)?;
        self.prev = Some(std::mem::replace(&mut tag.name, self.name.clone()));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        if let Some(prev) = self.prev.take()
            && let Ok(tag) = tag_mut(doc, self.index)
        {
            tag.name = prev;
        }
    }

    fn merge(&mut self, next: &Self) -> bool {
        if self.index != next.index {
            return false;
        }
        self.name.clone_from(&next.name);
        true
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::None
    }
}

/// Set the playback direction of the tag at `index`.
#[derive(Debug, Clone)]
pub struct SetTagDirection {
    index: usize,
    direction: TagDirection,
    prev: Option<(TagDirection, Option<u8>)>,
}

impl SetTagDirection {
    pub fn new(index: usize, direction: TagDirection) -> Self {
        Self {
            index,
            direction,
            prev: None,
        }
    }
}

impl Command for SetTagDirection {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let tag = tag_mut(doc, self.index)?;
        self.prev = Some((tag.direction, tag.raw_direction.take()));
        tag.direction = self.direction;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        if let Some((prev, raw)) = self.prev.take()
            && let Ok(tag) = tag_mut(doc, self.index)
        {
            tag.direction = prev;
            tag.raw_direction = raw;
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Frame, Layer, LayerId};

    fn doc() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(4, 4)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_frame(Frame::new(100))
            .add_frame(Frame::new(100))
            .add_frame(Frame::new(100))
            .add_tag(Tag::new("idle", FrameIndex::new(0), FrameIndex::new(1)))
            .build()
            .expect("sprite builds");
        (sprite, CelMap::new())
    }

    fn f(i: u32) -> FrameIndex {
        FrameIndex::new(i)
    }

    #[test]
    fn add_tag_appends_and_revert_removes() {
        let (mut s, mut c) = doc();
        let mut cmd = AddTag::new(Tag::new("run", f(1), f(2)));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(s.tags.len(), 2);
        assert_eq!(s.tags[1].name, "run");
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.tags.len(), 1);
    }

    #[test]
    fn add_tag_with_range_past_last_frame_returns_err() {
        let (mut s, mut c) = doc();
        assert_eq!(
            AddTag::new(Tag::new("x", f(1), f(3))).apply(&mut s, &mut c),
            Err(CommandError::InvalidTagRange {
                from: 1,
                to: 3,
                frames: 3
            })
        );
    }

    #[test]
    fn add_tag_with_empty_name_returns_err() {
        let (mut s, mut c) = doc();
        assert_eq!(
            AddTag::new(Tag::new("", f(0), f(0))).apply(&mut s, &mut c),
            Err(CommandError::EmptyTagName)
        );
    }

    #[test]
    fn remove_tag_round_trips() {
        let (mut s, mut c) = doc();
        let t0 = s.tags.clone();
        let mut cmd = RemoveTag::new(0);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert!(s.tags.is_empty());
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.tags, t0);
    }

    #[test]
    fn remove_tag_unknown_index_returns_err() {
        let (mut s, mut c) = doc();
        assert_eq!(
            RemoveTag::new(4).apply(&mut s, &mut c),
            Err(CommandError::UnknownTag(4))
        );
    }

    #[test]
    fn set_tag_range_applies_and_reverts() {
        let (mut s, mut c) = doc();
        let mut cmd = SetTagRange::new(0, f(1), f(2));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!((s.tags[0].from, s.tags[0].to), (f(1), f(2)));
        cmd.revert(&mut s, &mut c);
        assert_eq!((s.tags[0].from, s.tags[0].to), (f(0), f(1)));
    }

    #[test]
    fn set_tag_range_inverted_returns_err() {
        let (mut s, mut c) = doc();
        assert!(matches!(
            SetTagRange::new(0, f(2), f(1)).apply(&mut s, &mut c),
            Err(CommandError::InvalidTagRange { .. })
        ));
        assert_eq!(s.tags[0].to, f(1), "document unchanged");
    }

    #[test]
    fn set_tag_range_merge_keeps_the_first_prior_range() {
        let (mut s, mut c) = doc();
        let mut first = SetTagRange::new(0, f(0), f(2));
        first.apply(&mut s, &mut c).expect("apply");
        let mut second = SetTagRange::new(0, f(1), f(2));
        second.apply(&mut s, &mut c).expect("apply");
        assert!(first.merge(&second));
        first.revert(&mut s, &mut c);
        assert_eq!((s.tags[0].from, s.tags[0].to), (f(0), f(1)));
    }

    #[test]
    fn set_tag_name_applies_and_reverts() {
        let (mut s, mut c) = doc();
        let mut cmd = SetTagName::new(0, "walk");
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(s.tags[0].name, "walk");
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.tags[0].name, "idle");
    }

    #[test]
    fn set_tag_name_empty_returns_err() {
        let (mut s, mut c) = doc();
        assert_eq!(
            SetTagName::new(0, "").apply(&mut s, &mut c),
            Err(CommandError::EmptyTagName)
        );
    }

    #[test]
    fn set_tag_direction_applies_and_reverts() {
        let (mut s, mut c) = doc();
        let mut cmd = SetTagDirection::new(0, TagDirection::Pingpong);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(s.tags[0].direction, TagDirection::Pingpong);
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.tags[0].direction, TagDirection::Forward);
    }

    #[test]
    fn tag_commands_report_no_dirty_region() {
        assert_eq!(RemoveTag::new(0).dirty_region(), DirtyRegion::None);
        assert_eq!(
            SetTagDirection::new(0, TagDirection::Reverse).dirty_region(),
            DirtyRegion::None
        );
    }

    #[test]
    fn set_tag_direction_clears_raw_direction_and_undo_restores_it() {
        let mut doc = Sprite::builder(1, 1)
            .add_layer(Layer::image(LayerId::new(0), "L"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite");
        let mut tag = Tag::new("t", FrameIndex::new(0), FrameIndex::new(0));
        tag.raw_direction = Some(9);
        doc.tags.push(tag);
        let mut cels = CelMap::new();
        let mut cmd = SetTagDirection::new(0, TagDirection::Reverse);
        cmd.apply(&mut doc, &mut cels).expect("apply");
        assert_eq!(doc.tags[0].raw_direction, None);
        assert_eq!(doc.tags[0].direction, TagDirection::Reverse);
        cmd.revert(&mut doc, &mut cels);
        assert_eq!(doc.tags[0].raw_direction, Some(9));
        assert_eq!(doc.tags[0].direction, TagDirection::Forward);
    }
}
