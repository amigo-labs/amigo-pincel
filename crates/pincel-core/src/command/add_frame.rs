//! `AddFrame` command — append a frame to the sprite's frame list.
//!
//! M2 supports append-only insertion. Inserting a frame at an arbitrary index
//! would require remapping cel `FrameIndex` keys; that is deferred to a later
//! milestone.

use crate::document::{CelMap, Frame, Sprite};

use super::Command;
use super::error::CommandError;

/// Append a frame to the sprite's playback sequence.
#[derive(Debug, Clone)]
pub struct AddFrame {
    frame: Option<Frame>,
    /// `Some(index)` after `apply`; the position the frame was appended to.
    inserted_index: Option<usize>,
}

impl AddFrame {
    /// Append `frame` to the end of the sprite's frame list.
    pub fn append(frame: Frame) -> Self {
        Self {
            frame: Some(frame),
            inserted_index: None,
        }
    }
}

impl Command for AddFrame {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let frame = self
            .frame
            .take()
            .expect("AddFrame applied without a frame payload");
        let index = doc.frames.len();
        doc.frames.push(frame);
        self.inserted_index = Some(index);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some(index) = self.inserted_index.take() else {
            return;
        };
        if index >= doc.frames.len() {
            return;
        }
        let frame = doc.frames.remove(index);
        self.frame = Some(frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Sprite;

    fn empty_doc() -> (Sprite, CelMap) {
        (
            Sprite::builder(4, 4).build().expect("sprite builds"),
            CelMap::new(),
        )
    }

    #[test]
    fn apply_appends_frame() {
        let (mut sprite, mut cels) = empty_doc();
        let mut cmd = AddFrame::append(Frame::new(50));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(sprite.frames.len(), 1);
        assert_eq!(sprite.frames[0].duration_ms, 50);
    }

    #[test]
    fn revert_removes_appended_frame() {
        let (mut sprite, mut cels) = empty_doc();
        let mut cmd = AddFrame::append(Frame::new(75));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        cmd.revert(&mut sprite, &mut cels);
        assert!(sprite.frames.is_empty());
    }

    #[test]
    fn apply_revert_apply_round_trip_preserves_frame() {
        let (mut sprite, mut cels) = empty_doc();
        let mut cmd = AddFrame::append(Frame::new(33));
        cmd.apply(&mut sprite, &mut cels).expect("apply 1");
        cmd.revert(&mut sprite, &mut cels);
        cmd.apply(&mut sprite, &mut cels).expect("apply 2");
        assert_eq!(sprite.frames.len(), 1);
        assert_eq!(sprite.frames[0].duration_ms, 33);
    }
}
