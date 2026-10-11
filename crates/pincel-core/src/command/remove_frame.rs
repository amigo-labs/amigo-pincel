//! `RemoveFrame` command — delete one frame and close the gap.
//!
//! Every frame-indexed reference after the removed frame shifts down by
//! one (spec §3.2 / §3.5 / §3.6):
//!
//! - cels at later frames are re-keyed (`CelKey` and `Cel::frame`);
//! - `CelData::Linked` targets shift with them, and a link that pointed
//!   at the removed frame is materialised into a copy of the removed cel
//!   (its data has nowhere else to live);
//! - tag ranges shrink or shift; a tag that covered only the removed
//!   frame is dropped;
//! - slice keys shift; a key on the removed frame is kept (it moves to
//!   the frame that slides into its place) unless the next frame already
//!   has its own key.
//!
//! `revert` restores the exact prior state: the removed frame, the
//! original tags and slice keys, and every cel the command touched.

use crate::document::{
    Cel, CelData, CelKey, CelMap, Frame, FrameIndex, Slice, SliceKey, Sprite, Tag,
};

use super::Command;
use super::error::CommandError;

/// Remove the frame at `index`.
#[derive(Debug, Clone)]
pub struct RemoveFrame {
    index: FrameIndex,
    applied: Option<Applied>,
}

/// Everything `revert` needs, captured by `apply`.
#[derive(Debug, Clone)]
struct Applied {
    frame: Frame,
    tags: Vec<Tag>,
    /// Prior `keys` per slice, in `Sprite::slices` order.
    slice_keys: Vec<Vec<SliceKey>>,
    /// Original cels that `apply` removed or rewrote.
    old_cels: Vec<Cel>,
    /// Keys of the rewritten cels `apply` inserted.
    new_keys: Vec<CelKey>,
}

impl RemoveFrame {
    pub fn new(index: FrameIndex) -> Self {
        Self {
            index,
            applied: None,
        }
    }
}

impl Command for RemoveFrame {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let idx = self.index.0;
        if idx as usize >= doc.frames.len() {
            return Err(CommandError::UnknownFrame(idx));
        }
        if doc.frames.len() == 1 {
            return Err(CommandError::LastFrame);
        }

        let frame = doc.frames.remove(idx as usize);
        let new_len = doc.frames.len() as u32;

        let tags = doc.tags.clone();
        doc.tags = shift_tags(&tags, idx);

        let slice_keys: Vec<Vec<SliceKey>> = doc.slices.iter().map(|s| s.keys.clone()).collect();
        for slice in &mut doc.slices {
            shift_slice_keys(slice, idx, new_len);
        }

        // Cels that change: everything at or after the removed frame, plus
        // any cel (at any frame) linked to a frame at or after it.
        let affected: Vec<CelKey> = cels
            .iter()
            .filter(|(key, cel)| {
                key.1.0 >= idx || matches!(cel.data, CelData::Linked(t) if t.0 >= idx)
            })
            .map(|(key, _)| *key)
            .collect();
        let old_cels: Vec<Cel> = affected
            .iter()
            .filter_map(|&(layer, frame)| cels.remove(layer, frame))
            .collect();

        let mut new_keys = Vec::new();
        for old in &old_cels {
            if old.frame.0 == idx {
                continue;
            }
            let mut cel = old.clone();
            if cel.frame.0 > idx {
                cel.frame = FrameIndex::new(cel.frame.0 - 1);
            }
            if let CelData::Linked(target) = cel.data {
                if target.0 == idx {
                    // The link target is going away: copy its content in.
                    match resolve_removed(&old_cels, old.layer, idx) {
                        Some(src) => {
                            cel.position = src.position;
                            cel.opacity = src.opacity;
                            cel.data = src.data.clone();
                        }
                        // Dangling link to an empty cel: nothing to show.
                        None => continue,
                    }
                } else if target.0 > idx {
                    cel.data = CelData::Linked(FrameIndex::new(target.0 - 1));
                }
            }
            new_keys.push((cel.layer, cel.frame));
            cels.insert(cel);
        }

        self.applied = Some(Applied {
            frame,
            tags,
            slice_keys,
            old_cels,
            new_keys,
        });
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap) {
        let Some(applied) = self.applied.take() else {
            return;
        };
        for (layer, frame) in &applied.new_keys {
            cels.remove(*layer, *frame);
        }
        for cel in applied.old_cels {
            cels.insert(cel);
        }
        doc.frames.insert(self.index.0 as usize, applied.frame);
        doc.tags = applied.tags;
        for (slice, keys) in doc.slices.iter_mut().zip(applied.slice_keys) {
            slice.keys = keys;
        }
    }
}

/// The removed frame's cel on `layer`, following one level of linkage to an
/// earlier frame (the removed cel itself may be a link).
fn resolve_removed(old_cels: &[Cel], layer: crate::document::LayerId, idx: u32) -> Option<&Cel> {
    let removed = old_cels
        .iter()
        .find(|c| c.layer == layer && c.frame.0 == idx)?;
    match removed.data {
        CelData::Linked(t) => old_cels
            .iter()
            .find(|c| c.layer == layer && c.frame == t && !matches!(c.data, CelData::Linked(_))),
        _ => Some(removed),
    }
}

/// Tags after removing frame `idx`.
fn shift_tags(tags: &[Tag], idx: u32) -> Vec<Tag> {
    tags.iter()
        .filter_map(|tag| {
            let (from, to) = (tag.from.0, tag.to.0);
            let mut tag = tag.clone();
            if to < idx {
                // Entirely before the removed frame.
            } else if from > idx {
                tag.from = FrameIndex::new(from - 1);
                tag.to = FrameIndex::new(to - 1);
            } else if from == to {
                // The tag covered only the removed frame.
                return None;
            } else {
                tag.to = FrameIndex::new(to - 1);
            }
            Some(tag)
        })
        .collect()
}

/// Shift `slice`'s keys for the removal of frame `idx`; `new_len` is the
/// frame count afterwards.
fn shift_slice_keys(slice: &mut Slice, idx: u32, new_len: u32) {
    let next_has_key = slice.keys.iter().any(|k| k.frame.0 == idx + 1);
    let has_earlier_key = slice.keys.iter().any(|k| k.frame.0 < idx);
    let mut keys = Vec::with_capacity(slice.keys.len());
    for key in &slice.keys {
        let mut key = *key;
        if key.frame.0 == idx {
            // Its geometry carries over to the frame that slides into
            // place, unless that frame brings its own key.
            if next_has_key {
                continue;
            }
            if idx >= new_len {
                // Removed the last frame: the key would apply nowhere.
                if has_earlier_key {
                    continue;
                }
                key.frame = FrameIndex::new(new_len - 1);
            }
        } else if key.frame.0 > idx {
            key.frame = FrameIndex::new(key.frame.0 - 1);
        }
        keys.push(key);
    }
    slice.keys = keys;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{ColorMode, Layer, LayerId, PixelBuffer, Rgba, SliceId};
    use crate::geometry::Rect;

    const L: LayerId = LayerId::new(0);

    fn cel(frame: u32, fill: u8) -> Cel {
        let mut buf = PixelBuffer::empty(1, 1, ColorMode::Rgba);
        buf.data.fill(fill);
        Cel::image(L, FrameIndex::new(frame), buf)
    }

    fn doc(frames: u32) -> (Sprite, CelMap) {
        let mut b = Sprite::builder(4, 4).add_layer(Layer::image(L, "bg"));
        for i in 0..frames {
            b = b.add_frame(Frame::new(100 + i as u16));
        }
        let sprite = b.build().expect("sprite builds");
        let mut cels = CelMap::new();
        for i in 0..frames {
            cels.insert(cel(i, i as u8 + 1));
        }
        (sprite, cels)
    }

    fn fill_at(cels: &CelMap, frame: u32) -> Option<u8> {
        match &cels.get(L, FrameIndex::new(frame))?.data {
            CelData::Image(b) => Some(b.data[0]),
            _ => None,
        }
    }

    #[test]
    fn apply_middle_frame_shifts_later_cels_down() {
        let (mut s, mut c) = doc(3);
        RemoveFrame::new(FrameIndex::new(1))
            .apply(&mut s, &mut c)
            .expect("apply");
        assert_eq!(s.frames.len(), 2);
        assert_eq!(s.frames[1].duration_ms, 102);
        assert_eq!(fill_at(&c, 0), Some(1));
        assert_eq!(fill_at(&c, 1), Some(3));
        assert_eq!(c.len(), 2);
        let moved = c.get(L, FrameIndex::new(1)).expect("cel");
        assert_eq!(moved.frame, FrameIndex::new(1), "Cel::frame re-keyed too");
    }

    #[test]
    fn revert_restores_frames_and_cels_exactly() {
        let (mut s, mut c) = doc(3);
        let (s0, c0) = (s.clone(), c.clone());
        let mut cmd = RemoveFrame::new(FrameIndex::new(0));
        cmd.apply(&mut s, &mut c).expect("apply");
        cmd.revert(&mut s, &mut c);
        assert_eq!(s, s0);
        assert_eq!(c, c0);
    }

    #[test]
    fn apply_last_remaining_frame_returns_err() {
        let (mut s, mut c) = doc(1);
        assert_eq!(
            RemoveFrame::new(FrameIndex::new(0)).apply(&mut s, &mut c),
            Err(CommandError::LastFrame)
        );
        assert_eq!(s.frames.len(), 1);
    }

    #[test]
    fn apply_out_of_range_returns_err() {
        let (mut s, mut c) = doc(2);
        assert_eq!(
            RemoveFrame::new(FrameIndex::new(2)).apply(&mut s, &mut c),
            Err(CommandError::UnknownFrame(2))
        );
    }

    #[test]
    fn apply_adjusts_tags_and_drops_single_frame_tag() {
        let (mut s, mut c) = doc(4);
        s.tags = vec![
            Tag::new("before", FrameIndex::new(0), FrameIndex::new(0)),
            Tag::new("span", FrameIndex::new(0), FrameIndex::new(2)),
            Tag::new("only", FrameIndex::new(1), FrameIndex::new(1)),
            Tag::new("after", FrameIndex::new(2), FrameIndex::new(3)),
        ];
        let tags0 = s.tags.clone();
        let mut cmd = RemoveFrame::new(FrameIndex::new(1));
        cmd.apply(&mut s, &mut c).expect("apply");
        let got: Vec<(&str, u32, u32)> = s
            .tags
            .iter()
            .map(|t| (t.name.as_str(), t.from.0, t.to.0))
            .collect();
        assert_eq!(got, vec![("before", 0, 0), ("span", 0, 1), ("after", 1, 2)]);
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.tags, tags0);
    }

    #[test]
    fn apply_materialises_links_to_the_removed_frame() {
        let (mut s, mut c) = doc(3);
        c.insert(Cel {
            data: CelData::Linked(FrameIndex::new(1)),
            ..cel(2, 0)
        });
        let c0 = c.clone();
        let mut cmd = RemoveFrame::new(FrameIndex::new(1));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(fill_at(&c, 1), Some(2), "copy of the removed frame's cel");
        cmd.revert(&mut s, &mut c);
        assert_eq!(c, c0);
    }

    #[test]
    fn apply_shifts_link_targets_after_the_removed_frame() {
        let (mut s, mut c) = doc(4);
        c.insert(Cel {
            data: CelData::Linked(FrameIndex::new(2)),
            ..cel(3, 0)
        });
        RemoveFrame::new(FrameIndex::new(0))
            .apply(&mut s, &mut c)
            .expect("apply");
        let linked = c.get(L, FrameIndex::new(2)).expect("cel");
        assert_eq!(linked.data, CelData::Linked(FrameIndex::new(1)));
    }

    fn key(frame: u32, x: i32) -> SliceKey {
        SliceKey {
            frame: FrameIndex::new(frame),
            bounds: Rect::new(x, 0, 1, 1),
            center: None,
            pivot: None,
        }
    }

    fn slice(keys: Vec<SliceKey>) -> Slice {
        Slice {
            id: SliceId::new(0),
            name: "s".into(),
            color: Rgba::WHITE,
            keys,
        }
    }

    #[test]
    fn apply_shifts_slice_keys_and_keeps_the_removed_frames_key() {
        let (mut s, mut c) = doc(4);
        s.slices = vec![slice(vec![key(0, 0), key(1, 1), key(3, 3)])];
        let mut cmd = RemoveFrame::new(FrameIndex::new(1));
        cmd.apply(&mut s, &mut c).expect("apply");
        let frames: Vec<(u32, i32)> = s.slices[0]
            .keys
            .iter()
            .map(|k| (k.frame.0, k.bounds.x))
            .collect();
        assert_eq!(frames, vec![(0, 0), (1, 1), (2, 3)]);
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.slices[0].keys, vec![key(0, 0), key(1, 1), key(3, 3)]);
    }

    #[test]
    fn apply_drops_removed_frames_key_when_next_frame_has_one() {
        let (mut s, mut c) = doc(3);
        s.slices = vec![slice(vec![key(0, 0), key(1, 1), key(2, 2)])];
        RemoveFrame::new(FrameIndex::new(1))
            .apply(&mut s, &mut c)
            .expect("apply");
        let xs: Vec<i32> = s.slices[0].keys.iter().map(|k| k.bounds.x).collect();
        assert_eq!(xs, vec![0, 2]);
    }

    #[test]
    fn revert_before_apply_is_a_noop() {
        let (mut s, mut c) = doc(2);
        let (s0, c0) = (s.clone(), c.clone());
        RemoveFrame::new(FrameIndex::new(0)).revert(&mut s, &mut c);
        assert_eq!((s, c), (s0, c0));
    }
}
