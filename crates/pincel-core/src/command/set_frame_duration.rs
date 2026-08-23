//! `SetFrameDuration` command — change a frame's display duration.
//!
//! Duration is playback metadata (spec §3.3): it drives the timeline's
//! per-frame timing and round-trips through the Aseprite frame header. It
//! contributes nothing to the composite, so the command reports
//! [`DirtyRegion::None`] and the UI repaints the timeline only.
//!
//! Consecutive edits to the same frame merge into one undo entry — a
//! duration spinner or drag emits a command per tick, and undo should step
//! back to the value before the interaction, not through every tick.

use crate::document::{CelMap, FrameIndex, Sprite};

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// Set `frame`'s `duration_ms`.
#[derive(Debug, Clone)]
pub struct SetFrameDuration {
    frame: FrameIndex,
    duration_ms: u16,
    /// Prior `duration_ms`, captured on `apply` for `revert`.
    prev: Option<u16>,
}

impl SetFrameDuration {
    pub fn new(frame: FrameIndex, duration_ms: u16) -> Self {
        Self {
            frame,
            duration_ms,
            prev: None,
        }
    }
}

impl Command for SetFrameDuration {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let frame = doc
            .frames
            .get_mut(self.frame.0 as usize)
            .ok_or(CommandError::UnknownFrame(self.frame.0))?;
        self.prev = Some(frame.duration_ms);
        frame.duration_ms = self.duration_ms;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some(prev) = self.prev.take() else {
            return;
        };
        if let Some(frame) = doc.frames.get_mut(self.frame.0 as usize) {
            frame.duration_ms = prev;
        }
    }

    /// Absorb a follow-up edit to the same frame. `self.prev` is kept — it
    /// holds the value from before the first edit, which is what `revert`
    /// must restore.
    fn merge(&mut self, next: &Self) -> bool {
        if self.frame != next.frame {
            return false;
        }
        self.duration_ms = next.duration_ms;
        true
    }

    fn dirty_region(&self) -> DirtyRegion {
        // Duration is playback metadata; the composited pixels don't change.
        DirtyRegion::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Frame, Layer, LayerId, Sprite};

    fn doc_two_frames() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(8, 8)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_frame(Frame::new(100))
            .add_frame(Frame::new(200))
            .build()
            .expect("sprite builds");
        (sprite, CelMap::new())
    }

    #[test]
    fn apply_sets_and_revert_restores_duration() {
        let (mut s, mut c) = doc_two_frames();
        let mut cmd = SetFrameDuration::new(FrameIndex::new(1), 33);
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(s.frames[1].duration_ms, 33);
        assert_eq!(s.frames[0].duration_ms, 100, "other frames untouched");
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.frames[1].duration_ms, 200);
    }

    #[test]
    fn apply_with_out_of_range_frame_returns_err() {
        let (mut s, mut c) = doc_two_frames();
        let mut cmd = SetFrameDuration::new(FrameIndex::new(7), 33);
        assert_eq!(
            cmd.apply(&mut s, &mut c),
            Err(CommandError::UnknownFrame(7))
        );
        assert_eq!(s.frames[0].duration_ms, 100, "document unchanged");
    }

    #[test]
    fn revert_before_apply_is_a_noop() {
        let (mut s, mut c) = doc_two_frames();
        let mut cmd = SetFrameDuration::new(FrameIndex::new(0), 33);
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.frames[0].duration_ms, 100);
    }

    #[test]
    fn revert_after_failed_apply_is_a_noop() {
        let (mut s, mut c) = doc_two_frames();
        let mut cmd = SetFrameDuration::new(FrameIndex::new(7), 33);
        let _ = cmd.apply(&mut s, &mut c);
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.frames[0].duration_ms, 100);
        assert_eq!(s.frames[1].duration_ms, 200);
    }

    #[test]
    fn merge_same_frame_keeps_the_original_prior_value() {
        let (mut s, mut c) = doc_two_frames();
        let mut first = SetFrameDuration::new(FrameIndex::new(0), 50);
        first.apply(&mut s, &mut c).expect("apply");
        let second = SetFrameDuration::new(FrameIndex::new(0), 60);
        assert!(first.merge(&second));

        // The merged command now represents "set to 60"; applying the
        // absorbed value and reverting must land back on the pre-merge 100.
        let mut merged_apply = SetFrameDuration::new(FrameIndex::new(0), 60);
        merged_apply.apply(&mut s, &mut c).expect("apply");
        assert_eq!(s.frames[0].duration_ms, 60);
        first.revert(&mut s, &mut c);
        assert_eq!(s.frames[0].duration_ms, 100);
    }

    #[test]
    fn merge_different_frame_is_rejected() {
        let mut first = SetFrameDuration::new(FrameIndex::new(0), 50);
        let second = SetFrameDuration::new(FrameIndex::new(1), 50);
        assert!(!first.merge(&second));
    }

    #[test]
    fn dirty_region_is_none() {
        assert_eq!(
            SetFrameDuration::new(FrameIndex::new(0), 50).dirty_region(),
            DirtyRegion::None
        );
    }
}
